//! WebSocket 会话独立持有任务与有界消息；发送成功以实际写入 socket 为准。
use super::models::{RequestAuth, WsActionResult, WsMessage, WsSession};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

/// 发送命令附带完成通知，入队成功不冒充网络发送成功。
struct Outgoing {
    text: String,
    done: oneshot::Sender<Result<(), String>>,
}

/// 会话元数据与队列均由同一把短锁保护，锁不跨 await。
struct WsSessionHandle {
    url: String,
    connected_at: u64,
    open: bool,
    queue: VecDeque<WsMessage>,
    queue_bytes: usize,
    seq: u64,
    dropped: u64,
    error: Option<String>,
    tx: mpsc::Sender<Outgoing>,
    task: tauri::async_runtime::JoinHandle<()>,
}

/// 页签关闭时移除对应项；工具关闭时清空全部。
#[derive(Default)]
pub struct WsState(Mutex<HashMap<String, WsSessionHandle>>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl WsSessionHandle {
    fn push(&mut self, direction: &'static str, content: String) {
        self.seq += 1;
        self.queue_bytes += content.len();
        self.queue.push_back(WsMessage {
            seq: self.seq,
            direction,
            content,
            time: now_ms(),
        });
        while self.queue.len() > 500 || self.queue_bytes > 8 * 1024 * 1024 {
            if let Some(message) = self.queue.pop_front() {
                self.queue_bytes -= message.content.len();
            }
            self.dropped += 1;
        }
    }
    fn snapshot(&self, id: &str, after_seq: Option<u64>) -> WsSession {
        // 无游标或游标超前时返回完整保留窗口，供初始化和失步恢复。
        let after_seq = after_seq.filter(|seq| *seq <= self.seq);
        let start = after_seq.map_or(0, |seq| {
            self.queue.partition_point(|message| message.seq <= seq)
        });
        WsSession {
            id: id.into(),
            url: self.url.clone(),
            connected_at: self.connected_at,
            open: self.open,
            messages: self.queue.iter().skip(start).cloned().collect(),
            first_seq: self
                .queue
                .front()
                .map_or(self.seq.saturating_add(1), |message| message.seq),
            last_seq: self.seq,
            dropped: self.dropped,
            error: self.error.clone(),
        }
    }
}

/// 握手支持自定义请求头、统一认证与超时；UUID 避免同毫秒建连覆盖。
#[tauri::command]
pub async fn ws_connect(
    app: AppHandle,
    state: State<'_, WsState>,
    url: String,
    headers: Option<Vec<(String, String)>>,
    auth: Option<RequestAuth>,
    timeout_ms: Option<u64>,
) -> Result<WsSession, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let log_started = std::time::Instant::now();
    let result: Result<WsSession, String> = async {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "WebSocket 地址无效".to_string())?;
    if !matches!(parsed.scheme(), "ws" | "wss") {
        return Err("WebSocket 地址必须使用 WS 或 WSS".into());
    }
    let mut request = url
        .clone()
        .into_client_request()
        .map_err(|_| "WebSocket 地址无效".to_string())?;
    for (key, value) in headers.unwrap_or_default() {
        if key.trim().is_empty() {
            continue;
        }
        let key = key
            .parse::<tokio_tungstenite::tungstenite::http::HeaderName>()
            .map_err(|_| "请求头名称无效".to_string())?;
        let value = value
            .parse::<tokio_tungstenite::tungstenite::http::HeaderValue>()
            .map_err(|_| "请求头值无效".to_string())?;
        request.headers_mut().insert(key, value);
    }
    if let Some(value) = super::auth::authorization(&app, auth.as_ref())? {
        request.headers_mut().insert("authorization", value);
    }
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(15_000).clamp(1_000, 300_000));
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(1024 * 1024),
        max_frame_size: Some(1024 * 1024),
        ..Default::default()
    };
    let (socket, _) = tokio::time::timeout(
        timeout,
        tokio_tungstenite::connect_async_with_config(request, Some(config), false),
    )
    .await
    .map_err(|_| "WebSocket 连接超时".to_string())?
    .map_err(|e| e.to_string())?;
    let (mut write, mut read) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Outgoing>(16);
    let id = uuid::Uuid::new_v4().to_string();
    let task_id = id.clone();
    let mut map = state.0.lock().map_err(|e| e.to_string())?;
    let task = tauri::async_runtime::spawn(async move {
        let error = loop {
            tokio::select! {
                out = rx.recv() => {
                    let Some(out) = out else { break None; };
                    let content = out.text;
                    let result = tokio::time::timeout(Duration::from_secs(15), write.send(Message::Text(content.clone()))).await
                        .map_err(|_| "WebSocket 发送超时".to_string()).and_then(|result| result.map_err(|e| e.to_string()));
                    let error = result.as_ref().err().cloned();
                    if result.is_ok() {
                        match app.state::<WsState>().0.lock() {
                            Ok(mut map) => if let Some(handle) = map.get_mut(&task_id) { handle.push("sent", content); },
                            Err(error) => log::warn!("WebSocket 消息记录失败: {error_type}", error_type = std::any::type_name_of_val(&error)),
                        };
                    }
                    // 页签关闭后接收端可以消失，此时仍由任务清理 socket。
                    let _receiver_closed = out.done.send(result);
                    if error.is_some() { break error; }
                }
                frame = read.next() => {
                    let content = match frame {
                        Some(Ok(Message::Text(text))) => text,
                        Some(Ok(Message::Binary(bytes))) => format!("[二进制消息，{} 字节]\n{}", bytes.len(), bytes.iter().take(256).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")),
                        Some(Ok(Message::Close(_))) | None => break None,
                        Some(Err(error)) => break Some(error.to_string()),
                        Some(Ok(Message::Ping(_))) => { if let Err(error) = write.flush().await { break Some(error.to_string()); } continue; }
                        _ => continue,
                    };
                    match app.state::<WsState>().0.lock() {
                        Ok(mut map) => if let Some(handle) = map.get_mut(&task_id) { handle.push("received", content); },
                        Err(error) => break Some(format!("WebSocket 消息记录失败: {error}")),
                    };
                }
            }
        };
        if error.is_some() {
            log::warn!("WebSocket 连接异常结束 session={task_id}");
        } else {
            log::info!("WebSocket 连接结束 session={task_id}");
        }
        match app.state::<WsState>().0.lock() {
            Ok(mut map) => {
                if let Some(handle) = map.get_mut(&task_id) {
                    handle.open = false;
                    handle.error = error;
                }
            }
            Err(error) => log::error!("WebSocket 会话清理失败: {error_type}", error_type = std::any::type_name_of_val(&error)),
        };
    });
    let handle = WsSessionHandle {
        url,
        connected_at: now_ms(),
        open: true,
        queue: VecDeque::new(),
        queue_bytes: 0,
        seq: 0,
        dropped: 0,
        error: None,
        tx,
        task,
    };
    let snapshot = handle.snapshot(&id, None);
    map.insert(id, handle);
    Ok(snapshot)
    }.await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=ws_connect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=ws_connect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 有界发送并等待后台写入完成，关闭会话会使等待明确失败。
