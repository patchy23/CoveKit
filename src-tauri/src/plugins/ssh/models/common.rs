//! SSH 数据模型 · 通用（操作结果 / 认证方式 / 服务器配置 / 分组 / 连接快照）

use serde::{Deserialize, Serialize};

/* ── 通用 ── */

/// 操作结果（失败时 ok=false + error，不抛错给前端展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshActionResult {
    /// 操作是否成功
    pub(crate) ok: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 认证方式（密码 / 私钥 / 私钥+passphrase）
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    /// 用户名+密码
    Password,
    /// 用户名+私钥（无 passphrase）
    PrivateKey,
    /// 用户名+私钥+passphrase
    PrivateKeyWithPassphrase,
}

/// 服务器连接配置（凭证只存公共 Vault 的 credentialRef 引用，秘密永不入库/不落 profile）
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    /// 唯一 id（profile-<毫秒时间戳>）
    pub(crate) id: String,
    /// 显示名称（如「生产服务器」）
    pub(crate) name: String,
    /// 主机地址
    pub(crate) host: String,
    /// SSH 端口（默认 22）
    pub(crate) port: u16,
    /// 登录用户名
    pub(crate) username: String,
    /// 认证方式
    pub(crate) auth_method: AuthMethod,
    /// 公共 Vault 凭证引用；为空表示尚未保存凭证（连接时需要一次性凭证）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) credential_ref: Option<String>,
    /// 所属分组 id（为空 = 未分组；分组实体在 ssh_groups 表）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_id: Option<String>,
    /// 备注
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remark: Option<String>,
    /// 最后连接时间（毫秒时间戳）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_connected_at: Option<u64>,
}

/// 服务器分组
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshGroup {
    /// 唯一 id（group-<毫秒时间戳>）
    pub(crate) id: String,
    /// 分组名称
    pub(crate) name: String,
    /// 排序权重（创建顺序自增）
    pub(crate) sort_order: i64,
}

/// 目录书签（文件页签底栏；按服务器 profile 隔离）
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshBookmark {
    /// 唯一 id（bm-<毫秒时间戳>）
    pub(crate) id: String,
    /// 所属服务器配置 id
    pub(crate) profile_id: String,
    /// 显示名（默认目录名）
    pub(crate) name: String,
    /// 远程目录绝对路径
    pub(crate) path: String,
    /// 排序权重（追加自增）
    pub(crate) sort: i64,
}

/// 服务器连接状态
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 错误/断开
    Error,
    /// 重连中
    Reconnecting,
}

/// 服务器连接快照（侧栏列表展示用）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConnection {
    /// 关联的 ServerProfile id
    pub(crate) profile_id: String,
    /// 会话唯一 id（conn-<毫秒时间戳>）
    pub(crate) session_id: String,
    /// 当前状态
    pub(crate) status: ConnectionStatus,
    /// 已连接的服务器地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<String>,
    /// 延迟毫秒（ping 或 SSH 握手耗时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latency_ms: Option<u64>,
    /// 错误信息（status=error 时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// 建立连接的毫秒时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) connected_at: Option<u64>,
}
