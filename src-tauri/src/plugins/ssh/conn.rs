//! SSH 插件 · 连接会话注册表（russh 客户端）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>),
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。
//! 连接流程：分阶段事件（ssh://connect-stage）+ 稳定错误码（SshConnectError）；
//! 主机密钥校验在握手回调中等待前端人工确认（ssh://host-key-verify ↔ ssh_host_key_respond），
//! 首连不再静默记录 TOFU，指纹变更必须经用户明确决定（仅本次 / 保存 / 替换 / 取消）。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use russh::{client, ChannelMsg};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::copy_bidirectional, sync::oneshot};

use crate::framework::vault::{self, Credential, CredentialFields};
use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{
    AuthMethod, ConnectionStatus, CredentialOverride, HostKeyDecision, HostKeyVerifyRequest,
    KnownHostEntry, ServerConnection, ServerProfile, SshActionResult, SshConnectError,
    SshConnectOutcome, SshConnectPayload,
};
use crate::plugins::ssh::monitor::MonitorState;
use crate::plugins::ssh::store::{self, ProfileState};
use crate::plugins::ssh::terminal::TerminalState;
use crate::plugins::ssh::tunnel::{
    new_forward_targets, resume_for_profile, stop_session_tunnels, ForwardTargets,
};

/// 主机密钥确认等待时长（超时视为用户放弃，连接失败）
const HOST_KEY_WAIT: Duration = Duration::from_secs(180);
/// DNS 解析 / TCP 建连超时
const CONNECT_PHASE_TIMEOUT: Duration = Duration::from_secs(10);

/// 主机密钥确认应答注册表：requestId → oneshot 发送端（前端 respond 后触发握手回调继续）
pub struct HostKeyState(pub Mutex<HashMap<String, oneshot::Sender<HostKeyDecision>>>);

/// check_server_key 与连接流程共享的结构化失败记录（主机密钥阶段失败时优先取用）
#[derive(Default)]
pub(crate) struct HostKeyVerifySlot {
    /// 校验阶段产生的结构化失败（连接失败时优先取用，错误码更精确）
    pub(crate) failure: Mutex<Option<SshConnectError>>,
}

/// russh 客户端 Handler；主机密钥交由用户确认，不再静默 TOFU。
#[derive(Clone)]
pub struct SshHandler {
    /// 目标主机名。
    host: String,
    /// 目标 SSH 端口。
    port: u16,
    /// patchyBox 私有 known_hosts 文件。
    known_hosts_path: PathBuf,
    /// 事件推送句柄（host-key-verify / connect-stage）。
    app: AppHandle,
    /// 本次连接尝试的请求 id。
    request_id: String,
    /// 交互槽（接收端 + 结构化失败）。
    verify: Arc<HostKeyVerifySlot>,
    /// -R 远程转发的目标表（tunnel 模块注册，回调消费）。
    forward_targets: ForwardTargets,
}

impl SshHandler {
    /// 推送主机密钥确认请求并等待用户决定；超时视为取消。
    /// 应答经 HostKeyState 注册表路由（前端调用 ssh_host_key_respond）。
    async fn wait_user_decision(
        &self,
        request: HostKeyVerifyRequest,
    ) -> Result<HostKeyDecision, SshConnectError> {
        let internal = |message: &str, detail: Option<String>| SshConnectError {
            code: "INTERNAL".into(),
            message: message.into(),
            detail,
        };
        let (tx, rx) = oneshot::channel();
        {
            let state = self.app.state::<HostKeyState>();
            let mut map = state
                .0
                .lock()
                .map_err(|e| internal("主机密钥应答表异常", Some(e.to_string())))?;
            map.insert(request.request_id.clone(), tx);
        }
        if let Err(e) = self.app.emit("ssh://host-key-verify", &request) {
            // 推送失败：移除挂起项，避免注册表泄漏
            if let Some(state) = self.app.try_state::<HostKeyState>() {
                if let Ok(mut map) = state.0.lock() {
                    map.remove(&request.request_id);
                }
            }
            return Err(internal("主机密钥确认事件推送失败", Some(e.to_string())));
        }
        match tokio::time::timeout(HOST_KEY_WAIT, rx).await {
            Ok(Ok(decision)) => Ok(decision),
            _ => {
                // 超时/发送端已丢失：清理挂起项后按超时取消处理
                if let Some(state) = self.app.try_state::<HostKeyState>() {
                    if let Ok(mut map) = state.0.lock() {
                        map.remove(&request.request_id);
                    }
                }
                Err(SshConnectError {
                    code: "HOST_KEY_VERIFY_TIMEOUT".into(),
                    message: format!(
                        "等待主机密钥确认超时（{} 秒），已取消连接",
                        HOST_KEY_WAIT.as_secs()
                    ),
                    detail: None,
                })
            }
        }
    }
}

