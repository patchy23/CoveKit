//! 长任务 SSH 通道：结构化输入、心跳和有界 JSON 消息解码。
use crate::plugins::ssh::{
    conn::{shell_quote, SshHandler},
    models::{ArchiveEvent, ArchiveRequest},
};
use russh::{client, ChannelMsg};
use std::{sync::Arc, time::Duration};
use tauri::ipc::Channel;
use tokio::sync::watch;

/// 执行任务，不使用短命令的整条超时；只有启动与取消确认设置期限。
pub(super) async fn execute(
    session: Arc<client::Handle<SshHandler>>,
    request: ArchiveRequest,
    progress: Channel<ArchiveEvent>,
    mut cancelled: watch::Receiver<bool>,
) -> Result<ArchiveEvent, String> {
    let mut channel = tokio::time::timeout(Duration::from_secs(15), session.channel_open_session())
        .await
        .map_err(|_| "打开归档通道超时")?
        .map_err(|e| e.to_string())?;
    let command = format!("python3 -u -c {}", shell_quote(include_str!("worker.py")));
    channel
        .exec(true, command)
        .await
        .map_err(|e| format!("启动归档工作器失败：{e}"))?;
    let mut input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if input.len() > 1024 * 1024 {
        return Err("归档请求超过 1 MiB".into());
    }
    input.push(b'\n');
    channel.data_bytes(input).await.map_err(|e| e.to_string())?;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    let mut pending = Vec::new();
    let mut stderr = Vec::new();
    let mut outcome = None;
    let mut failure = None;
    let mut cancel_started = None;
    loop {
        tokio::select! {
            _ = cancelled.changed(), if cancel_started.is_none() => {
                channel.data_bytes(b"cancel\n".to_vec()).await.map_err(|e| e.to_string())?;
                cancel_started = Some(std::time::Instant::now());
            }
            _ = heartbeat.tick() => {
                if cancel_started.is_none() && *cancelled.borrow() {
                    channel.data_bytes(b"cancel\n".to_vec()).await.map_err(|e| e.to_string())?;
                    cancel_started = Some(std::time::Instant::now());
                }
                if cancel_started.is_some_and(|start| start.elapsed() > Duration::from_secs(15)) {
                    let _ = channel.eof().await;
                    return Err("未收到远端停止确认，任务结果未知；请核对服务器临时目录".into());
                }
                channel.data_bytes(b"ping\n".to_vec()).await.map_err(|e| format!("归档连接中断，结果未知：{e}"))?;
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        pending.extend_from_slice(&data);
                        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = pending.drain(..=end).collect();
                            let event: ArchiveEvent = serde_json::from_slice(&line)
                                .map_err(|_| "归档工作器返回了无效消息")?;
                            if event.kind == "error" { failure = event.error.clone(); }
                            if event.kind == "result" { outcome = Some(event.clone()); }
                            // 视图事件接收者失效时退出通道，远端由 EOF/心跳超时收尾。
                            progress.send(event).map_err(|e| e.to_string())?;
                        }
                        if pending.len() > 1024 * 1024 { return Err("归档消息超过大小上限".into()); }
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
