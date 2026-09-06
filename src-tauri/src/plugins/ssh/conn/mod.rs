//! SSH 插件 · 连接会话注册表（russh 客户端）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>),
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。
//! 连接流程：分阶段事件（ssh://connect-stage）+ 稳定错误码（SshConnectError）；
//! 主机密钥校验在握手回调中等待前端人工确认（ssh://host-key-verify ↔ ssh_host_key_respond），
//! 首连不再静默记录 TOFU，指纹变更必须经用户明确决定（仅本次 / 保存 / 替换 / 取消）。

//! 模块划分：handler = russh Handler（主机密钥交互）；connect = 建连与凭证解析；
//! reconnect = 重连/断开/快照命令；本文件保留会话注册表、共享工具与主机密钥管理命令。

pub(crate) mod connect;
pub(crate) mod handler;
pub(crate) mod reconnect;

pub(crate) use handler::SshHandler;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use russh::{client, ChannelMsg};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{
    ConnectionStatus, HostKeyDecision, KnownHostEntry, ServerConnection, ServerProfile,
    SshActionResult, SshConnectError,
};
use crate::plugins::ssh::tunnel::ForwardTargets;

/// 主机密钥确认等待时长（超时视为用户放弃，连接失败）
const HOST_KEY_WAIT: Duration = Duration::from_secs(180);
/// DNS 解析 / TCP 建连超时
const CONNECT_PHASE_TIMEOUT: Duration = Duration::from_secs(10);

/// 主机密钥确认应答注册表：requestId → oneshot 发送端（前端 respond 后触发握手回调继续）
pub struct HostKeyState(pub Mutex<HashMap<String, oneshot::Sender<HostKeyDecision>>>);

/// SSH 会话句柄（存注册表；russh Handle 为 Clone+Send，可安全跨任务使用）
pub(crate) struct SshSessionHandle {
    /// 关联的服务器配置 id
    pub(crate) profile_id: String,
    /// 主机地址（快照展示用）
    pub(crate) host: String,
    /// 是否处于连接状态
    pub(crate) open: bool,
    /// 建立连接的毫秒时间戳
    pub(crate) connected_at: u64,
    /// russh 会话句柄（终端/文件通道从此开启）
    pub(crate) session: std::sync::Arc<client::Handle<SshHandler>>,
    /// SFTP 长驻会话（惰性创建 + 全连接期复用，句柄销毁时随之回收）。
    /// 每次新建需 channel open + 子系统握手（约 2~3 次 RTT）——
    /// 逐操作新建是文件页签切目录卡顿与内存飙升的根因；
    /// SftpSession 设计为长生命周期且支持并发请求，标准做法即每连接复用一个。
    pub(crate) sftp: Mutex<Option<Arc<russh_sftp::client::SftpSession>>>,
    /// -R 远程转发目标表（与 handler 共享；tunnel 模块注册/注销）
    pub(crate) forward_targets: ForwardTargets,
}

/// SSH 会话注册表（State 注入，惰性初始化）
pub struct SshState(pub Mutex<HashMap<String, SshSessionHandle>>);

