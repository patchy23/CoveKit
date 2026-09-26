//! 长任务 SSH 通道：结构化输入、心跳和有界 JSON 消息解码。
use crate::plugins::ssh::{
    conn::{close_channel_writer, shell_quote, SshHandler},
    models::{ArchiveEvent, ArchiveRequest},
};
use russh::{client, ChannelMsg};
use std::{sync::Arc, time::Duration};
use tauri::ipc::Channel;
use tokio::sync::watch;

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// 既有消息上限在拼接与解析前生效；完整单块行直接借用，不复制或反复移动后续行。
fn decode_lines(
    pending: &mut Vec<u8>,
    data: &[u8],
    mut receive: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    for part in data.split_inclusive(|byte| *byte == b'\n') {
        let complete = part.last() == Some(&b'\n');
        let line = if complete {
            &part[..part.len() - 1]
        } else {
            part
        };
        if line.len() > MAX_MESSAGE_BYTES.saturating_sub(pending.len()) {
            return Err("归档消息超过大小上限".into());
        }
        if complete && pending.is_empty() {
            receive(line)?;
        } else {
            pending.extend_from_slice(line);
            if complete {
                receive(pending)?;
                pending.clear();
            }
        }
    }
    Ok(())
}

/// 执行任务，不使用短命令的整条超时；只有启动与取消确认设置期限。
pub(super) async fn execute(
    session: Arc<client::Handle<SshHandler>>,
    request: ArchiveRequest,
    progress: Channel<ArchiveEvent>,
    cancelled: watch::Receiver<bool>,
) -> Result<ArchiveEvent, String> {
    let mut input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if input.len() > MAX_MESSAGE_BYTES {
        return Err("归档请求超过 1 MiB".into());
    }
    input.push(b'\n');
    let channel = tokio::time::timeout(Duration::from_secs(15), session.channel_open_session())
        .await
        .map_err(|_| "打开归档通道超时")?
        .map_err(|e| e.to_string())?;
    let (mut reader, writer) = channel.split();
    let result = execute_channel(&mut reader, &writer, input, progress, cancelled).await;
    close_channel_writer(&writer).await;
    result
}

async fn execute_channel(
    reader: &mut russh::ChannelReadHalf,
    writer: &russh::ChannelWriteHalf<client::Msg>,
    input: Vec<u8>,
    progress: Channel<ArchiveEvent>,
    cancelled: watch::Receiver<bool>,
) -> Result<ArchiveEvent, String> {
    if *cancelled.borrow() {
        return Ok(super::cancelled_event());
    }
    // 两个借用 future 归当前调用持有，不创建脱离生命周期的发送任务或消息队列。
    let sending = send_input(writer, input, cancelled.clone());
    let deadline = cancellation_deadline(cancelled, Duration::from_secs(15));
    tokio::pin!(sending, deadline);
    let mut sending_done = false;
    let mut pending = Vec::new();
    let mut stderr = Vec::new();
    let mut outcome = None;
    let mut failure = None;
    loop {
        tokio::select! {
            _ = &mut deadline => {
                return Err("未收到远端停止确认，任务结果未知；请核对服务器临时目录".into());
            }
            result = &mut sending, if !sending_done => {
                result?;
                sending_done = true;
            }
            message = reader.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        decode_lines(&mut pending, &data, |line| {
                            let event: ArchiveEvent = serde_json::from_slice(line)
                                .map_err(|_| "归档工作器返回了无效消息")?;
                            if event.kind == "error" { failure = event.error.clone(); }
                            if event.kind == "result" { outcome = Some(event.clone()); }
                            // 视图事件接收者失效时退出通道，远端由 EOF/心跳超时收尾。
                            progress.send(event).map_err(|e| e.to_string())?;
                            Ok(())
                        })?;
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let room = 8192usize.saturating_sub(stderr.len());
                        stderr.extend(data.iter().take(room));
                    }
                    Some(ChannelMsg::Failure) => return Err("服务器拒绝归档命令".into()),
                    Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
        }
    }
    let mut result = outcome.ok_or_else(|| {
        let details = String::from_utf8_lossy(&stderr);
        if details.is_empty() {
            "归档通道关闭但没有完成确认，结果未知".to_string()
        } else {
            format!("无法执行归档任务，需要远端 Python 3.8+ 标准库：{details}")
        }
    })?;
    if let Some(error) = failure {
        result.error = Some(error);
        if result.status.as_deref() == Some("succeeded") {
            result.status = Some("failed".into());
        }
    }
    Ok(result)
}

