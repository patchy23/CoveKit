//! SSH 插件 · 端口隧道（-L 本地转发 / -R 远程转发 / -D 动态 SOCKS5）
//! 生命周期：配置存 ssh.db（随 profile）；运行时句柄存 TunnelState。desired_running
//! 记录"期望运行"——start/stop 改写它，断线不改写，因此重连成功后按它自动恢复。
//! 数据面：local/dynamic = 本地 TcpListener + direct-tcpip 通道互拷；
//! remote = tcpip_forward 请求服务端监听，入站连接由 SshHandler 回调按目标表管道回本机。

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use russh::client::Handle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::copy_bidirectional, net::TcpListener, sync::watch};

use crate::plugins::ssh::conn::{SshHandler, SshSessionHandle, SshState};
use crate::plugins::ssh::models::{TunnelConfig, TunnelRuntime, TunnelStatus, TunnelType};
use crate::plugins::ssh::store::{self, ProfileState};

/// with_db 的裸数据版（ProfileState 不经 State 包装时使用；逻辑与 store::with_db 一致）
fn with_db_data<T>(
    app: &AppHandle,
    state: &ProfileState,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let arc = {
        let mut slot = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(db) = slot.as_ref() {
            db.clone()
        } else {
            let db = Arc::new(crate::framework::store::PluginDb::open(
                app,
                "ssh",
                store::MIGRATIONS,
            )?);
            *slot = Some(db.clone());
            db
        }
    };
    arc.with_conn(f)
}

/// -R 转发目标（SshHandler 回调按监听地址查此表，把服务端入站连接管道到本机侧目标）
pub(crate) struct ForwardTarget {
    /// 本机侧目标主机
    pub(crate) host: String,
    /// 本机侧目标端口
    pub(crate) port: u16,
    /// 活动连接数（与 TunnelHandle 共享同一计数器）
    pub(crate) counter: Arc<AtomicU64>,
}

/// -R 目标表：(监听 host, 监听 port) → 目标
pub(crate) type ForwardTargets = Arc<Mutex<HashMap<(String, u16), ForwardTarget>>>;

/// 新建空目标表（open_session 时创建，handler 与会话句柄共享）
pub(crate) fn new_forward_targets() -> ForwardTargets {
    Arc::new(Mutex::new(HashMap::new()))
}

/// 隧道运行时注册表：tunnelId → 句柄（断开后保留，承载 desired_running 供重连恢复）
pub struct TunnelState(pub Mutex<HashMap<String, Arc<TunnelHandle>>>);

/// 单条隧道运行时句柄
pub(crate) struct TunnelHandle {
    /// 隧道配置（运行期不可变；改配置 = 停止后重新启动）
    pub(crate) config: TunnelConfig,
    /// 绑定的连接会话 id（重连后更新）
    pub(crate) connection_id: Mutex<String>,
    /// 当前状态
    pub(crate) status: Mutex<TunnelStatus>,
    /// 异常信息（status=error 时）
    pub(crate) error: Mutex<Option<String>>,
    /// 活动连接计数（-R 与 handler 回调共享）
    pub(crate) connections: Arc<AtomicU64>,
    /// 期望运行：start=true / 用户 stop=false；断线不改写
    pub(crate) desired_running: AtomicBool,
    /// 停止信号：发送 true 或 drop 都使监听任务退出
    pub(crate) cancel_tx: Mutex<Option<watch::Sender<bool>>>,
    /// 本地监听任务（remote 类型为 None）
    pub(crate) listener: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl TunnelHandle {
    /// 构造运行时快照
    fn runtime(&self) -> TunnelRuntime {
        TunnelRuntime {
            config: self.config.clone(),
            status: self
                .status
                .lock()
                .map(|s| *s)
                .unwrap_or(TunnelStatus::Stopped),
            connections: self.connections.load(Ordering::Relaxed),
            error: self.error.lock().ok().and_then(|e| e.clone()),
        }
    }

    /// 更新状态并推送事件（尽力而为）
    fn set_status(&self, app: &AppHandle, status: TunnelStatus, error: Option<String>) {
        if let Ok(mut slot) = self.status.lock() {
            *slot = status;
        }
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
        let _ = app.emit("ssh://tunnel-status", &self.runtime());
    }
}

/* ── 命令 ── */

/// 启动隧道（绑定到指定连接；已在运行时直接返回现状）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_tunnel_start(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    tunnel_state: State<'_, TunnelState>,
    profile_state: State<'_, ProfileState>,
    connection_id: String,
    tunnel_id: String,
) -> Result<TunnelRuntime, String> {
    ssh_tunnel_start_inner(
        &app,
        &ssh_state,
        &tunnel_state,
        profile_state.inner(),
        &connection_id,
        &tunnel_id,
    )
    .await
}