/// 当前毫秒时间戳
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 生成进程内单调唯一的资源 id，避免同一毫秒并发创建时互相覆盖。
pub(crate) fn resource_id(prefix: &str) -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    format!(
        "{prefix}-{}-{}",
        now_ms(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

/// POSIX shell 单引号转义；所有拼入远程命令的字符串参数必须先经过此函数。
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// 从连接注册表取会话句柄（各命令通用样板，消除重复）
/// 返回 Arc 句柄；连接不存在或已断开时返回错误。
pub(crate) fn get_session(
    state: &tauri::State<'_, SshState>,
    connection_id: &str,
) -> Result<Arc<client::Handle<SshHandler>>, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(connection_id)
        .map(|h| h.session.clone())
        .ok_or_else(|| "连接不存在或已断开".to_string())
}

/// 取该连接的 SFTP 长驻会话（惰性创建，之后复用；并发首访时后者覆盖前者，
/// 被覆盖的会话随 Arc 释放自动关通道，无害）。文件浏览/编辑等高频操作走此入口；
/// 大文件传输仍用独立通道（不占用交互会话的请求窗口）。
pub(crate) async fn get_sftp_session(
    state: &tauri::State<'_, SshState>,
    connection_id: &str,
) -> Result<Arc<russh_sftp::client::SftpSession>, String> {
    // 先查缓存（两把锁都只在小作用域内持有，不跨 await）
    let cached = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        let handle = map.get(connection_id).ok_or("连接不存在或已断开")?;
        // 先落局部变量再作为块尾值：避免 MutexGuard 临时量的析构顺序借用问题
        let cached_slot = handle.sftp.lock().map_err(|e| e.to_string())?.clone();
        cached_slot
    };
    if let Some(sftp) = cached {
        return Ok(sftp);
    }
    // 缓存未命中：新建 channel + SFTP 子系统握手
    let session = get_session(state, connection_id)?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_subsystem(false, "sftp")
        .await
        .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
    let stream = channel.into_stream();
    let sftp = Arc::new(
        russh_sftp::client::SftpSession::new(stream)
            .await
            .map_err(|e| format!("SFTP 初始化失败: {e}"))?,
    );
    // 回存（连接可能已断开，存不进去就直接返回新建的这个）
    if let Ok(map) = state.0.lock() {
        if let Some(handle) = map.get(connection_id) {
            if let Ok(mut slot) = handle.sftp.lock() {
                *slot = Some(sftp.clone());
            }
        }
    }
    Ok(sftp)
}

/// 使缓存的 SFTP 会话失效（操作报通道/协议错误时调用，下次操作自动重建）
pub(crate) fn invalidate_sftp_session(state: &tauri::State<'_, SshState>, connection_id: &str) {
    if let Ok(map) = state.0.lock() {
        if let Some(handle) = map.get(connection_id) {
            if let Ok(mut slot) = handle.sftp.lock() {
                *slot = None;
            }
        }
    }
}

/// 执行远程命令并收集全部输出（监控/服务/进程/Docker 共用）
/// 非交互 exec：开通道 → exec → 循环 wait() 收 Data 直到 Eof/Close
pub(crate) async fn exec_collect(
    session: &client::Handle<SshHandler>,
    command: &str,
) -> Result<String, String> {
    const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .exec(false, command)
        .await
        .map_err(|e| format!("执行失败: {e}"))?;
    let mut out = String::new();
    let mut exit_status = None;
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { data } => {
                if out.len().saturating_add(data.len()) > MAX_OUTPUT_BYTES {
                    return Err("远程命令输出超过 16 MiB 安全上限".into());
                }
                out.push_str(&String::from_utf8_lossy(&data));
            }
            ChannelMsg::ExtendedData { data, .. } => {
                if out.len().saturating_add(data.len()) > MAX_OUTPUT_BYTES {
                    return Err("远程命令输出超过 16 MiB 安全上限".into());
                }
                out.push_str(&String::from_utf8_lossy(&data));
            }
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => exit_status = Some(status),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    match exit_status {
        Some(0) | None => Ok(out),
        Some(status) => Err(if out.trim().is_empty() {
            format!("远程命令失败（退出码 {status}）")
        } else {
            format!("远程命令失败（退出码 {status}）：{}", out.trim())
        }),
    }
}

/// 构造对外快照（纯函数，供命令与事件共用）
pub(crate) fn snapshot(
    session_id: &str,
    h: &SshSessionHandle,
    status: ConnectionStatus,
    error: Option<String>,
) -> ServerConnection {
    ServerConnection {
        profile_id: h.profile_id.clone(),
        session_id: session_id.to_string(),
        status,
        host: if h.open { Some(h.host.clone()) } else { None },
        latency_ms: None,
        error,
        connected_at: if h.open { Some(h.connected_at) } else { None },
    }
}