impl client::Handler for SshHandler {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    /// 主机密钥校验：首连弹确认（仅本次/保存），指纹变更弹阻断（替换/仅本次/取消）。
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = host_keys::fingerprint_of(server_public_key);
        let algorithm = server_public_key.algorithm().as_str().to_string();
        let saved = host_keys::entries_for(&self.known_hosts_path, &self.host, self.port)?;
        if saved.iter().any(|e| e.fingerprint == fingerprint) {
            return Ok(true);
        }
        let kind = if saved.is_empty() {
            "unknown"
        } else {
            "mismatch"
        };
        let request = HostKeyVerifyRequest {
            request_id: self.request_id.clone(),
            kind: kind.into(),
            host: self.host.clone(),
            port: self.port,
            algorithm,
            fingerprint,
            saved_fingerprints: saved.iter().map(|e| e.fingerprint.clone()).collect(),
        };
        let decision = match self.wait_user_decision(request).await {
            Ok(decision) => decision,
            Err(e) => {
                // 结构化失败落入共享槽（open_session 分类失败时优先取用）
                self.record_failure(&e.code, &e.message, e.detail.unwrap_or_default());
                return Err(e.message.into());
            }
        };
        match (kind, decision) {
            ("unknown", HostKeyDecision::TrustSave) => {
                if let Err(e) = host_keys::learn(
                    &self.known_hosts_path,
                    &self.host,
                    self.port,
                    server_public_key,
                ) {
                    self.record_failure(HOST_KEY_STORE_FAILED, "主机密钥保存失败", e.clone());
                    return Err(e.into());
                }
                Ok(true)
            }
            ("unknown", HostKeyDecision::TrustOnce) => Ok(true),
            ("mismatch", HostKeyDecision::TrustOnce) => Ok(true),
            ("mismatch", HostKeyDecision::Replace) => {
                if let Err(e) = host_keys::replace_entries(
                    &self.known_hosts_path,
                    &self.host,
                    self.port,
                    server_public_key,
                ) {
                    self.record_failure(HOST_KEY_STORE_FAILED, "主机密钥替换失败", e.clone());
                    return Err(e.into());
                }
                Ok(true)
            }
            _ => {
                let (code, message) = if kind == "mismatch" {
                    (
                        "HOST_KEY_MISMATCH",
                        format!(
                            "服务器 {}:{} 的主机密钥与已保存指纹不一致，已取消连接",
                            self.host, self.port
                        ),
                    )
                } else {
                    ("CANCELLED", format!("已取消连接 {}", self.host))
                };
                self.record_failure(code, &message, String::new());
                Err(message.into())
            }
        }
    }
    /// -R 远程转发入站连接：按目标表管道到本机侧目标；无目标则拒绝（丢弃 reply 自动拒绝）
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: russh::client::ChannelOpenHandle,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let target = self.forward_targets.lock().ok().and_then(|map| {
            map.get(&(connected_address.to_string(), connected_port as u16))
                .map(|t| (t.host.clone(), t.port, t.counter.clone()))
        });
        let Some((host, port, counter)) = target else {
            return Ok(());
        };
        reply.accept().await;
        counter.fetch_add(1, Ordering::Relaxed);
        let mut stream = channel.into_stream();
        tauri::async_runtime::spawn(async move {
            let pipe = async {
                let mut tcp = tokio::net::TcpStream::connect((host.as_str(), port))
                    .await
                    .map_err(|e| e.to_string())?;
                copy_bidirectional(&mut stream, &mut tcp)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            };
            let _ = pipe.await;
            counter.fetch_sub(1, Ordering::Relaxed);
        });
        Ok(())
    }
}

impl SshHandler {
    /// 记录结构化失败（连接失败时优先取用，比字符串分类更精确）
    fn record_failure(&self, code: &str, message: &str, detail: String) {
        if let Ok(mut slot) = self.verify.failure.lock() {
            *slot = Some(SshConnectError {
                code: code.into(),
                message: message.into(),
                detail: (!detail.is_empty()).then_some(detail),
            });
        }
    }
}