/// 停止隧道（desired_running 置 false：显式停止即用户不再想要自动恢复）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_tunnel_stop(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    tunnel_state: State<'_, TunnelState>,
    tunnel_id: String,
) -> Result<TunnelRuntime, String> {
    let handle = {
        let map = tunnel_state.0.lock().map_err(|e| e.to_string())?;
        map.get(&tunnel_id).cloned()
    };
    let Some(handle) = handle else {
        return Err("隧道未在运行".into());
    };
    let session_info = session_for_handle(&ssh_state, &handle);
    stop_handle_with_session(&app, session_info, &handle).await;
    handle.desired_running.store(false, Ordering::Relaxed);
    Ok(handle.runtime())
}

/// 某连接下的全部隧道运行时快照
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_tunnels(
    tunnel_state: State<'_, TunnelState>,
    connection_id: String,
) -> Result<Vec<TunnelRuntime>, String> {
    let map = tunnel_state.0.lock().map_err(|e| e.to_string())?;
    Ok(map
        .values()
        .filter(|h| {
            h.connection_id
                .lock()
                .map(|c| c.as_str() == connection_id)
                .unwrap_or(false)
        })
        .map(|h| h.runtime())
        .collect())
}

/// 删除隧道配置（运行中先停止）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_tunnel_delete(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    tunnel_state: State<'_, TunnelState>,
    profile_state: State<'_, ProfileState>,
    tunnel_id: String,
) -> Result<(), String> {
    let handle = {
        let map = tunnel_state.0.lock().map_err(|e| e.to_string())?;
        map.get(&tunnel_id).cloned()
    };
    if let Some(handle) = handle {
        let session_info = session_for_handle(&ssh_state, &handle);
        stop_handle_with_session(&app, session_info, &handle).await;
    }
    store::with_db(&app, &profile_state, |conn| {
        store::delete_tunnel(conn, &tunnel_id)
    })?;
    if let Ok(mut map) = tunnel_state.0.lock() {
        map.remove(&tunnel_id);
    }
    Ok(())
}

/* ── 启停核心 ── */

