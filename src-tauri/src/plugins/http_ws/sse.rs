//! SSE 增量解析与任务所有权；停止请求立即取消网络读取，不自动重连。
use super::models::{HttpRequestPayload, SseEvent, SseUpdate};
use super::sse_flow::{Acknowledgements, Window};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, Manager, State};

/// 每条连接拥有可取消任务，页签关闭和工具退出均释放它。
#[derive(Default)]
pub struct SseState(Mutex<HashMap<String, Session>>);

struct Session {
    task: tauri::async_runtime::JoinHandle<()>,
    acknowledgements: Arc<Acknowledgements>,
}

/// 保留未完成字节行，避免网络分块切断 UTF-8 或 CRLF。
#[derive(Default)]
struct Parser {
    line: Vec<u8>,
    skip_lf: bool,
    started: bool,
    data: String,
    has_data: bool,
    event: String,
    id: String,
    retry: Option<u64>,
}

impl Parser {
    /// 按字节分隔行，只有空行才派发完整事件；流结束不补发残缺事件。
    fn push(
        &mut self,
        bytes: &[u8],
        mut emit: impl FnMut(SseEvent) -> Result<(), String>,
    ) -> Result<(), String> {
        for &byte in bytes {
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\r' || byte == b'\n' {
                self.skip_lf = byte == b'\r';
                if let Some(event) = self.finish_line()? {
                    emit(event)?;
                }
            } else {
                if self.line.len() >= 1024 * 1024 {
                    return Err("SSE 单行超过 1 MiB".into());
                }
                self.line.push(byte);
            }
        }
        Ok(())
    }

    fn finish_line(&mut self) -> Result<Option<SseEvent>, String> {
        let bytes = std::mem::take(&mut self.line);
        // SSE 按 UTF-8 解码，无效字节依标准替换，不影响后续事件。
        let text = String::from_utf8_lossy(&bytes);
        let text = if !self.started {
            self.started = true;
            text.trim_start_matches('\u{feff}')
        } else {
            &text
        };
        if text.is_empty() {
            let event = std::mem::take(&mut self.event);
            if !self.has_data {
                return Ok(None);
            }
            self.has_data = false;
            self.data.pop();
            return Ok(Some(SseEvent {
                event: if event.is_empty() {
                    "message".into()
                } else {
                    event
                },
                id: self.id.clone(),
                data: std::mem::take(&mut self.data),
                retry: self.retry,
            }));
        }
        if text.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = text.split_once(':').unwrap_or((text, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => {
                if self.data.len() + value.len() + 1 > 1024 * 1024 {
                    return Err("SSE 单个事件超过 1 MiB".into());
                }
                self.data.push_str(value);
                self.data.push('\n');
                self.has_data = true;
            }
            "event" => self.event = value.into(),
            "id" if !value.contains('\0') => self.id = value.into(),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                if let Ok(retry) = value.parse() {
                    self.retry = Some(retry);
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

struct Batch {
    consumed: usize,
    events: Vec<SseEvent>,
    bytes: usize,
    error: Option<String>,
}

/// 以输出字节控制批次；重复的大 id/event 元数据也计入，不能仅按输入分块。
fn parse_batch(parser: &mut Parser, input: &[u8]) -> Batch {
    let mut batch = Batch {
        consumed: 0,
        events: Vec::new(),
        bytes: 64,
        error: None,
    };
    for byte in input {
        batch.consumed += 1;
        if let Err(error) = parser.push(std::slice::from_ref(byte), |event| {
            batch.bytes += 64 + event.data.len() + event.id.len() + event.event.len();
            batch.events.push(event);
            Ok(())
        }) {
            batch.error = Some(error);
            break;
        }
        if batch.bytes >= 64 * 1024 {
            break;
        }
    }
    batch
}

async fn receive(
    app: &AppHandle,
    payload: HttpRequestPayload,
    channel: &Channel<SseUpdate>,
    acknowledgements: &Acknowledgements,
    mut window: Option<Window>,
) -> Result<(), String> {
    let timeout = Duration::from_millis(payload.timeout_ms.unwrap_or(15_000).clamp(1_000, 300_000));
    let request = super::http::request(app, payload, true)?;
    let mut response = tokio::time::timeout(timeout, request.send())
        .await
        .map_err(|_| "SSE 连接超时".to_string())?
        .map_err(|e| e.without_url().to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "SSE 服务器返回 HTTP {}",
            response.status().as_u16()
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("text/event-stream")
    {
        return Err("响应类型不是 text/event-stream".into());
    }
    log::info!("SSE 连接已建立 status={}", response.status().as_u16());
    channel
        .send(SseUpdate::Connected {
            status: response.status().as_u16(),
            headers: super::http::response_headers(&response),
        })
        .map_err(|e| e.to_string())?;
    let mut parser = Parser::default();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| e.without_url().to_string())?
    {
        if let Some(window) = &mut window {
            // 限制一次解析产生的批次，而非网络正文或完整事件大小。
            let mut offset = 0;
            while offset < chunk.len() {
                let end = (offset + 16 * 1024).min(chunk.len());
                let batch = parse_batch(&mut parser, &chunk[offset..end]);
                offset += batch.consumed;
                if !batch.events.is_empty() {
                    let sequence = window.reserve(batch.bytes).await?;
                    acknowledgements.sent.store(sequence, Ordering::Release);
                    channel
                        .send(SseUpdate::Events {
                            sequence,
                            events: batch.events,
                        })
                        .map_err(|e| e.to_string())?;
                }
                // 同一网络块中错误之前已完成的事件照常交付。
                if let Some(error) = batch.error {
                    return Err(error);
                }
            }
        } else {
            parser.push(&chunk, |event| {
                channel
                    .send(SseUpdate::Event { event })
                    .map_err(|e| e.to_string())
            })?;
        }
    }
    if let Some(window) = &mut window {
        window.finish().await?;
    }
    channel.send(SseUpdate::Closed).map_err(|e| e.to_string())
}

/// 先登记任务再返回；通道属于单个接口页签，重复 ID 拒绝覆盖。
#[tauri::command]
pub fn sse_start(
    app: AppHandle,
    state: State<'_, SseState>,
    id: String,
    payload: HttpRequestPayload,
    on_event: Channel<SseUpdate>,
    flow_control: Option<bool>,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = (|| {
        let mut tasks = state.0.lock().map_err(|e| e.to_string())?;
        if tasks.contains_key(&id) {
            return Err("SSE 会话已经存在".into());
        }
        let task_id = id.clone();
        let (acknowledgements, window) = Acknowledgements::new();
        let acknowledgements = Arc::new(acknowledgements);
        let task_acknowledgements = acknowledgements.clone();
        let task = tauri::async_runtime::spawn(async move {
            log::info!("SSE 接收任务开始 session={task_id}");
            if let Err(message) = receive(
                &app,
                payload,
                &on_event,
                &task_acknowledgements,
                flow_control.unwrap_or(false).then_some(window),
            )
            .await
            {
                log::warn!("SSE 接收任务异常结束 session={task_id}");
                // 接收端关闭时任务也结束；报告失败不能再启动另一条连接。
                if let Err(error) = on_event.send(SseUpdate::Error { message }) {
                    log::info!(
                        "SSE 状态通道已关闭: {error_type}",
                        error_type = std::any::type_name_of_val(&error)
                    );
                }
            }
            log::info!("SSE 接收任务结束 session={task_id}");
            match app.state::<SseState>().0.lock() {
                Ok(mut tasks) => {
                    // 旧任务结束不得删除同 ID 后来登记的新连接。
                    if tasks.get(&task_id).is_some_and(|session| {
                        Arc::ptr_eq(&session.acknowledgements, &task_acknowledgements)
                    }) {
                        tasks.remove(&task_id);
                    }
                }
                Err(error) => log::error!(
                    "SSE 会话清理失败: {error_type}",
                    error_type = std::any::type_name_of_val(&error)
                ),
            };
        });
        tasks.insert(
            id,
            Session {
                task,
                acknowledgements,
            },
        );
        Ok(())
    })();
    match &result {
        Ok(_value) => log::info!(
            "接收请求已受理 operation=sse_start elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=sse_start elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 幂等取消，abort 使挂起的握手和读取也能被终止。
#[tauri::command]
pub fn sse_stop(state: State<'_, SseState>, id: String) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = (|| {
        if let Some(session) = state.0.lock().map_err(|e| e.to_string())?.remove(&id) {
            session.task.abort();
        }
        Ok(())
    })();
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=sse_stop elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=sse_stop elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 前端完成本批状态更新后累积确认；迟到的已关闭会话确认幂等忽略。
#[tauri::command]
pub fn sse_ack(state: State<'_, SseState>, id: String, sequence: u64) -> Result<(), String> {
    let sessions = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(session) = sessions.get(&id) {
        session.acknowledgements.acknowledge(sequence)?;
    }
    Ok(())
}

/// 工具与应用生命周期统一清理入口。
pub(super) fn close_all(state: &SseState) -> Result<(), String> {
    for (_, session) in state.0.lock().map_err(|e| e.to_string())?.drain() {
        session.task.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_chunks_preserve_utf8_crlf_and_event_metadata() {
        let mut parser = Parser::default();
        let mut events = Vec::new();
        let input = "\u{feff}:心跳\r\nid: 7\revent: delta\nretry: 2500\ndata: 中文\r\ndata: 第二行\r\n\r\ndata:\n\nid: bad\0id\nretry: -1\ndata: [DONE]\n\ndata: 未完成";
        for byte in input.as_bytes() {
            parser
                .push(&[*byte], |event| {
                    events.push(event);
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            SseEvent {
                event: "delta".into(),
                id: "7".into(),
                data: "中文\n第二行".into(),
                retry: Some(2500)
            }
        );
        assert_eq!(events[1].data, "");
        assert_eq!(events[2].id, "7");
        assert_eq!(events[2].event, "message");
        assert_eq!(events[2].data, "[DONE]");
    }
    #[test]
    fn oversized_line_fails_before_unbounded_growth() {
        assert!(Parser::default()
            .push(&vec![b'x'; 1024 * 1024 + 1], |_| Ok(()))
            .is_err());
    }

    #[test]
    fn batch_accounts_for_repeated_metadata_and_preserves_completed_events_before_error() {
        let mut parser = Parser {
            id: "x".repeat(128 * 1024),
            ..Parser::default()
        };
        let input = b"data: one\n\ndata: two\n\n";
        let first = parse_batch(&mut parser, input);
        assert_eq!(first.events.len(), 1);
        assert_eq!(first.events[0].data, "one");
        assert!(first.bytes > 128 * 1024);
        let second = parse_batch(&mut parser, &input[first.consumed..]);
        assert_eq!(second.events.len(), 1);
        assert_eq!(second.events[0].data, "two");
        let input = format!("data: complete\n\n{}", "x".repeat(1024 * 1024 + 1));
        let batch = parse_batch(&mut Parser::default(), input.as_bytes());
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].data, "complete");
        assert!(batch.error.is_some());
    }
}
