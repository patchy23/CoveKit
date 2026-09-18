//! SSH 数据模型 · 主机密钥校验（已知主机 / 确认请求 / 用户决定）

use serde::{Deserialize, Serialize};

/* ── 主机密钥校验（首连确认 / 变更阻断） ── */

/// 已知主机条目（来自 CoveKit 私有 known_hosts 文件）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostEntry {
    /// 主机地址
    pub(crate) host: String,
    /// 端口
    pub(crate) port: u16,
    /// 公钥算法名（如 ssh-ed25519）
    pub(crate) algorithm: String,
    /// SHA256 指纹（SHA256:base64）
    pub(crate) fingerprint: String,
}

/// 主机密钥人工确认请求（后端在握手回调中推送，等待前端 ssh_host_key_respond）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyVerifyRequest {
    /// 本次连接尝试的唯一请求 id
    pub(crate) request_id: String,
    /// 类型：unknown = 首次连接；mismatch = 与已保存指纹不一致
    pub(crate) kind: String,
    /// 主机地址
    pub(crate) host: String,
    /// 端口
    pub(crate) port: u16,
    /// 公钥算法名
    pub(crate) algorithm: String,
    /// 服务器公钥 SHA256 指纹
    pub(crate) fingerprint: String,
    /// kind=mismatch 时已保存的指纹列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) saved_fingerprints: Vec<String>,
}

/// 用户对主机密钥的决定（respond 命令入参）
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HostKeyDecision {
    /// 仅本次信任（不写入 known_hosts）
    TrustOnce,
    /// 保存并连接（写入 known_hosts）
    TrustSave,
    /// 取消连接
    Cancel,
    /// 仅 kind=mismatch：确认替换已保存指纹（前端已二次确认）
    Replace,
}