/// 取消期限独立于所有 exec/data 写入，慢远端不会让期限停在发送分支内部。
async fn cancellation_deadline(mut cancelled: watch::Receiver<bool>, grace: Duration) {
    let _ = cancelled.wait_for(|value| *value).await;
    tokio::time::sleep(grace).await;
}

async fn send_input(
    writer: &russh::ChannelWriteHalf<client::Msg>,
    input: Vec<u8>,
    mut cancelled: watch::Receiver<bool>,
) -> Result<(), String> {
    let command = format!("python3 -u -c {}", shell_quote(include_str!("worker.py")));
    writer.exec(true, command).await.map_err(|e| format!("启动归档工作器失败：{e}"))?;
    writer.data_bytes(input).await.map_err(|e| e.to_string())?;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            biased;
            _ = cancelled.wait_for(|value| *value) => {
                writer.data_bytes(b"cancel\n".to_vec()).await.map_err(|e| e.to_string())?;
                return Ok(());
            }
            _ = heartbeat.tick() => {
                writer.data_bytes(b"ping\n".to_vec()).await
                    .map_err(|e| format!("归档连接中断，结果未知：{e}"))?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_deadline_remains_live_while_sender_is_stalled() {
        let (cancel, receiver) = watch::channel(false);
        let deadline = cancellation_deadline(receiver, Duration::ZERO);
        tokio::pin!(deadline);
        assert!(tokio::time::timeout(Duration::from_millis(1), &mut deadline).await.is_err());
        cancel.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                _ = std::future::pending::<()>() => panic!("模拟发送不会完成"),
                _ = &mut deadline => {}
            }
        }).await.unwrap();
    }

    #[test]
    fn line_decoder_preserves_every_utf8_split_and_multiple_lines() {
        let input = "{\"name\":\"中文🙂\"}\r\n{\"name\":\"next\"}\n".as_bytes();
        for split in 0..=input.len() {
            let mut pending = Vec::new();
            let mut names = Vec::new();
            for chunk in [&input[..split], &input[split..]] {
                decode_lines(&mut pending, chunk, |line| {
                    let value: serde_json::Value = serde_json::from_slice(line).unwrap();
                    names.push(value["name"].as_str().unwrap().to_string());
                    Ok(())
                })
                .unwrap();
            }
            assert_eq!(names, ["中文🙂", "next"]);
            assert!(pending.is_empty());
        }
    }

    #[test]
    fn oversized_complete_and_partial_lines_are_rejected_before_parse_or_append() {
        for complete in [false, true] {
            let mut input = vec![b'x'; MAX_MESSAGE_BYTES + 1];
            if complete {
                input.push(b'\n');
            }
            let mut pending = Vec::new();
            let result = decode_lines(&mut pending, &input, |_| panic!("超限消息不得交给解析器"));
            assert!(result.is_err());
            assert!(pending.is_empty());
        }
        let mut pending = vec![b'x'; MAX_MESSAGE_BYTES];
        assert!(decode_lines(&mut pending, b"y\n", |_| {
            panic!("超限消息不得交给解析器")
        })
        .is_err());
        assert_eq!(pending.len(), MAX_MESSAGE_BYTES);
    }

    #[test]
    fn message_limit_is_per_line_and_receiver_failure_stops_following_lines() {
        let mut line = vec![b'x'; MAX_MESSAGE_BYTES];
        line.push(b'\n');
        let mut input = line.clone();
        input.extend_from_slice(&line);
        let mut pending = Vec::new();
        let mut count = 0;
        decode_lines(&mut pending, &input, |message| {
            assert_eq!(message.len(), MAX_MESSAGE_BYTES);
            count += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(count, 2);
        count = 0;
        assert!(decode_lines(&mut pending, b"one\ntwo\n", |_| {
            count += 1;
            Err("接收者已关闭".into())
        })
        .is_err());
        assert_eq!(count, 1);
    }
}