/// 启动实现（命令与断线重连恢复共用）
async fn ssh_tunnel_start_inner(
    app: &AppHandle,
    ssh_state: &SshState,
    tunnel_state: &TunnelState,
    profile_state: &ProfileState,
    connection_id: &str,
    tunnel_id: &str,
) -> Result<TunnelRuntime, String> {
    let config = {
        let tid = tunnel_id.to_string();
        let app2 = app.clone();
        let state2 = profile_state;
        with_db_data(&app2, state2, |c| store::get_tunnel(c, &tid))?
    };
    {
        let map = tunnel_state.0.lock().map_err(|e| e.to_string())?;
        if let Some(handle) = map.get(tunnel_id) {
            let status = handle
                .status
                .lock()
                .map(|s| *s)
                .map_err(|e| e.to_string())?;
            if matches!(status, TunnelStatus::Starting | TunnelStatus::Running) {
                return Ok(handle.runtime());
            }
        }
    }
    // 会话句柄（含 -R 目标表）
    let conn = conn_handle(ssh_state, connection_id)?;

    let (cancel_tx, cancel_rx) = watch::channel(false);
    let handle = Arc::new(TunnelHandle {
        config: config.clone(),
        connection_id: Mutex::new(connection_id.to_string()),
        status: Mutex::new(TunnelStatus::Starting),
        error: Mutex::new(None),
        connections: Arc::new(AtomicU64::new(0)),
        desired_running: AtomicBool::new(true),
        cancel_tx: Mutex::new(Some(cancel_tx)),
        listener: Mutex::new(None),
    });

    let bind_addr = (config.listen_host.as_str(), config.listen_port);
    match config.tunnel_type {
        TunnelType::Local => {
            let target_host = config
                .target_host
                .clone()
                .filter(|h| !h.trim().is_empty())
                .ok_or("本地转发必须填写目标地址")?;
            let target_port = config.target_port.ok_or("本地转发必须填写目标端口")?;
            let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
                format!(
                    "监听 {}:{} 失败：{e}",
                    config.listen_host, config.listen_port
                )
            })?;
            handle.set_status(app, TunnelStatus::Running, None);
            let task = spawn_local_forward(
                app.clone(),
                handle.clone(),
                conn.session.clone(),
                listener,
                cancel_rx,
                target_host,
                target_port,
            );
            if let Ok(mut slot) = handle.listener.lock() {
                *slot = Some(task);
            }
        }
        TunnelType::Dynamic => {
            let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
                format!(
                    "监听 {}:{} 失败：{e}",
                    config.listen_host, config.listen_port
                )
            })?;
            handle.set_status(app, TunnelStatus::Running, None);
            let task =
                spawn_dynamic_forward(handle.clone(), conn.session.clone(), listener, cancel_rx);
            if let Ok(mut slot) = handle.listener.lock() {
                *slot = Some(task);
            }
        }
        TunnelType::Remote => {
            let target_host = config
                .target_host
                .clone()
                .filter(|h| !h.trim().is_empty())
                .ok_or("远程转发必须填写本机侧目标地址")?;
            let target_port = config.target_port.ok_or("远程转发必须填写本机侧目标端口")?;
            if let Err(e) = conn
                .session
                .tcpip_forward(config.listen_host.as_str(), u32::from(config.listen_port))
                .await
            {
                let message = format!(
                    "服务端拒绝监听 {}:{}：{e}（检查 GatewayPorts 与端口权限）",
                    config.listen_host, config.listen_port
                );
                handle.set_status(app, TunnelStatus::Error, Some(message.clone()));
                if let Ok(mut map) = tunnel_state.0.lock() {
                    map.insert(tunnel_id.to_string(), handle.clone());
                }
                return Err(message);
            }
            if let Ok(mut targets) = conn.forward_targets.lock() {
                targets.insert(
                    (config.listen_host.clone(), config.listen_port),
                    ForwardTarget {
                        host: target_host,
                        port: target_port,
                        counter: handle.connections.clone(),
                    },
                );
            }
            handle.set_status(app, TunnelStatus::Running, None);
        }
    }

    if let Ok(mut map) = tunnel_state.0.lock() {
        map.insert(tunnel_id.to_string(), handle.clone());
    }
    Ok(handle.runtime())
}

/// 从注册表取连接句柄（含会话与 -R 目标表）
fn conn_handle(state: &SshState, connection_id: &str) -> Result<Arc<SshSessionHandle>, String> {
    let handle = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        let handle = map
            .get(connection_id)
            .ok_or_else(|| "连接不存在或已断开".to_string())?;
        let session = handle.session.clone();
        let forward_targets = handle.forward_targets.clone();
        let profile_id = handle.profile_id.clone();
        let host = handle.host.clone();
        (
            handle.open,
            handle.connected_at,
            session,
            forward_targets,
            profile_id,
            host,
        )
    };
    let (open, connected_at, session, forward_targets, profile_id, host) = handle;
    Ok(Arc::new(SshSessionHandle {
        profile_id,
        host,
        open,
        connected_at,
        session,
        sftp: Mutex::new(None),
        forward_targets,
    }))
}

/// 停止前同步提取的会话信息（避免持锁跨 await）
struct StopSession {
    /// SSH 会话句柄（用于注销远程转发）
    session: Arc<russh::client::Handle<SshHandler>>,
    /// -R 目标表（注销时移除条目）
    forward_targets: ForwardTargets,
}

/// 同步取回停止所需的会话数据
fn session_for_handle(ssh_state: &SshState, handle: &Arc<TunnelHandle>) -> Option<StopSession> {
    let connection_id = handle.connection_id.lock().ok()?.clone();
    let conn = conn_handle(ssh_state, &connection_id).ok()?;
    Some(StopSession {
        session: conn.session.clone(),
        forward_targets: conn.forward_targets.clone(),
    })
}

