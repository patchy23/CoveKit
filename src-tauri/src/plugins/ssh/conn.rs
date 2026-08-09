//! SSH 插件 · 连接会话注册表（russh 客户端）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>)，
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use russh::{client, ChannelMsg};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::plugins::ssh::models::{
    AuthMethod, ConnectionStatus, ServerConnection, ServerProfile, SshActionResult,
    SshConnectPayload,
};
use crate::plugins::ssh::monitor::MonitorState;
use crate::plugins::ssh::terminal::TerminalState;

/// russh 客户端 Handler；以 TOFU 策略校验并持久化服务器主机密钥。
#[derive(Clone)]
pub struct SshHandler {
    /// 目标主机名。
    host: String,
    /// 目标 SSH 端口。
    port: u16,
    /// patchyBox 私有 known_hosts 文件。
    known_hosts_path: PathBuf,
}

impl client::Handler for SshHandler {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    /// 首次连接记录主机密钥；后续密钥不一致时拒绝连接。
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        static KNOWN_HOSTS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = KNOWN_HOSTS_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|e| format!("known_hosts 锁异常: {e}"))?;
        let known_keys = russh::keys::known_hosts::known_host_keys_path(
            &self.host,
            self.port,
            &self.known_hosts_path,
        )?;
        if known_keys.is_empty() {
            russh::keys::known_hosts::learn_known_hosts_path(
                &self.host,
                self.port,
                server_public_key,
                &self.known_hosts_path,
            )?;
        } else if !known_keys.iter().any(|(_, key)| key == server_public_key) {
            return Err(format!(
                "SSH 主机密钥与首次连接记录不一致：{}:{}（如服务器已合法更换密钥，请删除 {} 中对应条目后重试）",
                self.host,
                self.port,
                self.known_hosts_path.display()
            )
            .into());
        }
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

/// 执行远程命令并收集全部输出（监控/服务/进程/Docker 通用）
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

/// 认证并建立连接（ssh_connect / ssh_reconnect 共用）
pub(crate) async fn open_session(
    profile: &ServerProfile,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: PathBuf,
) -> Result<std::sync::Arc<client::Handle<SshHandler>>, String> {
    let config = Arc::new(client::Config::default());
    let addr = format!("{}:{}", profile.host, profile.port);
    let handler = SshHandler {
        host: profile.host.clone(),
        port: profile.port,
        known_hosts_path,
    };
    let mut session = client::connect(config, addr.as_str(), handler)
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
        app.path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("ssh-known-hosts"),
    )
    .await?;

    let session_id = resource_id("conn");
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
    app: AppHandle,
    state: State<'_, SshState>,
    terminal_state: State<'_, TerminalState>,
    monitor_state: State<'_, MonitorState>,
    session_id: String,
) -> Result<SshActionResult, String> {
    // 先取走句柄并释放锁（std MutexGuard 非 Send，不能跨 await 持锁）
    let handle = {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        map.remove(&session_id)
    };
    monitor_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&session_id);
    if let Some(h) = handle {
        // 先关闭此连接下的全部终端任务，避免断开后注册表残留无效通道。
        if let Ok(mut terminals) = terminal_state.0.lock() {
            terminals.retain(|_, terminal| {
                if terminal.connection_id == session_id {
                    let _ = terminal.cancel.send(true);
                    false
                } else {
                    true
                }
            });
        }
        let _ = h
            .session
            .disconnect(russh::Disconnect::ByApplication, "用户断开", "")
            .await;
        let conn = ServerConnection {
            profile_id: h.profile_id,
            session_id,
            status: ConnectionStatus::Disconnected,
            host: None,
            latency_ms: None,
            error: None,
            connected_at: None,
        };
        app.emit("ssh://connection-status", &conn)
            .map_err(|e| e.to_string())?;
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
    let payload = payload.ok_or("重连需要服务器配置与凭证")?;
    let profile = payload.profile;
    // 先建立并认证新连接；失败时保留仍可用的旧连接。
    let session = open_session(
        &profile,
        payload.password.as_deref(),
        payload.private_key.as_deref(),
        payload.passphrase.as_deref(),
        app.path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("ssh-known-hosts"),
    )
    .await?;

    let new_id = resource_id("conn");
    let connected_at = now_ms();
    let old = {
        let mut sessions = state.0.lock().map_err(|e| e.to_string())?;
        let old = sessions.remove(&session_id);
        sessions.insert(
            new_id.clone(),
            SshSessionHandle {
                profile_id: profile.id.clone(),
                host: profile.host.clone(),
                open: true,
                connected_at,
                session,
            },
        );
        old
    };
    if let Some(h) = old {
        let _ = h
            .session
            .disconnect(russh::Disconnect::ByApplication, "重连", "")
            .await;
    }
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{resource_id, shell_quote};

    #[test]
    fn shell参数中的单引号不会逃逸() {
        assert_eq!(shell_quote("nginx.service"), "'nginx.service'");
        assert_eq!(
            shell_quote("a'; touch /tmp/pwn; echo '"),
            "'a'\"'\"'; touch /tmp/pwn; echo '\"'\"''"
        );
    }

    #[test]
    fn 并发资源id不会碰撞() {
        let ids = (0..1000)
            .map(|_| resource_id("test"))
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 1000);
    }
}