/* ── 分阶段连接 ── */

/// 推送连接阶段事件（尽力而为，推送失败不阻断连接）
pub(crate) fn emit_stage(
    app: &AppHandle,
    request_id: &str,
    profile_id: &str,
    stage: &str,
    status: &str,
    message: Option<String>,
) {
    let _ = app.emit(
        "ssh://connect-stage",
        &crate::plugins::ssh::models::ConnectStage {
            request_id: request_id.to_string(),
            profile_id: profile_id.to_string(),
            stage: stage.into(),
            status: status.into(),
            message,
        },
    );
}

/// 便捷构造结构化错误
pub(crate) fn connect_error(
    code: &str,
    message: String,
    detail: Option<String>,
) -> SshConnectError {
    SshConnectError {
        code: code.into(),
        message,
        detail,
    }
}

/// 从 outcome 组装连接成功快照并登记会话
fn register_session(
    state: &SshState,
    profile: &ServerProfile,
    session: std::sync::Arc<client::Handle<SshHandler>>,
    forward_targets: ForwardTargets,
) -> Result<String, SshConnectError> {
    let session_id = resource_id("conn");
    let handle = SshSessionHandle {
        profile_id: profile.id.clone(),
        host: profile.host.clone(),
        open: true,
        connected_at: now_ms(),
        session,
        sftp: Mutex::new(None),
        forward_targets,
    };
    // 注册表锁中毒不吞：断开刚建立的会话并报错（宁可连接失败也不留无人持有的会话）
    match state.0.lock() {
        Ok(mut map) => {
            map.insert(session_id.clone(), handle);
            Ok(session_id)
        }
        Err(e) => Err(connect_error(
            "INTERNAL",
            "会话注册失败（注册表异常）".into(),
            Some(e.to_string()),
        )),
    }
}

/// 全部会话快照（侧栏/状态栏轮询用）
#[tauri::command]
pub async fn ssh_connections(state: State<'_, SshState>) -> Result<Vec<ServerConnection>, String> {
    let map = state.0.lock().map_err(|e| e.to_string())?;
    Ok(map
        .iter()
        .map(|(id, h)| {
            snapshot(
                id,
                h,
                if h.open {
                    ConnectionStatus::Connected
                } else {
                    ConnectionStatus::Disconnected
                },
                None,
            )
        })
        .collect())
}

/* ── 主机密钥确认与已知主机管理 ── */

/// 前端应答主机密钥确认（握手回调被唤醒后继续/终止连接）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_host_key_respond(
    state: State<'_, HostKeyState>,
    request_id: String,
    decision: HostKeyDecision,
) -> Result<SshActionResult, String> {
    let sender = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id);
    match sender {
        Some(tx) => {
            let _ = tx.send(decision);
            Ok(SshActionResult {
                ok: true,
                error: None,
            })
        }
        None => Ok(SshActionResult {
            ok: false,
            error: Some("确认请求不存在或已超时".into()),
        }),
    }
}

/// 已知主机列表
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_known_host_list(app: AppHandle) -> Result<Vec<KnownHostEntry>, String> {
    host_keys::list_entries(&host_keys::known_hosts_file(&app)?)
}

/// 删除已知主机条目（fingerprint 为空删除该主机全部条目）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_known_host_delete(
    app: AppHandle,
    host: String,
    port: u16,
    fingerprint: Option<String>,
) -> Result<SshActionResult, String> {
    let path = host_keys::known_hosts_file(&app)?;
    let removed =
        host_keys::delete_entries(&path, &host, port, fingerprint.as_deref().unwrap_or(""))?;
    if removed == 0 {
        return Ok(SshActionResult {
            ok: false,
            error: Some("未找到匹配的已知主机条目".into()),
        });
    }
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}