/// 校验阶段存储失败的错误码常量
const HOST_KEY_STORE_FAILED: &str = "HOST_KEY_STORE_FAILED";

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
fn emit_stage(
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
fn connect_error(code: &str, message: String, detail: Option<String>) -> SshConnectError {
    SshConnectError {
        code: code.into(),
        message,
        detail,
    }
}

/// 认证并建立连接（ssh_connect / ssh_reconnect 共用）。
/// 阶段：resolve → tcp → handshake（含 verify 回调）→ auth；每阶段推送事件。
async fn open_session(
    app: &AppHandle,
    request_id: &str,
    resolved: &ResolvedConnectPayload,
    known_hosts_path: PathBuf,
) -> Result<(std::sync::Arc<client::Handle<SshHandler>>, ForwardTargets), SshConnectError> {
    // 注意：认证用的是 resolved.profile——Vault 凭证中的用户名会覆盖表单展示值
    let profile = &resolved.profile;
    let addr = format!("{}:{}", profile.host, profile.port);

    // ① DNS 解析
    emit_stage(app, request_id, &profile.id, "resolve", "start", None);
    let addrs: Vec<std::net::SocketAddr> = match tokio::time::timeout(
        CONNECT_PHASE_TIMEOUT,
        tokio::net::lookup_host(addr.as_str()),
    )
    .await
    {
        Err(_) => {
            return Err(connect_error(
                "DNS_RESOLVE_FAILED",
                format!("解析主机超时：{}", profile.host),
                None,
            ))
        }
        Ok(Err(e)) => {
            return Err(connect_error(
                "DNS_RESOLVE_FAILED",
                format!("解析主机失败：{}", profile.host),
                Some(e.to_string()),
            ))
        }
        Ok(Ok(iter)) => iter.collect(),
    };
    if addrs.is_empty() {
        return Err(connect_error(
            "DNS_RESOLVE_FAILED",
            format!("主机没有可用地址：{}", profile.host),
            None,
        ));
    }
    let resolved_ips = addrs
        .iter()
        .map(|a| a.ip().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    emit_stage(
        app,
        request_id,
        &profile.id,
        "resolve",
        "ok",
        Some(resolved_ips),
    );

    // ② TCP 建连（显式超时 + 错误分类）
    emit_stage(app, request_id, &profile.id, "tcp", "start", None);
    let stream = match tokio::time::timeout(
        CONNECT_PHASE_TIMEOUT,
        tokio::net::TcpStream::connect(addrs.as_slice()),
    )
    .await
    {
        Err(_) => {
            return Err(connect_error(
                "TCP_TIMEOUT",
                format!(
                    "连接超时（{}:{}，{} 秒）",
                    profile.host,
                    profile.port,
                    CONNECT_PHASE_TIMEOUT.as_secs()
                ),
                None,
            ))
        }
        Ok(Err(e)) => {
            let (code, message) = match e.kind() {
                std::io::ErrorKind::ConnectionRefused => (
                    "CONNECTION_REFUSED",
                    format!(
                        "连接被拒绝（{}:{}）——端口未开放或服务未启动",
                        profile.host, profile.port
                    ),
                ),
                std::io::ErrorKind::PermissionDenied => (
                    "TCP_DENIED",
                    format!("连接被系统拒绝（{}:{}）", profile.host, profile.port),
                ),
                _ => (
                    "TCP_FAILED",
                    format!("无法连接 {}:{}：{}", profile.host, profile.port, e),
                ),
            };
            return Err(connect_error(code, message, None));
        }
        Ok(Ok(stream)) => stream,
    };
    let _ = stream.set_nodelay(true);
    emit_stage(app, request_id, &profile.id, "tcp", "ok", None);

    // ③ 握手（含主机密钥人工确认回调）+ ④ 认证
    let config = Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let forward_targets = new_forward_targets();
    let handler = SshHandler {
        host: profile.host.clone(),
        port: profile.port,
        known_hosts_path,
        app: app.clone(),
        request_id: request_id.to_string(),
        verify: resolved.verify.clone(),
        forward_targets: forward_targets.clone(),
    };
    emit_stage(app, request_id, &profile.id, "handshake", "start", None);
    let mut session = match client::connect_stream(config, stream, handler).await {
        Ok(session) => session,
        Err(e) => {
            // 主机密钥阶段的失败已在槽里带结构化信息（更精确），其余按协议错误分类
            let structured = resolved
                .verify
                .failure
                .lock()
                .ok()
                .and_then(|mut slot| slot.take());
            if let Some(err) = structured {
                return Err(err);
            }
            return Err(connect_error(
                "PROTOCOL_ERROR",
                format!("SSH 握手失败：{}", profile.host),
                Some(e.to_string()),
            ));
        }
    };
    emit_stage(app, request_id, &profile.id, "verify", "ok", None);

    emit_stage(app, request_id, &profile.id, "auth", "start", None);
    let auth = match profile.auth_method {
        AuthMethod::Password => {
            let pw = resolved
                .password
                .as_deref()
                .ok_or_else(|| connect_error("MISSING_CREDENTIAL", "缺少密码".into(), None))?;
            session
                .authenticate_password(&profile.username, pw)
                .await
                .map_err(|e| {
                    connect_error(
                        "AUTH_FAILED",
                        format!("认证阶段失败：{}", profile.username),
                        Some(e.to_string()),
                    )
                })?
        }
        AuthMethod::PrivateKey | AuthMethod::PrivateKeyWithPassphrase => {
            let key_text = resolved
                .private_key
                .as_deref()
                .ok_or_else(|| connect_error("MISSING_CREDENTIAL", "缺少私钥内容".into(), None))?;
            let key = russh::keys::PrivateKey::from_openssh(key_text).map_err(|e| {
                connect_error(
                    "KEY_PARSE_FAILED",
                    "私钥解析失败（内容不是有效的 OpenSSH 私钥）".into(),
                    Some(e.to_string()),
                )
            })?;
            // 加密私钥：先按 passphrase 解密（无 passphrase 时直接使用）
            let key = match (resolved.passphrase.as_deref(), profile.auth_method) {
                (Some(pp), AuthMethod::PrivateKeyWithPassphrase) => {
                    key.decrypt(pp).map_err(|e| {
                        connect_error(
                            "KEY_PASSPHRASE_INVALID",
                            "私钥口令错误或私钥损坏".into(),
                            Some(e.to_string()),
                        )
                    })?
                }
                _ => key,
            };
            session
                .authenticate_publickey(
                    &profile.username,
                    russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None),
                )
                .await
                .map_err(|e| {
                    connect_error(
                        "AUTH_FAILED",
                        format!("认证阶段失败：{}", profile.username),
                        Some(e.to_string()),
                    )
                })?
        }
    };

    if !auth.success() {
        return Err(connect_error(
            "AUTH_FAILED",
            format!("认证失败：用户名或凭证错误（{}）", profile.username),
            None,
        ));
    }
    emit_stage(app, request_id, &profile.id, "auth", "ok", None);
    emit_stage(app, request_id, &profile.id, "session", "ok", None);
    Ok((std::sync::Arc::new(session), forward_targets))
}

/// 已解析的 SSH 连接参数。Vault 路径在 Rust 内取出明文，秘密不回传前端。
pub(crate) struct ResolvedConnectPayload {
    /// 解析后实际用于认证的服务器配置（Vault 用户名覆盖表单展示值）。
    pub(crate) profile: ServerProfile,
    /// 密码认证秘密。
    password: Option<String>,
    /// OpenSSH 私钥原文。
    private_key: Option<String>,
    /// 加密私钥的可选口令。
    passphrase: Option<String>,
    /// 主机密钥交互槽（连接流程与握手回调共享）。
    verify: Arc<HostKeyVerifySlot>,
}

/// 将 Vault 凭据映射为 SSH 认证参数；凭据中的用户名优先于 profile 展示值。
fn apply_vault_credential(
    mut profile: ServerProfile,
    credential: Credential,
) -> Result<ResolvedConnectPayload, String> {
    let verify = Arc::new(HostKeyVerifySlot::default());
    let resolved = match (profile.auth_method, credential.fields) {
        (AuthMethod::Password, CredentialFields::Password { username, password }) => {
            profile.username = username;
            ResolvedConnectPayload {
                profile,
                password: Some(password),
                private_key: None,
                passphrase: None,
                verify,
            }
        }
        (
            AuthMethod::PrivateKey | AuthMethod::PrivateKeyWithPassphrase,
            CredentialFields::SshKey {
                username,
                private_key,
                passphrase,
            },
        ) => {
            if profile.auth_method == AuthMethod::PrivateKeyWithPassphrase
                && passphrase.as_deref().unwrap_or_default().is_empty()
            {
                return Err("所选 SSH 私钥凭证缺少 Passphrase".into());
            }
            profile.username = username;
            ResolvedConnectPayload {
                profile,
                password: None,
                private_key: Some(private_key),
                passphrase,
                verify,
            }
        }
        (AuthMethod::Password, _) => return Err("所选 Vault 凭证不是“用户名密码”类型".into()),
        (AuthMethod::PrivateKey | AuthMethod::PrivateKeyWithPassphrase, _) => {
            return Err("所选 Vault 凭证不是“SSH 私钥”类型".into())
        }
    };
    Ok(resolved)
}

/// 解析连接凭证：一次性覆盖优先，其次 Vault 引用；都没有则报 MISSING_CREDENTIAL。
/// （旧版手工凭证已在首次导入时迁移进 Vault，SSH 插件不再保存明文副本。）
fn resolve_credentials(
    app: &AppHandle,
    profile: &ServerProfile,
    overrides: Option<CredentialOverride>,
) -> Result<ResolvedConnectPayload, SshConnectError> {
    if let Some(ov) = overrides.filter(|o| {
        !o.password.as_deref().unwrap_or_default().is_empty()
            || !o.private_key.as_deref().unwrap_or_default().is_empty()
    }) {
        return Ok(ResolvedConnectPayload {
            profile: profile.clone(),
            password: ov.password,
            private_key: ov.private_key,
            passphrase: ov.passphrase,
            verify: Arc::new(HostKeyVerifySlot::default()),
        });
    }
    let credential_ref = profile
        .credential_ref
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            connect_error(
                "MISSING_CREDENTIAL",
                format!(
                    "服务器「{}」尚未保存凭证：请编辑服务器保存凭证后连接",
                    profile.name
                ),
                None,
            )
        })?;
    let credential = vault::resolve(app, credential_ref).map_err(|e| {
        connect_error(
            "CREDENTIAL_UNAVAILABLE",
            format!("读取 Vault 凭证失败: {e}"),
            None,
        )
    })?;
    apply_vault_credential(profile.clone(), credential)
        .map_err(|e| connect_error("CREDENTIAL_MISMATCH", e, None))
}

