//! SSH 插件 · 重连 / 断开 / 会话快照命令
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>),
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。
//! 连接流程：分阶段事件（ssh://connect-stage）+ 稳定错误码（SshConnectError）；
//! 主机密钥校验在握手回调中等待前端人工确认（ssh://host-key-verify ↔ ssh_host_key_respond），
//! 首连不再静默记录 TOFU，指纹变更必须经用户明确决定（仅本次 / 保存 / 替换 / 取消）。

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::plugins::ssh::conn::{now_ms, resource_id, SshSessionHandle, SshState};
use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{
    ConnectionStatus, CredentialOverride, ServerConnection, SshActionResult, SshConnectOutcome,
};
use crate::plugins::ssh::monitor::MonitorState;
use crate::plugins::ssh::store::{self, ProfileState};
use crate::plugins::ssh::terminal::TerminalState;
use crate::plugins::ssh::tunnel::{resume_for_profile, stop_session_tunnels};

use super::connect::resolve_credentials;
use super::connect_error;
use super::session::open_session;

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
    let log_started = std::time::Instant::now();
    let result: Result<SshActionResult, String> = async {
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
                // 断开动作已完成；事件推送失败只记诊断日志，不把已断开误报为失败
                .unwrap_or_else(|e| {
                    log::warn!(
                        "断开事件推送失败: {e_type}",
                        e_type = std::any::type_name_of_val(&e)
                    )
                });
        }
        Ok(SshActionResult {
            ok: true,
            error: None,
        })
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=ssh_disconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=ssh_disconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=ssh_disconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
    let log_started = std::time::Instant::now();
    let result: Result<SshConnectOutcome, String> = async {
        // 终端/监控/隧道的 State 走 app.state 内部获取（命令参数过多触发 clippy 8/7）
        let terminal_state = app.state::<TerminalState>();
        let monitor_state = app.state::<MonitorState>();
        let tunnel_state = app.state::<crate::plugins::ssh::tunnel::TunnelState>();
        let request_id = resource_id("sshc");
        // 先取旧句柄的 profileId（句柄保留在表中，失败时不影响旧连接）
        let profile_id = {
            let map = state.0.lock().map_err(|e| e.to_string())?;
            map.get(&session_id).map(|h| h.profile_id.clone())
        };
        let Some(profile_id) = profile_id else {
            // 结构化返回（与其他失败路径一致），前端据此停止重连而非反复报 IPC 错
            return Ok(SshConnectOutcome {
                ok: false,
                connection: None,
                request_id,
                error: Some(connect_error(
                    "SESSION_NOT_FOUND",
                    "连接不存在或已断开，无法重连".into(),
                    None,
                )),
            });
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
                    id_names: Mutex::new(None),
                },
            );
            old
        };
        if let Some(h) = old {
            // 旧会话资源随重连一并清理：终端任务取消、监控采样移除、旧隧道停置（desired 保留供恢复）
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
            monitor_state
                .0
                .lock()
                .map_err(|e| e.to_string())?
                .remove(&session_id);
            crate::plugins::ssh::tunnel::stop_session_tunnels(&tunnel_state, &session_id);
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
        // 与 ssh_connect 同理：推送失败只记日志，新会话已注册，不能误判失败产生幽灵会话
        if let Err(e) = app.emit("ssh://connection-status", &conn) {
            log::warn!(
                "重连状态事件推送失败（连接本身已成功）: {e_type}",
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
            "操作完成 operation=ssh_reconnect request={} elapsed_ms={}",
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
                    "连接已取消 operation=ssh_reconnect request={}",
                    value.request_id
                );
            } else {
                log::warn!(
                    "连接未建立 operation=ssh_reconnect request={} code={code} elapsed_ms={}",
                    value.request_id,
                    log_started.elapsed().as_millis()
                );
            }
        }
        Err(_) => log::error!(
            "连接准备失败 operation=ssh_reconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::framework::vault::models::CredentialKind;
    use crate::framework::vault::{Credential, CredentialFields};
    use crate::plugins::ssh::models::{AuthMethod, ServerProfile};

    use crate::plugins::ssh::conn::connect::apply_vault_credential;
    use crate::plugins::ssh::conn::{resource_id, shell_quote};

    fn profile(auth_method: AuthMethod) -> ServerProfile {
        ServerProfile {
            id: "p1".into(),
            name: "测试服务器".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "manual-user".into(),
            auth_method,
            credential_ref: Some("vault-1".into()),
            has_local_auth: false,
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
