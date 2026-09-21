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
mod exec_output;
pub(crate) mod handler;
pub(crate) mod reconnect;
pub(crate) mod session;

pub(crate) use handler::SshHandler;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use russh::client;
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
    /// uid/gid → 用户名/组名缓存（None = 尚未解析；文件列表展示用，解析失败回退数字）
    pub(crate) id_names: Mutex<Option<Arc<IdNameMap>>>,
}

/// uid/gid → 名称映射（文件列表 owner/group 列展示；SFTP 线协议只带数字，名字靠 getent 解析）
#[derive(Default)]
pub(crate) struct IdNameMap {
    /// uid → 用户名
    pub(crate) users: HashMap<u32, String>,
    /// gid → 组名
    pub(crate) groups: HashMap<u32, String>,
}

/// SSH 会话注册表（State 注入，惰性初始化）
pub struct SshState(pub Mutex<HashMap<String, SshSessionHandle>>);

/// 解析连接的 uid/gid → 用户名/组名（每连接只执行一次，失败缓存空表避免反复 exec）。
/// OpenSSH 的 SFTP attrs 只带 uid/gid 数字；所有者名称从 `getent passwd/group`（退化 /etc/passwd）解析。
pub(crate) async fn resolve_id_names(ssh_state: &SshState, connection_id: &str) -> Arc<IdNameMap> {
    // 先查缓存（锁不跨 await：取出 Arc 即放锁）
    let cached: Option<Arc<IdNameMap>> = ssh_state.0.lock().ok().and_then(|map| {
        map.get(connection_id)
            .and_then(|h| h.id_names.lock().ok()?.clone())
    });
    if let Some(map) = cached {
        return map;
    }
    let session = {
        ssh_state
            .0
            .lock()
            .ok()
            .and_then(|map| map.get(connection_id).map(|h| Arc::clone(&h.session)))
    };
    let mut parsed = IdNameMap::default();
    if let Some(session) = session {
        // getent 覆盖 NSS（含 LDAP），失败退 /etc/passwd + /etc/group
        if let Ok(out) = exec_collect(
            &session,
            "getent passwd 2>/dev/null || cat /etc/passwd; echo; echo ---groups---; getent group 2>/dev/null || cat /etc/group",
        )
        .await
        {
            let mut in_groups = false;
            for line in out.lines() {
                let line = line.trim();
                if line == "---groups---" {
                    in_groups = true;
                    continue;
                }
                let parts: Vec<&str> = line.split(':').collect();
                // passwd: name:x:uid:gid:...；group: name:x:gid:...
                if parts.len() >= 3 {
                    if let Ok(id) = parts[2].parse::<u32>() {
                        if in_groups {
                            parsed.groups.insert(id, parts[0].to_string());
                        } else {
                            parsed.users.insert(id, parts[0].to_string());
                        }
                    }
                }
            }
        }
    }
    let map = Arc::new(parsed);
    // 写回缓存（含解析失败——失败也缓存，避免每次列目录都 exec）
    if let Ok(guard) = ssh_state.0.lock() {
        if let Some(h) = guard.get(connection_id) {
            if let Ok(mut slot) = h.id_names.lock() {
                *slot = Some(Arc::clone(&map));
            }
        }
    }
    map
}

/// 当前毫秒时间戳
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 生成全局唯一的资源 id（书签 / 隧道 / 分组 / 连接会话）。
///
/// 用 UUIDv4 而不是「毫秒 + 进程内计数」：导入/合并会把**另一台机器**上生成的记录搬进来，
/// 进程内计数在跨机器场景下必然撞号（同一毫秒 + 同一个起始计数），撞号的记录会互相覆盖。
pub(crate) fn resource_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
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
        // 已标记断开的会话不再放出新操作（避免在死连接上堆积失败调用）
        .filter(|h| h.open)
        .map(|h| h.session.clone())
        .ok_or_else(|| "连接不存在或已断开".to_string())
}

/// 仅在传输层确已关闭时标记断线；通道配额或策略拒绝不影响仍存活的连接。
/// 同时比对句柄身份，避免旧请求迟到后把重连成功的新会话标成断开。
pub(crate) fn mark_session_closed(
    state: &SshState,
    connection_id: &str,
    session: &Arc<client::Handle<SshHandler>>,
) {
    if !session.is_closed() {
        return;
    }
    if let Ok(mut map) = state.0.lock() {
        if let Some(h) = map.get_mut(connection_id) {
            if !Arc::ptr_eq(&h.session, session) {
                return;
            }
            h.open = false;
            if let Ok(mut slot) = h.sftp.lock() {
                *slot = None;
            }
        }
    }
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
    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(e) => {
            mark_session_closed(state.inner(), connection_id, &session);
            return Err(format!("打开通道失败: {e}"));
        }
    };
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
/// 非交互 exec：开通道 → exec → 循环 wait() 收输出和退出状态直到 Close
pub(crate) async fn exec_collect(
    session: &client::Handle<SshHandler>,
    command: &str,
) -> Result<String, String> {
    // 整体兜底超时：仅靠 keepalive 时协议层 stall 会无限挂起（轮询类命令全是快路径）
    const EXEC_TIMEOUT: Duration = Duration::from_secs(30);
    tokio::time::timeout(EXEC_TIMEOUT, exec_collect_inner(session, command))
        .await
        .map_err(|_| format!("命令执行超时（{} 秒）", EXEC_TIMEOUT.as_secs()))?
}

/// exec_collect 本体（超时由外层包裹）
async fn exec_collect_inner(
    session: &client::Handle<SshHandler>,
    command: &str,
) -> Result<String, String> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .exec(false, command)
        .await
        .map_err(|e| format!("执行失败: {e}"))?;
    let mut output = exec_output::ExecOutput::default();
    while let Some(message) = channel.wait().await {
        if output.receive(message)? {
            break;
        }
    }
    output.finish()
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
    log::debug!("SSH 连接阶段 request={request_id} stage={stage} status={status}");
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
        id_names: Mutex::new(None),
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
