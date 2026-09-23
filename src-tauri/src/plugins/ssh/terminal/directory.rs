//! 终端 shell 的身份与当前目录；不向交互输入注入命令。
use super::TerminalState;
use crate::plugins::ssh::conn::{get_sftp_session, shell_quote, SshState};
use russh::{client, Channel, ChannelMsg};
use tauri::State;
use tokio::io::AsyncReadExt;

/// 在登录 shell 启动前登记 PID；exec 保留该进程身份和 PTY，不修改远端配置。
pub(super) async fn start_shell(
    channel: &mut Channel<client::Msg>,
    terminal_id: &str,
) -> Result<(u32, Vec<u8>), String> {
    let prefix = format!("\x1b]777;{terminal_id};");
    let script = format!(
        "printf '\\033]777;{terminal_id};%s\\007' \"$$\"; exec env COVEKIT_TERMINAL_ID={} \"\u{24}{{SHELL:-/bin/sh}}\" -l",
        shell_quote(terminal_id)
    );
    channel
        .exec(true, format!("/bin/sh -c {}", shell_quote(&script)))
        .await
        .map_err(|e| format!("登录 shell 启动失败：{e}"))?;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut output = Vec::new();
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    output.extend_from_slice(&data);
                    if output.len() > 65536 {
                        return Err("shell 启动输出过大".to_string());
                    }
                    if let Some(start) = output
                        .windows(prefix.len())
                        .position(|w| w == prefix.as_bytes())
                    {
                        let from = start + prefix.len();
                        if let Some(end) = output[from..].iter().position(|b| *b == 7) {
                            let pid = std::str::from_utf8(&output[from..from + end])
                                .ok()
                                .and_then(|v| v.parse::<u32>().ok())
                                .filter(|v| *v > 1)
                                .ok_or("无法登记终端进程")?;
                            output.drain(start..from + end + 1);
                            return Ok((pid, output));
                        }
                    }
                }
                Some(ChannelMsg::Failure) | Some(ChannelMsg::Close) | None => {
                    return Err("服务器拒绝启动登录 shell".into())
                }
                _ => {}
            }
        }
    })
    .await
    .map_err(|_| "等待终端进程登记超时".to_string())?
}

/// Linux 通过受会话标记保护的 /proc 读取 shell cwd；不可用时明确失败供前端采用 OSC 报告。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_directory(
    state: State<'_, TerminalState>,
    ssh_state: State<'_, SshState>,
    terminal_id: String,
) -> Result<String, String> {
    let (connection_id, pid) = {
        let registry = state.0.lock().map_err(|e| e.to_string())?;
        let terminal = registry.get(&terminal_id).ok_or("终端已关闭")?;
        (
            terminal.connection_id.clone(),
            terminal.shell_pid.ok_or("此终端不支持主机目录查询")?,
        )
    };
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    let file = sftp
        .open(format!("/proc/{pid}/environ"))
        .await
        .map_err(|_| "无法读取终端目录：服务器不提供可访问的进程目录信息")?;
    let mut environment = Vec::new();
    file.take(1024 * 1024)
        .read_to_end(&mut environment)
        .await
        .map_err(|e| format!("无法确认终端进程：{e}"))?;
    let marker = format!("COVEKIT_TERMINAL_ID={terminal_id}");
    if !environment
        .split(|b| *b == 0)
        .any(|v| v == marker.as_bytes())
    {
        return Err("终端进程身份已变化，无法可靠读取目录".into());
    }
    let path = sftp
        .canonicalize(format!("/proc/{pid}/cwd"))
        .await
        .map_err(|e| format!("读取终端目录失败：{e}"))?;
    if !path.starts_with('/') {
        return Err("终端返回了非绝对目录".into());
    }
    Ok(path)
}