/// 停止单条隧道（cancel 信号 + 远程转发注销 + abort 监听任务）
async fn stop_handle_with_session(
    app: &AppHandle,
    session_info: Option<StopSession>,
    handle: &Arc<TunnelHandle>,
) {
    if let Ok(mut tx) = handle.cancel_tx.lock() {
        if let Some(tx) = tx.take() {
            let _ = tx.send(true);
        }
    }
    if let Ok(mut slot) = handle.listener.lock() {
        if let Some(task) = slot.take() {
            task.abort();
        }
    }
    if handle.config.tunnel_type == TunnelType::Remote {
        if let Some(info) = session_info {
            let _ = info
                .session
                .cancel_tcpip_forward(
                    handle.config.listen_host.as_str(),
                    u32::from(handle.config.listen_port),
                )
                .await;
            if let Ok(mut targets) = info.forward_targets.lock() {
                targets.remove(&(handle.config.listen_host.clone(), handle.config.listen_port));
            }
        }
    }
    handle.set_status(app, TunnelStatus::Stopped, None);
}

/* ── 生命周期联动（conn.rs 调用） ── */

/// 连接断开：该会话的全部隧道置为 Stopped（desired_running 保留，重连后自动恢复）
pub(crate) fn stop_session_tunnels(state: &TunnelState, connection_id: &str) {
    if let Ok(map) = state.0.lock() {
        for handle in map.values() {
            let mine = handle
                .connection_id
                .lock()
                .map(|c| c.as_str() == connection_id)
                .unwrap_or(false);
            if !mine {
                continue;
            }
            if let Ok(tx) = handle.cancel_tx.lock() {
                if let Some(tx) = tx.as_ref() {
                    let _ = tx.send(true);
                }
            }
            if let Ok(slot) = handle.listener.lock() {
                if let Some(task) = slot.as_ref() {
                    task.abort();
                }
            }
            if let Ok(mut status) = handle.status.lock() {
                *status = TunnelStatus::Stopped;
            }
        }
    }
}

/// 连接建立/重连成功：恢复该 profile 下期望运行的隧道。
/// 规则：已有句柄看 desired_running；首次（无句柄）看 auto_start。
pub(crate) fn resume_for_profile(
    app: &AppHandle,
    profile_state: &ProfileState,
    profile_id: &str,
    connection_id: &str,
) {
    let configs = {
        let pid = profile_id.to_string();
        let app2 = app.clone();
        match with_db_data(&app2, profile_state, |c| store::list_tunnels(c, &pid)) {
            Ok(list) => list,
            Err(_) => return,
        }
    };
    for config in configs {
        // desired 判定需要读运行注册表（同步、短临界区）
        let desired = {
            let state = app.state::<TunnelState>();
            let map = state.0.lock().ok();
            map.and_then(|map| {
                map.get(&config.id)
                    .map(|h| h.desired_running.load(Ordering::Relaxed))
            })
        };
        let should_start = desired.unwrap_or(config.auto_start);
        if !should_start {
            continue;
        }
        let app2 = app.clone();
        let cid = connection_id.to_string();
        let tid = config.id.clone();
        tauri::async_runtime::spawn(async move {
            let ssh = app2.state::<SshState>();
            let tunnels = app2.state::<TunnelState>();
            let profiles = app2.state::<ProfileState>();
            let _ = ssh_tunnel_start_inner(&app2, &ssh, &tunnels, &profiles, &cid, &tid).await;
        });
    }
}

/* ── 数据面：本地 / SOCKS5 ── */

/// 本地转发（-L）：accept → direct-tcpip → 双向互拷
fn spawn_local_forward(
    app: AppHandle,
    handle: Arc<TunnelHandle>,
    session: Arc<Handle<SshHandler>>,
    listener: TcpListener,
    mut cancel_rx: watch::Receiver<bool>,
    target_host: String,
    target_port: u16,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                accepted = listener.accept() => accepted,
            };
            let (tcp, peer) = match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    handle.set_status(
                        &app,
                        TunnelStatus::Error,
                        Some(format!("接受连接失败: {e}")),
                    );
                    break;
                }
            };
            let session = session.clone();
            let counter = handle.connections.clone();
            let target_host = target_host.clone();
            counter.fetch_add(1, Ordering::Relaxed);
            tauri::async_runtime::spawn(async move {
                let pipe = async {
                    let channel = session
                        .channel_open_direct_tcpip(
                            target_host.as_str(),
                            u32::from(target_port),
                            peer.ip().to_string(),
                            u32::from(peer.port()),
                        )
                        .await
                        .map_err(|e| format!("打开转发通道失败: {e}"))?;
                    let mut stream = channel.into_stream();
                    let mut tcp = tcp;
                    copy_bidirectional(&mut stream, &mut tcp)
                        .await
                        .map_err(|e| format!("转发中断: {e}"))?;
                    Ok::<(), String>(())
                };
                let _ = pipe.await;
                counter.fetch_sub(1, Ordering::Relaxed);
            });
        }
    })
}

