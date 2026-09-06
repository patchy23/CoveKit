//! SSH 插件 · 建连（分阶段事件 + 结构化错误 + 凭证解析）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>),
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。
//! 连接流程：分阶段事件（ssh://connect-stage）+ 稳定错误码（SshConnectError）；
//! 主机密钥校验在握手回调中等待前端人工确认（ssh://host-key-verify ↔ ssh_host_key_respond），
//! 首连不再静默记录 TOFU，指纹变更必须经用户明确决定（仅本次 / 保存 / 替换 / 取消）。

use std::{path::PathBuf, sync::Arc};

use russh::client;
use tauri::{AppHandle, Emitter, State};

use crate::framework::vault::{self, Credential, CredentialFields};
use crate::plugins::ssh::conn::handler::{HostKeyVerifySlot, SshHandler};
use crate::plugins::ssh::conn::{now_ms, resource_id, SshState};
use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{
    AuthMethod, ConnectionStatus, CredentialOverride, ServerConnection, ServerProfile,
    SshConnectError, SshConnectOutcome, SshConnectPayload,
};
use crate::plugins::ssh::store::{self, ProfileState};
use crate::plugins::ssh::tunnel::{new_forward_targets, resume_for_profile, ForwardTargets};

use super::{connect_error, emit_stage, register_session, CONNECT_PHASE_TIMEOUT};

/// 认证并建立连接（ssh_connect / ssh_reconnect 共用）。
/// 阶段：resolve → tcp → handshake（含 verify 回调）→ auth；每阶段推送事件。
pub(crate) async fn open_session(
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
pub(crate) fn resolve_credentials(
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

    let session_id = match register_session(&state, &profile, session, forward_targets) {
        Ok(id) => id,
        Err(e) => {
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(e),
            })
        }
    };
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
