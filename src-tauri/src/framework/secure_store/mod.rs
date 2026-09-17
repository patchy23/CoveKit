//! 框架凭证安全原语（T05-1）：加解密 / 主密钥来源 / 原子写与备份恢复 / 文件权限 / 保护状态
//!
//! 一个「域」= 一个主密钥 + 一组密文文件。Vault（凭证条目模型）与 `credentials`
//! （兼容 KV 命名空间）只在此共用底层原语，业务数据格式各自保留。
//!
//! - 加解密：`crypto`（AES-256-GCM，`nonce(12B)‖ciphertext`，解密先校验最小长度；
//!   带 AAD 变体供数据包容器把头字节绑进认证，`derive_key_argon2id` 为框架唯一口令派生）
//! - 主密钥：`key`（系统密钥库 → 本地降级密钥文件 → 首次生成；**有既有密文时以密文为准**）
//! - 文件：`file`（唯一临时名 + fsync + 旧文件转 `.bak`；读路径按「认证 + 解析」择版恢复）
//! - 保护状态：`status`（`system-keyring` / `file-fallback` / `unavailable`，供设置页展示）
//! - 平台权限：`win_acl`（Windows DACL 收紧到当前用户）；其它平台见 `file::restrict_to_current_user`
//!
//! 安全边界：**密钥与密文同目录时，整目录被拷走仍可被离线解密**。本模块防止的是「凭证明文落盘、
//! 单个文件泄漏」，不防「已登录当前系统账户的恶意进程」，也不提供抗离线解密能力。

mod crypto;
mod file;
mod key;
#[cfg(test)]
mod keyring_probe;
mod status;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(windows)]
mod win_acl;

pub(crate) use crypto::{
    decrypt_with_aad_nonce, derive_key_argon2id, encrypt_with_aad, encrypt_with_aad_nonce,
};
pub(crate) use file::{backup_path, ciphertext_evidence, load_verified, replace_file};
pub(crate) use key::{
    keyring_store_for, native_backend_available, resolve_master_key, MasterKeyStore,
    ScopedKeyringStore, CREDENTIALS_KEY_SPEC, VAULT_KEY_SPEC,
};
pub(crate) use status::inspect_domain;
pub use status::ProtectionStatus;

/// 仅测试构建可见：uid 绑定格式的认证校验（构造夹具用）
#[cfg(test)]
pub(crate) use crypto::authenticates_with_aad;
/// 仅测试构建可见：清掉「降级密钥已登记」的进程内记录（断言登记行为的用例用）
#[cfg(test)]
pub(crate) use key::reset_promotion_attempts;
/// 仅测试构建可见：串行化会进入「降级密钥登记」分支的用例（登记记录是进程级共享状态）
#[cfg(test)]
pub(crate) use test_support::promotion_test_guard;
/// 仅测试构建可见：写入降级密钥文件
#[cfg(test)]
pub(crate) use test_support::seed_fallback_file;
