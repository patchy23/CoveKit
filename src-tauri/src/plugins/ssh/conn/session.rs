//! SSH 插件 · 会话建立（分阶段连接），供 connect/reconnect 命令复用

use std::{path::PathBuf, sync::Arc};

use russh::client;
use tauri::AppHandle;

use super::connect::ResolvedConnectPayload;
use crate::plugins::ssh::conn::handler::SshHandler;
use crate::plugins::ssh::models::{AuthMethod, SshConnectError};
use crate::plugins::ssh::tunnel::{new_forward_targets, ForwardTargets};

use super::{connect_error, emit_stage, CONNECT_PHASE_TIMEOUT};

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