/// 从 outcome 组装连接成功快照并登记会话
fn register_session(
    state: &SshState,
    profile: &ServerProfile,
    session: std::sync::Arc<client::Handle<SshHandler>>,
    forward_targets: ForwardTargets,
) -> String {
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
    if let Ok(mut map) = state.0.lock() {
        map.insert(session_id.clone(), handle);
    }
    session_id
}

/// 建立连接（ssh_connect 命令）：按 profileId 从插件库取配置，凭证在 Rust 侧解析
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_connect(
    app: AppHandle,
    state: State<'_, SshState>,
    profile_state: State<'_, ProfileState>,
    payload: SshConnectPayload,
) -> Result<SshConnectOutcome, String> {
    let request_id = resource_id("sshc");
    // 从插件库读配置
    let profile = {
        let profile_id = payload.profile_id.clone();
        store::with_db(&app, &profile_state, |c| store::get_profile(c, &profile_id))
    };
    let profile = match profile {
        Ok(p) => p,
        Err(_) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(connect_error(
                    "PROFILE_NOT_FOUND",
                    format!("服务器配置不存在（{}）", payload.profile_id),
                    None,
                )),
            })
        }
    };
    // 解析凭证（一次性覆盖 > Vault 引用）
    let resolved = match resolve_credentials(&app, &profile, payload.overrides) {
        Ok(r) => r,
        Err(e) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(e),
            })
        }
    };
    let known_hosts = host_keys::known_hosts_file(&app)?;
    let opened = open_session(&app, &request_id, &resolved, known_hosts).await;
    let (session, forward_targets) = match opened {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(e),
            })
        }
    };

    let session_id = register_session(&state, &profile, session, forward_targets);
    let _ = store::with_db(&app, &profile_state, |c| {
        store::touch_last_connected(c, &profile.id, now_ms() as i64)
    });

    let conn = ServerConnection {
        profile_id: profile.id.clone(),
        session_id: session_id.clone(),
        status: ConnectionStatus::Connected,
        host: Some(profile.host.clone()),
        latency_ms: None,
        error: None,
        connected_at: Some(now_ms()),
    };
    resume_for_profile(&app, profile_state.inner(), &profile.id, &session_id);
    app.emit("ssh://connection-status", &conn)
        .map_err(|e| e.to_string())?;
    Ok(SshConnectOutcome {
        ok: true,
        connection: Some(conn),
        request_id,
        error: None,
    })
}

