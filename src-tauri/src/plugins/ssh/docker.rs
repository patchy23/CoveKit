//! SSH 插件 · Docker 容器管理
//! docker 命令输出解析为结构化容器列表；解析函数为纯函数（可单测）。
//! docker exec 复用 PTY 通道（交互式容器终端）。

use russh::ChannelMsg;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::plugins::ssh::conn::{exec_collect, now_ms, SshState};
use crate::plugins::ssh::models::{
    DockerContainer, SshActionResult, TerminalData, TerminalSession,
};
use crate::plugins::ssh::terminal::TerminalState;

/// 解析 docker ps 行（纯函数）：tab 分隔 ID/NAME/IMAGE/STATUS/PORTS → DockerContainer
/// （--format 用 tab 分隔，STATUS 含空格也不受影响）
fn parse_container_line(line: &str) -> Option<DockerContainer> {
    let parts: Vec<&str> = line.split('\x09').collect();
    if parts.len() < 4 {
        return None;
    }
    Some(DockerContainer {
        id: parts[0].to_string(),
        name: parts[1].to_string(),
        image: parts[2].to_string(),
        status: parts[3].to_string(),
        ports: parts.get(4).map(|s| s.to_string()).unwrap_or_default(),
        created_at: 0,
    })
}

/// 容器列表
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
) -> Result<Vec<DockerContainer>, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let out = exec_collect(
        &session,
        "docker ps -a --no-trunc --format '{{.ID}}\x09{{.Names}}\x09{{.Image}}\x09{{.Status}}\x09{{.Ports}}' 2>&1",
    )
    .await?;
    if out.contains("Cannot connect to the Docker daemon") {
        return Err("Docker 未运行或当前用户无权限".into());
    }
    let mut containers: Vec<DockerContainer> =
        out.lines().filter_map(parse_container_line).collect();
    containers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(containers)
}

/// 容器操作（start/stop/restart/remove）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_action(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    container_id: String,
    action: String,
) -> Result<SshActionResult, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let action = match action.as_str() {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "remove" => "rm -f",
        _ => return Err(format!("不支持的操作: {action}")),
    };
    let out = exec_collect(&session, &format!("docker {action} {container_id} 2>&1")).await?;
    Ok(SshActionResult {
        ok: out.trim().is_empty() || !out.contains("Error"),
        error: if out.trim().is_empty() {
            None
        } else {
            Some(out.trim().to_string())
        },
    })
}

/// 容器日志（最近 N 行）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_logs(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    container_id: String,
    lines: Option<u32>,
) -> Result<serde_json::Value, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let n = lines.unwrap_or(100);
    let out = exec_collect(
        &session,
        &format!("docker logs --tail {n} {container_id} 2>&1"),
    )
    .await?;
    Ok(serde_json::json!({ "ok": true, "logs": out }))
}

/// 进入容器终端（PTY exec；复用终端事件通道）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_exec(
    app: AppHandle,
    state: State<'_, TerminalState>,
    ssh_state: State<'_, SshState>,
    connection_id: String,
    container_id: String,
    cols: u32,
    rows: u32,
) -> Result<TerminalSession, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;

    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_pty(false, "xterm", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY 请求失败: {e}"))?;
    channel
        .exec(false, format!("docker exec -it {container_id} /bin/sh"))
        .await
        .map_err(|e| format!("进入容器失败: {e}"))?;

    let terminal_id = format!("term-{}", now_ms());
    let (tx, mut rx) = mpsc::unbounded_channel::<crate::plugins::ssh::terminal::TerminalCmd>();

    state.0.lock().map_err(|e| e.to_string())?.insert(
        terminal_id.clone(),
        crate::plugins::ssh::terminal::TerminalHandle {
            connection_id: connection_id.clone(),
            title: format!("docker:{container_id}"),
            cols,
            rows,
            active: true,
            tx,
        },
    );

    // 后台读写任务（与 terminal.rs 同构）
    let app2 = app.clone();
    let task_id = terminal_id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(crate::plugins::ssh::terminal::TerminalCmd::Write(data)) => {
                            if channel.data_bytes(data).await.is_err() { break; }
                        }
                        Some(crate::plugins::ssh::terminal::TerminalCmd::Resize(c, r)) => {
                            let _ = channel.window_change(c, r, 0, 0).await;
                        }
                        Some(crate::plugins::ssh::terminal::TerminalCmd::Close) | None => break,
                    }
                }
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { data }) => {
                            let payload = TerminalData {
                                terminal_id: task_id.clone(),
                                data: String::from_utf8_lossy(&data).to_string(),
                                time: now_ms(),
                            };
                            let _ = app2.emit("ssh://terminal-data", &payload);
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                        _ => {}
                    }
                }
            }
        }
        if let Ok(mut m) = app2.state::<TerminalState>().0.lock() {
            m.remove(&task_id);
        }
    });

    Ok(TerminalSession {
        id: terminal_id,
        connection_id,
        title: format!("docker:{container_id}"),
        cols: cols as u16,
        rows: rows as u16,
        active: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析容器行() {
        let c = parse_container_line(
            "abc123def456\x09mynginx\x09nginx:latest\x09Up 2 hours\x090.0.0.0:80->80/tcp",
        )
        .unwrap();
        assert_eq!(c.id, "abc123def456");
        assert_eq!(c.name, "mynginx");
        assert_eq!(c.image, "nginx:latest");
        assert_eq!(c.status, "Up 2 hours");
        assert_eq!(c.ports, "0.0.0.0:80->80/tcp");
    }

    #[test]
    fn 忽略空行() {
        assert!(parse_container_line("").is_none());
        assert!(parse_container_line("CONTAINER ID IMAGE COMMAND").is_none());
    }
}
