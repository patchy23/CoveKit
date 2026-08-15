//! 数据模型：凭证条目 / 类型枚举 / serde tagged 字段 / 命令出入参结构
//! 与前端 src/core/ipc/contracts.ts 的 Vault 段逐字段同步（camelCase）。
//! 字段结构用 serde 内部标签 `type` 按 kind 分派（kebab-case 类型串）。

use serde::{Deserialize, Serialize};

/// 凭证类型（第一版 5 种，覆盖现有与规划工具；数据库凭证复用 Password 不单独设类型）
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    /// 用户名 + 密码（数据库、SSH 密码登录、HTTP Basic）
    Password,
    /// 用户名 + 私钥 + 可选 passphrase（SSH 密钥登录）
    SshKey,
    /// 单 token（HTTP 调试 Bearer、开放平台 token）
    ApiToken,
    /// AccessKey 对（阿里云、腾讯云 CAM、CF API）
    AccessKeyPair,
    /// 任意键值对（兜底结构）
    Custom,
}

/// 自定义键值条目（custom 类型 fields 的元素）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEntry {
    /// 键名（非空）
    pub key: String,
    /// 值
    pub value: String,
    /// 是否秘密值（true = 列表掩码、复制走 reveal）
    pub secret: bool,
}

/// 凭证秘密字段（serde 内部标签 `type` 按 kind 分派；各变体字段 camelCase）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum CredentialFields {
    /// 用户名 + 密码
    #[serde(rename = "password", rename_all = "camelCase")]
    Password {
        /// 用户名
        username: String,
        /// 密码（秘密）
        password: String,
    },
    /// SSH 私钥登录
    #[serde(rename = "ssh-key", rename_all = "camelCase")]
    SshKey {
        /// 登录用户名
        username: String,
        /// 私钥内容（秘密，PEM/OpenSSH 格式原文）
        private_key: String,
        /// 私钥口令（可选，秘密）
        passphrase: Option<String>,
    },
    /// 单 token
    #[serde(rename = "api-token", rename_all = "camelCase")]
    ApiToken {
        /// token 值（秘密）
        token: String,
    },
    /// AccessKey 对
    #[serde(rename = "access-key-pair", rename_all = "camelCase")]
    AccessKeyPair {
        /// AccessKey ID
        access_key_id: String,
        /// AccessKey Secret（秘密）
        access_key_secret: String,
    },
    /// 任意键值对
    #[serde(rename = "custom", rename_all = "camelCase")]
    Custom {
        /// 键值条目数组
        entries: Vec<CustomEntry>,
    },
}

impl CredentialFields {
    /// 字段结构对应的凭证类型（保存时以此为准回填 kind，保证二者一致）
    pub fn kind(&self) -> CredentialKind {
        match self {
            CredentialFields::Password { .. } => CredentialKind::Password,
            CredentialFields::SshKey { .. } => CredentialKind::SshKey,
            CredentialFields::ApiToken { .. } => CredentialKind::ApiToken,
            CredentialFields::AccessKeyPair { .. } => CredentialKind::AccessKeyPair,
            CredentialFields::Custom { .. } => CredentialKind::Custom,
        }
    }
}

/// 凭证条目（vault.dat 解密后的数组元素；引用方 profile 只存 id）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    /// uuid v4，引用方只存这个
    pub id: String,
    /// 显示名（如「生产 MySQL」「腾讯云 CAM」）
    pub name: String,
    /// 类型（冗余存储便于列表直读；与 fields 的 type 标签一致）
    pub kind: CredentialKind,
    /// 秘密字段（按 kind 分派）
    pub fields: CredentialFields,
    /// 备注（可空）
    pub note: String,
    /// 创建时间（秒级时间戳）
    pub created_at: i64,
    /// 更新时间（秒级时间戳）
    pub updated_at: i64,
}

/// 凭证脱敏摘要（vault_list 返回项；永远不含明文秘密）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSummary {
    /// 凭证 id
    pub id: String,
    /// 显示名
    pub name: String,
    /// 类型
    pub kind: CredentialKind,
    /// 掩码摘要（如 AKI****xyz / 用户名 / 「N 个字段」）
    pub masked: String,
    /// 备注
    pub note: String,
    /// 创建时间（秒级时间戳）
    pub created_at: i64,
    /// 更新时间（秒级时间戳）
    pub updated_at: i64,
}

/// vault_save 入参（payload 结构体打包；id 可选 = upsert）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSavePayload {
    /// 凭证 id（None = 新增；Some = 更新对应条目）
    pub id: Option<String>,
    /// 显示名（非空）
    pub name: String,
    /// 类型（必须与 fields 的 type 标签一致）
    pub kind: CredentialKind,
    /// 字段值（按 kind 分派的 tagged enum）
    pub fields: CredentialFields,
    /// 备注（可空）
    #[serde(default)]
    pub note: String,
}

/// vault_delete 返回（被引用计数供前端删除前提示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultDeleteResult {
    /// 是否删除成功
    pub ok: bool,
    /// 错误信息（无则 None）
    pub error: Option<String>,
    /// 仍引用该凭证的插件 profile 数量（引用扫描随设计 §6 迁移接入，当前恒 0）
    pub referenced_by: usize,
}

/// vault_import 返回
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultImportResult {
    /// 实际写入条数
    pub imported: usize,
    /// 合并模式下因同 id 冲突跳过的条数（覆盖模式恒 0）
    pub skipped: usize,
}