#[tauri::command]
pub async fn ws_send(
    state: State<'_, WsState>,
    id: String,
    message: String,
) -> Result<WsActionResult, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let log_started = std::time::Instant::now();
    let result: Result<WsActionResult, String> = async {
        if message.len() > 1024 * 1024 {
            return Err("单条消息不能超过 1 MiB".into());
        }
        let tx = {
            let map = state.0.lock().map_err(|e| e.to_string())?;
            map.get(&id)
                .filter(|h| h.open)
                .ok_or("会话不存在或已断开")?
                .tx
                .clone()
        };
        let (done, received) = oneshot::channel();
        tx.try_send(Outgoing {
            text: message,
            done,
        })
        .map_err(|_| "发送队列已满或连接已关闭".to_string())?;
        received
            .await
            .map_err(|_| "发送未完成，连接已关闭".to_string())??;
        Ok(WsActionResult {
            ok: true,
            message: None,
        })
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::debug!(
            "消息发送完成 elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!("消息未发送 code=ws.send_failed"),
        Err(_) => log::warn!(
            "操作未完成 operation=ws_send elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 按消息序号交付增量；缺省游标保留完整快照契约，读取不跨 await 持锁。
#[tauri::command]
pub async fn ws_recv(
    state: State<'_, WsState>,
    id: String,
    after_seq: Option<u64>,
) -> Result<WsSession, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&id)
        .map(|h| h.snapshot(&id, after_seq))
        .ok_or_else(|| "会话不存在".into())
}

/// 幂等关闭；直接取消任务使阻塞读写同时释放。
#[tauri::command]
pub async fn ws_close(state: State<'_, WsState>, id: String) -> Result<WsActionResult, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let log_started = std::time::Instant::now();
    let result: Result<WsActionResult, String> = async {
        if let Some(handle) = state.0.lock().map_err(|e| e.to_string())?.remove(&id) {
            handle.task.abort();
        }
        Ok(WsActionResult {
            ok: true,
            message: None,
        })
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=ws_close elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=ws_close elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 生命周期入口，不把锁故障当作清理成功。
pub(crate) fn close_all_sessions(state: &WsState) -> Result<(), String> {
    for (_, handle) in state.0.lock().map_err(|e| e.to_string())?.drain() {
        handle.task.abort();
    }
    Ok(())
}

/// 全部会话快照，仅返回本 owner 的运行状态。
#[tauri::command]
pub async fn ws_sessions(state: State<'_, WsState>) -> Result<Vec<WsSession>, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    Ok(state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|(id, h)| h.snapshot(id, None))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn incremental_snapshot_keeps_window_bounds_and_full_snapshot_fallback() {
        let (tx, _rx) = mpsc::channel(1);
        let task = tauri::async_runtime::spawn(async {});
        task.abort();
        let mut handle = WsSessionHandle {
            url: "ws://example.invalid".into(),
            connected_at: 1,
            open: true,
            queue: VecDeque::new(),
            queue_bytes: 0,
            seq: 0,
            dropped: 0,
            error: None,
            tx,
            task,
        };
        let empty = handle.snapshot("test", Some(0));
        assert_eq!((empty.first_seq, empty.last_seq), (1, 0));
        for index in 1..=501 {
            handle.push("received", format!("message {index}"));
        }
        let full = handle.snapshot("test", None);
        assert_eq!(full.messages.len(), 500);
        assert_eq!((full.first_seq, full.last_seq, full.dropped), (2, 501, 1));
        let incremental = handle.snapshot("test", Some(500));
        assert_eq!(incremental.messages.len(), 1);
        assert_eq!(incremental.messages[0].seq, 501);
        assert!(handle.snapshot("test", Some(501)).messages.is_empty());
        assert_eq!(handle.snapshot("test", Some(999)).messages.len(), 500);
        handle.open = false;
        handle.error = Some("closed".into());
        let closed = handle.snapshot("test", Some(501));
        assert!(!closed.open);
        assert_eq!(closed.error.as_deref(), Some("closed"));
        assert_eq!(closed.dropped, 1);
        assert!(closed.messages.is_empty());
    }
}
