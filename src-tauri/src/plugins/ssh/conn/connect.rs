//! SSH 插件 · 凭证解析与 ssh_connect 命令

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::framework::vault::{self, Credential, CredentialFields};
use crate::plugins::ssh::conn::handler::HostKeyVerifySlot;
use crate::plugins::ssh::conn::{connect_error, now_ms, register_session, resource_id, SshState};
use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{
    AuthMethod, ConnectionStatus, CredentialOverride, ServerConnection, ServerProfile,
    SshConnectError, SshConnectOutcome, SshConnectPayload,
};
use crate::plugins::ssh::store::{self, ProfileState};
use crate::plugins::ssh::tunnel::resume_for_profile;

use super::session::open_session;
/// 已解析的 SSH 连接参数（凭证在 Rust 内解密，秘密不回传前端）
pub(crate) struct ResolvedConnectPayload {
    /// 解析后实际用于认证的服务器配置（Vault 用户名覆盖表单展示值）。
    pub(crate) profile: ServerProfile,
    /// 密码认证秘密。
    pub(crate) password: Option<String>,
    /// OpenSSH 私钥原文。
    pub(crate) private_key: Option<String>,
    /// 加密私钥的可选口令。
    pub(crate) passphrase: Option<String>,
    /// 主机密钥交互槽（连接流程与握手回调共享）。
    pub(crate) verify: Arc<HostKeyVerifySlot>,
}

/// 将 Vault 凭据映射为 SSH 认证参数；凭据中的用户名优先于 profile 展示值。
pub(crate) fn apply_vault_credential(
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

/// 解析连接凭证：一次性覆盖优先；随后按配置读取 Vault 或本地认证，无认证则报错。
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
    if profile.credential_ref.is_none() {
        let local = store::with_db(app, &app.state::<ProfileState>(), |conn| {
            store::local_auth::get(conn, profile)
        })
        .map_err(|e| connect_error("LOCAL_AUTH_UNAVAILABLE", e, None))?;
        if let Some(local) = local {
            return Ok(ResolvedConnectPayload {
                profile: profile.clone(),
                password: local.password,
                private_key: local.private_key,
                passphrase: local.passphrase,
                verify: Arc::new(HostKeyVerifySlot::default()),
            });
        }
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
                    "服务器「{}」缺少认证信息：请输入密码或私钥，也可选择已保存的凭证",
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
    let log_started = std::time::Instant::now();
    let result: Result<SshConnectOutcome, String> = async {
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
        // 解析一次性覆盖、Vault 引用或本地认证。
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
        // 事件推送失败（如窗口已关闭）只记诊断日志：会话已注册、连接可用，
        // 返回 Err 会让前端误判连接失败，且 session_id 未回传形成无法断开的幽灵会话
        if let Err(e) = app.emit("ssh://connection-status", &conn) {
            log::warn!(
                "连接状态事件推送失败（连接本身已成功）: {e_type}",
                e_type = std::any::type_name_of_val(&e)
            );
        }
        Ok(SshConnectOutcome {
            ok: true,
            connection: Some(conn),
            request_id,
            error: None,
        })
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=ssh_connect request={} elapsed_ms={}",
            value.request_id,
            log_started.elapsed().as_millis()
        ),
        Ok(value) => {
            let code = value
                .error
                .as_ref()
                .map(|error| error.code.as_str())
                .unwrap_or("UNKNOWN");
            if code == "CANCELLED" {
                log::info!(
                    "连接已取消 operation=ssh_connect request={}",
                    value.request_id
                );
            } else {
                log::warn!(
                    "连接未建立 operation=ssh_connect request={} code={code} elapsed_ms={}",
                    value.request_id,
                    log_started.elapsed().as_millis()
                );
            }
        }
        Err(_) => log::error!(
            "连接准备失败 operation=ssh_connect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
