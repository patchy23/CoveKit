//! SSH 插件 · 终端通道（PTY shell）
//! 每个终端 = 一个 SSH 通道（连接复用，多通道并行）；
//! 后台任务 select 双路：前端指令（write/resize/close）与通道输出（wait()），
//! 通道输出经事件 ssh://terminal-data 推送（ANSI 原样，xterm.js 渲染）。

use std::{collections::HashMap, sync::Mutex};

use russh::ChannelMsg;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, watch};

use crate::plugins::ssh::conn::{now_ms, resource_id, SshState};
use crate::plugins::ssh::models::{SshActionResult, TerminalData, TerminalSession};

/// 终端指令（前端 → 后台任务）
pub(crate) enum TerminalCmd {
    /// 写入数据（键盘输入/粘贴）
    Write(Vec<u8>),
    /// 调整窗口大小（xterm fit 后同步）
    Resize(u32, u32),
}

/// 终端句柄（注册表存发送端；channel 归属后台任务）
pub(crate) struct TerminalHandle {
    /// 所属连接 session_id
    pub(crate) connection_id: String,
    /// 终端标题（默认服务器名）
    pub(crate) title: String,
    /// 当前列数
    pub(crate) cols: u32,
    /// 当前行数
    pub(crate) rows: u32,
    /// 是否活跃（前端正在展示）
    pub(crate) active: bool,
    /// 指令通道（drop 后任务收到 None → 关闭）
    pub(crate) tx: mpsc::Sender<TerminalCmd>,
    /// 独立取消信号，不受已满的数据队列阻塞。
    pub(crate) cancel: watch::Sender<bool>,
}

/// 终端注册表（State 注入）
pub struct TerminalState(pub Mutex<HashMap<String, TerminalHandle>>);

/// 打开终端：在连接上开 PTY shell 通道并启动后台读写任务
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_open(
    app: AppHandle,
    state: State<'_, TerminalState>,
    ssh_state: State<'_, SshState>,
    connection_id: String,
    cols: u32,
    rows: u32,
) -> Result<TerminalSession, String> {
    if cols == 0 || rows == 0 || cols > u16::MAX as u32 || rows > u16::MAX as u32 {
        return Err("终端行列数必须在 1..=65535 范围内".into());
    }
    // 从连接注册表取会话
    let handle = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| {
            // Handle 是 Clone，取出供通道开启（不持锁跨 await）
            (h.session.clone(), h.host.clone(), h.profile_id.clone())
        })
        .ok_or("连接不存在或已断开")?;
    let (session, _host, _profile_id) = handle;

    // 开通道 + PTY + shell（xterm 终端类型）
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_pty(false, "xterm", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY 请求失败: {e}"))?;
    channel
        .request_shell(false)
        .await
        .map_err(|e| format!("shell 启动失败: {e}"))?;

    let terminal_id = resource_id("term");
    let (tx, mut rx) = mpsc::channel::<TerminalCmd>(128);
    let (cancel, mut cancel_rx) = watch::channel(false);

    // 登记句柄（先插入，任务退出时移除）
    state.0.lock().map_err(|e| e.to_string())?.insert(
        terminal_id.clone(),
        TerminalHandle {
            connection_id: connection_id.clone(),
            title: String::new(),
            cols,
            rows,
            active: true,
            tx,
            cancel,
        },
    );

    // 后台读写任务：指令下发 + 输出推送
    let app2 = app.clone();
    let task_id = terminal_id.clone();
    let title_for_task = task_id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    if changed.is_err() || *cancel_rx.borrow() { break; }
                }
                // 前端指令
                cmd = rx.recv() => {
                    match cmd {
                        Some(TerminalCmd::Write(data)) => {
                            if channel.data_bytes(data).await.is_err() { break; }
                        }
                        Some(TerminalCmd::Resize(c, r)) => {
                            let _ = channel.window_change(c, r, 0, 0).await;
                        }
                        None => break,
                    }
                }
                // 通道输出（服务端推送）
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
        // 任务退出：标记非活跃并从注册表移除
        if let Ok(mut m) = app2.state::<TerminalState>().0.lock() {
            m.remove(&task_id);
        }
        let _ = title_for_task;
    });

    Ok(TerminalSession {
        id: terminal_id,
        connection_id,
        title: String::new(),
        cols: cols as u16,
        rows: rows as u16,
        active: true,
    })
}

/// 写入终端数据（键盘输入）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_write(
    state: State<'_, TerminalState>,
    terminal_id: String,
    data: String,
) -> Result<SshActionResult, String> {
    let tx = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&terminal_id)
        .map(|h| h.tx.clone())
        .ok_or("终端不存在或已关闭")?;
    tx.send(TerminalCmd::Write(data.into_bytes()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

/// 调整终端窗口大小（xterm fit 时调用）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_resize(
    state: State<'_, TerminalState>,
    terminal_id: String,
    cols: u32,
    rows: u32,
) -> Result<SshActionResult, String> {
    if cols == 0 || rows == 0 || cols > u16::MAX as u32 || rows > u16::MAX as u32 {
        return Err("终端行列数必须在 1..=65535 范围内".into());
    }
    let tx = {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        let h = map.get_mut(&terminal_id).ok_or("终端不存在或已关闭")?;
        h.cols = cols;
        h.rows = rows;
        h.tx.clone()
    };
    tx.send(TerminalCmd::Resize(cols, rows))
        .await
        .map_err(|e| e.to_string())?;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

/// 关闭终端（独立取消信号 → 后台任务退出并清理）
#[tauri::command]
pub async fn ssh_terminal_close(
    state: State<'_, TerminalState>,
    terminal_id: String,
) -> Result<SshActionResult, String> {
    let cancel = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&terminal_id)
        .map(|h| h.cancel);
    if let Some(cancel) = cancel {
        let _ = cancel.send(true);
    }
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

/// 某连接下的全部终端会话
#[tauri::command]
pub async fn ssh_terminal_list(
    state: State<'_, TerminalState>,
    connection_id: String,
) -> Result<Vec<TerminalSession>, String> {
    let map = state.0.lock().map_err(|e| e.to_string())?;
    Ok(map
        .iter()
        .filter(|(_, h)| h.connection_id == connection_id)
        .map(|(id, h)| TerminalSession {
            id: id.clone(),
            connection_id: h.connection_id.clone(),
            title: h.title.clone(),
            cols: h.cols as u16,
            rows: h.rows as u16,
            active: h.active,
        })
        .collect())
}