/// 动态转发（-D）：SOCKS5 握手后按客户端请求 direct-tcpip
fn spawn_dynamic_forward(
    handle: Arc<TunnelHandle>,
    session: Arc<Handle<SshHandler>>,
    listener: TcpListener,
    mut cancel_rx: watch::Receiver<bool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                accepted = listener.accept() => accepted,
            };
            let (tcp, peer) = match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    let _ = &handle;
                    let _ = e;
                    break;
                }
            };
            let session = session.clone();
            let counter = handle.connections.clone();
            counter.fetch_add(1, Ordering::Relaxed);
            tauri::async_runtime::spawn(async move {
                let _ = serve_socks5(session, tcp, peer.port()).await;
                counter.fetch_sub(1, Ordering::Relaxed);
            });
        }
    })
}

/// 单个 SOCKS5 客户端的最小实现：无认证 + CONNECT 命令
async fn serve_socks5(
    session: Arc<Handle<SshHandler>>,
    mut tcp: tokio::net::TcpStream,
    peer_port: u16,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut head = [0u8; 2];
    tcp.read_exact(&mut head)
        .await
        .map_err(|e| format!("读取 SOCKS5 握手失败: {e}"))?;
    if head[0] != 5 {
        return Err("不是 SOCKS5 协议".into());
    }
    let methods = head[1] as usize;
    let mut skip = vec![0u8; methods];
    tcp.read_exact(&mut skip)
        .await
        .map_err(|e| format!("读取认证方法失败: {e}"))?;
    tcp.write_all(&[5, 0])
        .await
        .map_err(|e| format!("应答认证失败: {e}"))?;

    let mut req = [0u8; 4];
    tcp.read_exact(&mut req)
        .await
        .map_err(|e| format!("读取请求失败: {e}"))?;
    if req[1] != 1 {
        reply_socks5(&mut tcp, 0x07).await?;
        return Err("仅支持 CONNECT 命令".into());
    }
    let host = match req[3] {
        1 => {
            let mut buf = [0u8; 4];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            std::net::Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            tcp.read_exact(&mut len).await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; len[0] as usize];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|_| "域名不是合法 UTF-8".to_string())?
        }
        4 => {
            let mut buf = [0u8; 16];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            std::net::Ipv6Addr::from(buf).to_string()
        }
        other => {
            reply_socks5(&mut tcp, 0x08).await?;
            return Err(format!("不支持的地址类型 {other}"));
        }
    };
    let mut port_buf = [0u8; 2];
    tcp.read_exact(&mut port_buf)
        .await
        .map_err(|e| e.to_string())?;
    let port = u16::from_be_bytes(port_buf);

    let channel = match session
        .channel_open_direct_tcpip(
            host.as_str(),
            u32::from(port),
            "127.0.0.1",
            u32::from(peer_port),
        )
        .await
    {
        Ok(channel) => channel,
        Err(e) => {
            reply_socks5(&mut tcp, 0x01).await?;
            return Err(format!("打开转发通道失败: {e}"));
        }
    };
    reply_socks5(&mut tcp, 0x00).await?;
    let mut stream = channel.into_stream();
    copy_bidirectional(&mut stream, &mut tcp)
        .await
        .map_err(|e| format!("转发中断: {e}"))?;
    Ok(())
}

/// SOCKS5 应答（仅状态字节有意义的极简包）
async fn reply_socks5(tcp: &mut tokio::net::TcpStream, code: u8) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    tcp.write_all(&[5, code, 0, 0, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| format!("SOCKS5 应答失败: {e}"))
}

/// 单测：TunnelType 序列化与解析往返
#[cfg(test)]
mod tests {
    use crate::plugins::ssh::models::TunnelType;

    #[test]
    fn tunnel_type_roundtrip() {
        for t in [TunnelType::Local, TunnelType::Remote, TunnelType::Dynamic] {
            assert_eq!(TunnelType::from_str(t.as_str()), t);
        }
        assert_eq!(TunnelType::from_str("bogus"), TunnelType::Local);
    }
}
