//! SSH 数据模型 · 配置载荷（连接 / 保存 / 导入）

use super::common::ServerProfile;
use serde::Deserialize;

/* ── 连接请求载荷（前端 ssh_connect 入参） ── */

/// 一次性凭证覆盖（仅本次连接在内存中使用，不落任何存储）
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CredentialOverride {
    /// 密码（auth_method=Password 时使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 私钥内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
}

/// SSH 连接请求载荷：只传 profile id（后端自行读取配置并解析 Vault 凭证）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectPayload {
    /// 服务器配置 id
    pub(crate) profile_id: String,
    /// 一次性凭证覆盖（优先于已保存凭证；仅在内存中使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overrides: Option<CredentialOverride>,
}

/// 服务器保存载荷：配置 + 可选手工凭证（勾选保存时写入 Vault，返回带 credentialRef 的记录）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfileSavePayload {
    /// 服务器配置
    pub(crate) profile: ServerProfile,
    /// 保存到 Vault 的密码（手工密码认证时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 保存到 Vault 的私钥内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
    /// 是否把本次输入的凭证保存到 Vault（false = 仅更新配置，凭证保持原引用）
    #[serde(default)]
    pub(crate) save_credential: bool,
}
