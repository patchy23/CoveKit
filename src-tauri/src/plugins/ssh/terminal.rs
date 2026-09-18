//! SSH 插件 · 终端通道（PTY shell）
//! 每个终端 = 一个 SSH 通道（连接复用，多通道并行）；
//! 后台任务 select 双路：前端指令（write/resize/close）与通道输出（wait()），
//! 通道输出经事件 ssh://terminal-data 推送（ANSI 原样，xterm.js 渲染）；
//! 开启会话日志时，同一块输出会旁路剥离 ANSI 后落盘（见 log.rs）。

use std::{collections::HashMap, sync::Mutex};

use russh::{client, ChannelMsg};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, watch};

use crate::plugins::ssh::conn::{now_ms, resource_id, SshState};
use crate::plugins::ssh::log::{self, SharedLog};
use crate::plugins::ssh::models::{SshActionResult, TerminalClosed, TerminalData, TerminalSession};

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
    /// 会话日志写入器（未录制时为 None；后台任务与命令层共享同一份）
    pub(crate) log: SharedLog,
}

/// 终端注册表（State 注入）
pub struct TerminalState(pub Mutex<HashMap<String, TerminalHandle>>);

/// 增量 UTF-8 解码器：SSH 通道分块不保证字符边界，跨块的多字节字符（中文/emoji）
/// 若逐块 from_utf8_lossy 会在块边界两侧各产生一个 U+FFFD 乱码，且不可恢复。
/// 尾部不完整字节留存待下一块补齐；真正的无效字节输出替换符并丢弃（防缓冲区卡死）。
#[derive(Default)]
struct Utf8ChunkDecoder {
    /// 暂存的尾部不完整字节（正常至多 3 字节）
    pending: Vec<u8>,
}

impl Utf8ChunkDecoder {
    /// 喂入一块通道数据，返回当前可完整解码的文本（可能为空串——等下一块补齐）
    fn feed(&mut self, data: &[u8]) -> String {
        self.pending.extend_from_slice(data);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // valid_up_to 之前必为合法 UTF-8（str::from_utf8 契约）
                    if let Ok(s) = std::str::from_utf8(&self.pending[..valid]) {
                        out.push_str(s);
                    }
                    match e.error_len() {
                        // 真无效字节：替换符 + 丢弃后继续解码剩余部分
                        Some(len) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..valid + len);
                        }
                        // 尾部不完整的多字节字符：留存等下一块
                        None => {
                            self.pending.drain(..valid);
                            break;
                        }
                    }
                }
            }
        }
        out
    }

    /// 收尾冲刷残留字节（正常连接不会有；坏字节按替换符输出，不丢失尾部文本）
    fn finish(&mut self) -> String {
        let tail = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        tail
    }
}

/// 启动终端通道后台任务（terminal.rs 与 docker.rs 共用）：
/// select 双路——取消信号 + 前端指令（write/resize/close）与通道输出（wait → emit terminal-data）；
/// 任务退出时从 TerminalState 注册表移除并推送 terminal-closed。
pub(crate) fn spawn_channel_task(
    app: AppHandle,
    terminal_id: String,
    connection_id: String,
    mut channel: russh::Channel<client::Msg>,
    mut rx: mpsc::Receiver<TerminalCmd>,
    mut cancel_rx: watch::Receiver<bool>,
    log: SharedLog,
) {
    tauri::async_runtime::spawn(async move {
        let mut decoder = Utf8ChunkDecoder::default();
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
                            let text = decoder.feed(&data);
                            // 文本为空说明整块都是某字符的前半截，等下一块即可，不发空事件
                            if !text.is_empty() {
                                let payload = TerminalData {
                                    terminal_id: terminal_id.clone(),
                                    connection_id: connection_id.clone(),
                                    data: text,
                                    time: now_ms(),
                                };
                                let _ = app.emit("ssh://terminal-data", &payload);
                            }
                            // 旁路写盘；失败则停录并通知前端（不静默）
                            if let Err(message) = log::append(&log, &data).await {
                                log::finish(&log).await;
                                let _ = app.emit(
                                    "ssh://terminal-log-error",
                                    &log::error_payload(&terminal_id, message),
                                );
                            }
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                        _ => {}
                    }
                }
            }
        }
        // 收尾：冲刷解码器残留（连接中断时尾部不完整字节按替换符输出，不丢文本）
        let tail = decoder.finish();
        if !tail.is_empty() {
            let _ = app.emit(
                "ssh://terminal-data",
                &TerminalData {
                    terminal_id: terminal_id.clone(),
                    connection_id: connection_id.clone(),
                    data: tail,
                    time: now_ms(),
                },
            );
        }
        // 任务退出：先收尾日志（flush 落盘），再从注册表移除
        log::finish(&log).await;
        if let Ok(mut m) = app.state::<TerminalState>().0.lock() {
            m.remove(&terminal_id);
        }
        let _ = app.emit(
            "ssh://terminal-closed",
            &TerminalClosed {
                terminal_id: terminal_id.clone(),
                connection_id: connection_id.clone(),
            },
        );
    });
}

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
    // 从连接注册表取会话（Handle 是 Clone，取出供通道开启，不持锁跨 await）
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;

    // 开通道 + PTY + shell（xterm 终端类型）
    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(e) => {
            crate::plugins::ssh::conn::mark_session_closed(
                ssh_state.inner(),
                &connection_id,
                &session,
            );
            return Err(format!("打开通道失败: {e}"));
        }
    };
    channel
        .request_pty(false, "xterm", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY 请求失败: {e}"))?;
    channel
        .request_shell(false)
        .await
        .map_err(|e| format!("shell 启动失败: {e}"))?;

    let terminal_id = resource_id("term");
    let (tx, rx) = mpsc::channel::<TerminalCmd>(128);
    let (cancel, cancel_rx) = watch::channel(false);
    // 会话日志共享状态（默认未录制；命令层开启后由后台任务写盘）
    let log = log::new_shared();

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
            log: log.clone(),
        },
    );

    spawn_channel_task(
        app.clone(),
        terminal_id.clone(),
        connection_id.clone(),
        channel,
        rx,
        cancel_rx,
        log,
    );

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

#[cfg(test)]
mod tests {
    use super::Utf8ChunkDecoder;

    #[test]
    fn 跨块的多字节字符不乱码() {
        // 「中」= E4 B8 AD；按 2+1 字节拆块，逐块 lossy 解码必出两个 U+FFFD
        let bytes = "a中b".as_bytes();
        let cut = 3; // 'a' + 中的前两字节
        let mut decoder = Utf8ChunkDecoder::default();
        let first = decoder.feed(&bytes[..cut]);
        let second = decoder.feed(&bytes[cut..]);
        assert_eq!(first, "a");
        assert_eq!(second, "中b");
        assert!(decoder.finish().is_empty());
    }

    #[test]
    fn 无效字节输出替换符且不卡死() {
        let mut decoder = Utf8ChunkDecoder::default();
        // 0xFF 是非法 UTF-8 起始字节：输出替换符并丢弃，后续文本正常
        let out = decoder.feed(&[0x68, 0xFF, 0x69]);
        assert_eq!(out, "h\u{FFFD}i");
        assert!(decoder.finish().is_empty());
    }

    #[test]
    fn 收尾冲刷不完整尾部() {
        let mut decoder = Utf8ChunkDecoder::default();
        let bytes = "中".as_bytes();
        assert_eq!(decoder.feed(&bytes[..2]), "");
        // 连接中断：残留半截字符按替换符输出，不丢也不等
        assert_eq!(decoder.finish(), "\u{FFFD}");
    }
}
