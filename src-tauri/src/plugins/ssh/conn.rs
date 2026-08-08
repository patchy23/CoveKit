//! SSH 插件 · 连接会话注册表（russh 客户端）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>)，
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use russh::{client, ChannelMsg};
use tauri::{AppHandle, Emitter, State};

use crate::plugins::ssh::models::{
    AuthMethod, ConnectionStatus, ServerConnection, ServerProfile, SshActionResult,
    SshConnectPayload,
};

/// russh 客户端 Handler 最小实现
/// 数据路由在通道级完成（terminal.rs 的 wait() 读循环），Handler 仅需接受服务器密钥。
#[derive(Clone)]
pub struct SshHandler;

impl client::Handler for SshHandler {
    type Error = russh::Error;

    /// 接受任意服务器密钥（主机指纹校验为 P2 增强项）
    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

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

/// 执行远程命令并收集全部输出（监控/服务/进程/Docker 通用）
/// 非交互 exec：开通道 → exec → 循环 wait() 收 Data 直到 Eof/Close
pub(crate) async fn exec_collect(
    session: &client::Handle<SshHandler>,
    cmd: &str,
) -> Result<String, String> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .exec(false, cmd)
        .await
        .map_err(|e| format!("执行失败: {e}"))?;
    let mut out = String::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.push_str(&String::from_utf8_lossy(&data)),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    Ok(out)
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

/// 认证并建立连接（ssh_connect / ssh_reconnect 共用）
pub(crate) async fn open_session(
    profile: &ServerProfile,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
) -> Result<std::sync::Arc<client::Handle<SshHandler>>, String> {
    let config = Arc::new(client::Config::default());
    let addr = format!("{}:{}", profile.host, profile.port);
    let mut session = client::connect(config, addr.as_str(), SshHandler)
        .await
        .map_err(|e| format!("连接失败: {e}"))?;

    // 按认证方式登录：密码 / 私钥 / 私钥+passphrase
    let auth = match profile.auth_method {
        AuthMethod::Password => {
            let pw = password.ok_or("缺少密码")?;
            session
                .authenticate_password(&profile.username, pw)
                .await
                .map_err(|e| format!("密码认证失败: {e}"))?
        }
        AuthMethod::PrivateKey | AuthMethod::PrivateKeyWithPassphrase => {
            let key_text = private_key.ok_or("缺少私钥内容")?;
            let key = russh::keys::PrivateKey::from_openssh(key_text)
                .map_err(|e| format!("私钥解析失败: {e}"))?;
            // 加密私钥：先按 passphrase 解密（无 passphrase 时直接使用）
            let key = match (passphrase, profile.auth_method) {
                (Some(pp), AuthMethod::PrivateKeyWithPassphrase) => {
                    key.decrypt(pp).map_err(|e| format!("私钥解密失败: {e}"))?
                }
                _ => key,
            };
            session
                .authenticate_publickey(
                    &profile.username,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None),
                )
                .await
                .map_err(|e| format!("密钥认证失败: {e}"))?
        }
    };

    if !auth.success() {
        return Err("认证失败：用户名或凭证错误".into());
    }
    Ok(std::sync::Arc::new(session))
}

/// 建立连接并登记会话（ssh_connect 命令；reconnect 复用）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_connect(
    app: AppHandle,
    state: State<'_, SshState>,
    payload: SshConnectPayload,
) -> Result<ServerConnection, String> {
    let profile = &payload.profile;
    let session = open_session(
        profile,
        payload.password.as_deref(),
        payload.private_key.as_deref(),
        payload.passphrase.as_deref(),
    )
    .await?;

    let session_id = format!("conn-{}", now_ms());
    let connected_at = now_ms();
    let handle = SshSessionHandle {
        profile_id: profile.id.clone(),
        host: profile.host.clone(),
        open: true,
        connected_at,
        session,
    };
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id.clone(), handle);

    // 构造快照并推送连接状态事件（前端侧栏实时更新）
    let conn = ServerConnection {
        profile_id: profile.id.clone(),
        session_id: session_id.clone(),
        status: ConnectionStatus::Connected,
        host: Some(profile.host.clone()),
        latency_ms: None,
        error: None,
        connected_at: Some(connected_at),
    };
    app.emit("ssh://connection-status", &conn)
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 断开连接并清理会话
#[tauri::command]
pub async fn ssh_disconnect(
    state: State<'_, SshState>,
    session_id: String,
) -> Result<SshActionResult, String> {
    // 先取走句柄并释放锁（std MutexGuard 非 Send，不能跨 await 持锁）
    let handle = {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        map.remove(&session_id)
    };
    if let Some(h) = handle {
        let _ = h
            .session
            .disconnect(russh::Disconnect::ByApplication, "用户断开", "")
            .await;
    }
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

/// 重新连接（需前端重新提供凭证，与 ssh_connect 同逻辑，新会话替换旧会话）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_reconnect(
    app: AppHandle,
    state: State<'_, SshState>,
    session_id: String,
    payload: Option<SshConnectPayload>,
) -> Result<ServerConnection, String> {
    // 先取走旧会话并释放锁（MutexGuard 非 Send，不能跨 await 持锁）
    let old = {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        map.remove(&session_id)
    };
    if let Some(h) = old {
        let _ = h
            .session
            .disconnect(russh::Disconnect::ByApplication, "重连", "")
            .await;
    }
    let payload = payload.ok_or("重连需要服务器配置与凭证")?;
    let profile = payload.profile;
    let session = open_session(
        &profile,
        payload.password.as_deref(),
        payload.private_key.as_deref(),
        payload.passphrase.as_deref(),
    )
    .await?;

    let new_id = format!("conn-{}", now_ms());
    let connected_at = now_ms();
    state.0.lock().map_err(|e| e.to_string())?.insert(
        new_id.clone(),
        SshSessionHandle {
            profile_id: profile.id.clone(),
            host: profile.host.clone(),
            open: true,
            connected_at,
            session,
        },
    );
    let conn = ServerConnection {
        profile_id: profile.id.clone(),
        session_id: new_id.clone(),
        status: ConnectionStatus::Connected,
        host: Some(profile.host.clone()),
        latency_ms: None,
        error: None,
        connected_at: Some(connected_at),
    };
    app.emit("ssh://connection-status", &conn)
        .map_err(|e| e.to_string())?;
    Ok(conn)
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
