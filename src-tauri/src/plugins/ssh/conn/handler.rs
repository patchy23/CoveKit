//! SSH 插件 · russh 客户端 Handler（主机密钥人工确认交互）
//! 会话模型：SshState(Mutex<HashMap<session_id, SshSessionHandle>>),
//! 与 http_ws 的 WsState 同构；终端/文件等通道从会话句柄上按需开启。
//! 状态变化经事件 ssh://connection-status 推送到前端。
//! 连接流程：分阶段事件（ssh://connect-stage）+ 稳定错误码（SshConnectError）；
//! 主机密钥校验在握手回调中等待前端人工确认（ssh://host-key-verify ↔ ssh_host_key_respond），
//! 首连不再静默记录 TOFU，指纹变更必须经用户明确决定（仅本次 / 保存 / 替换 / 取消）。

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::io::copy_bidirectional;

use russh::client;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::plugins::ssh::conn::HostKeyState;
use crate::plugins::ssh::host_keys;
use crate::plugins::ssh::models::{HostKeyDecision, HostKeyVerifyRequest, SshConnectError};
use crate::plugins::ssh::tunnel::ForwardTargets;

/// 校验阶段存储失败的错误码常量
pub(crate) const HOST_KEY_STORE_FAILED: &str = "HOST_KEY_STORE_FAILED";

use super::HOST_KEY_WAIT;

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
    pub(crate) host: String,
    /// 目标 SSH 端口。
    pub(crate) port: u16,
    /// patchyBox 私有 known_hosts 文件。
    pub(crate) known_hosts_path: PathBuf,
    /// 事件推送句柄（host-key-verify / connect-stage）。
    pub(crate) app: AppHandle,
    /// 本次连接尝试的请求 id。
    pub(crate) request_id: String,
    /// 交互槽（接收端 + 结构化失败）。
    pub(crate) verify: Arc<HostKeyVerifySlot>,
    /// -R 远程转发的目标表（tunnel 模块注册，回调消费）。
    pub(crate) forward_targets: ForwardTargets,
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