/// 断开连接并清理会话
#[tauri::command]
pub async fn ssh_disconnect(
    app: AppHandle,
    state: State<'_, SshState>,
    terminal_state: State<'_, TerminalState>,
    monitor_state: State<'_, MonitorState>,
    tunnel_state: State<'_, crate::plugins::ssh::tunnel::TunnelState>,
    session_id: String,
) -> Result<SshActionResult, String> {
    stop_session_tunnels(&tunnel_state, &session_id);
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

/// 重新连接：配置与凭证由后端按 profileId 解析，前端无需再传任何秘密
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_reconnect(
    app: AppHandle,
    state: State<'_, SshState>,
    profile_state: State<'_, ProfileState>,
    session_id: String,
    overrides: Option<CredentialOverride>,
) -> Result<SshConnectOutcome, String> {
    let request_id = resource_id("sshc");
    // 先取旧句柄的 profileId（句柄保留在表中，失败时不影响旧连接）
    let profile_id = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        map.get(&session_id).map(|h| h.profile_id.clone())
    };
    let Some(profile_id) = profile_id else {
        return Err("连接不存在或已断开，无法重连".into());
    };
    let profile = {
        let pid = profile_id.clone();
        store::with_db(&app, &profile_state, |c| store::get_profile(c, &pid))
    };
    let profile = match profile {
        Ok(p) => p,
        Err(_) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(connect_error(
                    "PROFILE_NOT_FOUND",
                    format!("服务器配置不存在（{}）", profile_id),
                    None,
                )),
            })
        }
    };
    let resolved = match resolve_credentials(&app, &profile, overrides) {
        Ok(r) => r,
        Err(e) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(e),
            })
        }
    };
    let known_hosts = host_keys::known_hosts_file(&app)?;
    let opened = open_session(&app, &request_id, &resolved, known_hosts).await;
    let (session, forward_targets) = match opened {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(e),
            })
        }
    };

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
                sftp: Mutex::new(None),
                forward_targets,
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
    resume_for_profile(&app, profile_state.inner(), &profile.id, &new_id);
    app.emit("ssh://connection-status", &conn)
        .map_err(|e| e.to_string())?;
    Ok(SshConnectOutcome {
        ok: true,
        connection: Some(conn),
        request_id,
        error: None,
    })
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::framework::vault::models::CredentialKind;
    use crate::framework::vault::{Credential, CredentialFields};
    use crate::plugins::ssh::models::{AuthMethod, ServerProfile};

    use super::{apply_vault_credential, resource_id, shell_quote};

    fn profile(auth_method: AuthMethod) -> ServerProfile {
        ServerProfile {
            id: "p1".into(),
            name: "测试服务器".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "manual-user".into(),
            auth_method,
            credential_ref: Some("vault-1".into()),
            group_id: None,
            remark: None,
            last_connected_at: None,
        }
    }

    fn credential(kind: CredentialKind, fields: CredentialFields) -> Credential {
        Credential {
            id: "vault-1".into(),
            name: "测试凭据".into(),
            kind,
            fields,
            note: String::new(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn shell_quote_keeps_single_quotes() {
        assert_eq!(shell_quote("nginx.service"), "'nginx.service'");
        assert_eq!(
            shell_quote("a'; touch /tmp/pwn; echo '"),
            "'a'\"'\"'; touch /tmp/pwn; echo '\"'\"''"
        );
    }

    #[test]
    fn resource_ids_do_not_collide() {
        let ids = (0..1000)
            .map(|_| resource_id("test"))
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn vault_password_overrides_profile_username() {
        let resolved = apply_vault_credential(
            profile(AuthMethod::Password),
            credential(
                CredentialKind::Password,
                CredentialFields::Password {
                    username: "vault-user".into(),
                    password: "secret".into(),
                },
            ),
        )
        .unwrap();
        assert_eq!(resolved.profile.username, "vault-user");
        assert_eq!(resolved.password.as_deref(), Some("secret"));
        assert!(resolved.private_key.is_none());
    }

    #[test]
    fn vault_auth_type_must_match_profile() {
        let result = apply_vault_credential(
            profile(AuthMethod::Password),
            credential(
                CredentialKind::SshKey,
                CredentialFields::SshKey {
                    username: "root".into(),
                    private_key: "key".into(),
                    passphrase: None,
                },
            ),
        );
        assert!(result.is_err());
    }
}
