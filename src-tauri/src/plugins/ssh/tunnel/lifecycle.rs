//! SSH 隧道 · 命令与连接生命周期联动

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use tauri::{AppHandle, Manager, State};
use tokio::{net::TcpListener, sync::watch};

use crate::plugins::ssh::conn::{SshHandler, SshSessionHandle, SshState};
use crate::plugins::ssh::models::{TunnelRuntime, TunnelStatus, TunnelType};
use crate::plugins::ssh::store::{self, ProfileState};
use crate::plugins::ssh::tunnel::TunnelState;

use super::forward::{spawn_dynamic_forward, spawn_local_forward};
use super::ftargets::{ForwardTarget, ForwardTargets};
use super::{with_db_data, TunnelHandle};

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
        id_names: Mutex::new(None),
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
