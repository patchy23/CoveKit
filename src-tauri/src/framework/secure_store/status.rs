//! 凭证保护状态（T04-5）：把「主密钥实际存在哪里、密文到底有没有系统级保护」变成可查询的事实
//!
//! 动机：keyring 缺平台特性时会退回进程内 mock —— 写入返回成功、同进程回读也成功，用户与开发
//! 都看不出密文其实没有系统级保护。本模块不做猜测，只报告可核验的观测：本平台有没有原生后端、
//! 密钥库里有没有记录、降级密钥文件在不在、密文在不在、以及「以密文为准」的解析是否成功。
//!
//! 状态取值（契约见任务书 §6「凭证保护」）：
//! - `backend`：`system-keyring`（本次用的密钥来自系统密钥库）/ `file-fallback`（来自本地降级文件）
//!   / `unavailable`（本平台没有原生后端，或候选密钥都解不开既有密文）；
//! - `availability`：`available` / `locked`（密文在而密钥对不上）/ `uninitialized`（还没写过凭证）；
//! - `fallbackReason`：来自降级路径时的原因文案（无秘密，可直接展示）。
//!
//! 注意：状态查询自身**不生成主密钥**。空环境只报告「尚未初始化」，密钥在首次写入凭证时生成。

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::file::ciphertext_evidence;
use super::key::{
    native_backend_available, resolve_master_key_readonly, KeySource, KeySpec, MasterKeyStore,
};

/// 主密钥的实际保护后端
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectionBackend {
    /// 系统密钥库（Windows 凭据管理器 / macOS 钥匙串）
    SystemKeyring,
    /// 本地降级密钥文件（收紧到当前用户，但与密文同机同目录）
    FileFallback,
    /// 没有可用的系统密钥库后端；不能声称系统级保护
    Unavailable,
}

/// 该域的可用性
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectionAvailability {
    /// 能正常读写凭证
    Available,
    /// 密文存在而候选密钥都无法解密：锁定，等待备份恢复
    Locked,
    /// 还没有写入过凭证，首次保存时生成主密钥
    Uninitialized,
}

/// 单个数据域（vault / credentials 命名空间集合）的保护状态
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainProtection {
    /// 域名（`vault` / `credentials`）
    pub(crate) domain: String,
    /// 主密钥实际来源
    pub(crate) backend: ProtectionBackend,
    /// 可用性
    pub(crate) availability: ProtectionAvailability,
    /// 走降级路径/锁定时面向用户的原因（无秘密）
    pub(crate) fallback_reason: Option<String>,
    /// 系统密钥库里是否有该域的主密钥记录
    pub(crate) keyring_has_key: bool,
    /// 本地降级密钥文件是否存在
    pub(crate) fallback_file_exists: bool,
    /// 该域是否已有密文（主文件或备份）
    pub(crate) ciphertext_exists: bool,
}

/// 全量保护状态（设置页展示）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionStatus {
    /// 本平台是否编译进系统密钥库原生后端（false = 只能降级到文件，绝不显示为系统保护）
    pub(crate) native_backend: bool,
    /// 各数据域状态
    pub(crate) domains: Vec<DomainProtection>,
}

/// 检查单个数据域的保护状态。
///
/// `key_dir` 是该域主密钥（含降级密钥文件）所在目录，`ciphertext` 是该域的密文文件路径清单。
pub(crate) fn inspect_domain(
    domain: &str,
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    ciphertext: &[PathBuf],
    aad: &[u8],
) -> DomainProtection {
    let native = native_backend_available();
    let keyring_has_key = matches!(store.read(spec.account), Ok(Some(_)));
    let fallback_file_exists = key_dir.join(spec.fallback_file).exists();
    let evidence = ciphertext_evidence(ciphertext).unwrap_or_default();
    let ciphertext_exists = !evidence.is_empty();

    let base = DomainProtection {
        domain: domain.to_string(),
        backend: ProtectionBackend::Unavailable,
        availability: ProtectionAvailability::Uninitialized,
        fallback_reason: None,
        keyring_has_key,
        fallback_file_exists,
        ciphertext_exists,
    };

    // 尚未初始化：密钥库里没记录、降级文件也没有、密文也没有。此处不生成密钥（查询不该有写副作用）
    if !ciphertext_exists && !keyring_has_key && !fallback_file_exists {
        return DomainProtection {
            backend: if native {
                ProtectionBackend::SystemKeyring
            } else {
                ProtectionBackend::Unavailable
            },
            fallback_reason: Some(if native {
                "尚未初始化：首次保存凭证时生成主密钥并存入系统密钥库".into()
            } else {
                unavailable_reason()
            }),
            ..base
        };
    }

    match resolve_master_key_readonly(key_dir, spec, store, &evidence, aad) {
        Ok(resolved) => {
            // 平台没有原生后端时，即使解析成功也必须如实显示为不可用（不能把 mock/文件说成系统密钥库）
            if !native {
                return DomainProtection {
                    backend: ProtectionBackend::Unavailable,
                    availability: ProtectionAvailability::Available,
                    fallback_reason: Some(unavailable_reason()),
                    ..base
                };
            }
            let (backend, reason) = match resolved.source {
                KeySource::Keyring | KeySource::CreatedNow => {
                    (ProtectionBackend::SystemKeyring, None)
                }
                KeySource::FallbackFile => (
                    ProtectionBackend::FileFallback,
                    Some(
                        resolved
                            .notes
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "主密钥来自本地降级密钥文件".into()),
                    ),
                ),
            };
            DomainProtection {
                backend,
                availability: ProtectionAvailability::Available,
                fallback_reason: reason,
                ..base
            }
        }
        Err(error) => DomainProtection {
            backend: ProtectionBackend::Unavailable,
            availability: ProtectionAvailability::Locked,
            fallback_reason: Some(error),
            ..base
        },
    }
}

/// 无原生后端时的统一说明（Linux 等平台：keyring 只有 mock，不能算系统保护）
fn unavailable_reason() -> String {
    "当前平台未编译系统密钥库后端（keyring mock 不算系统保护）：主密钥只能保存在本地密钥文件，请勿与密文一起复制到不可信设备".into()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::MemoryKeyStore;
    use super::*;
    use crate::framework::secure_store::{
        backup_path, seed_fallback_file, CREDENTIALS_KEY_SPEC, VAULT_KEY_SPEC,
    };

    /// 测试用临时目录
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "secure-store-status-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 空环境：报告「尚未初始化」，且状态查询不得生成任何密钥文件
    #[test]
    fn uninitialized_reports_without_creating_keys() {
        let dir = temp_dir("uninitialized");
        let store = MemoryKeyStore::new();
        let status = inspect_domain("vault", &dir, &VAULT_KEY_SPEC, &store, &[], &[]);
        assert_eq!(status.availability, ProtectionAvailability::Uninitialized);
        assert!(!status.fallback_file_exists);
        assert!(!status.ciphertext_exists);
        assert!(!dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        assert_eq!(store.write_attempts(), 0, "查询状态不应写密钥库");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥在系统密钥库 + 密文可解 → system-keyring/available
    #[test]
    fn keyring_key_reports_system_backend() {
        let dir = temp_dir("keyring");
        let store = MemoryKeyStore::new();
        let key = [0x31u8; 32];
        store.write(VAULT_KEY_SPEC.account, &key).unwrap();
        let enc = super::super::encrypt_with_aad(&key, b"[]", &[]).unwrap();
        std::fs::write(dir.join("vault.dat"), &enc).unwrap();

        let status = inspect_domain(
            "vault",
            &dir,
            &VAULT_KEY_SPEC,
            &store,
            &[dir.join("vault.dat")],
            &[],
        );
        if native_backend_available() {
            assert_eq!(status.backend, ProtectionBackend::SystemKeyring);
        } else {
            assert_eq!(status.backend, ProtectionBackend::Unavailable);
            assert!(status.fallback_reason.is_some());
        }
        assert_eq!(status.availability, ProtectionAvailability::Available);
        assert!(status.keyring_has_key);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥文件里的旧密钥能解开密文 → 归为 file-fallback 并给出原因（不谎称系统保护）
    #[test]
    fn fallback_key_reports_file_backend_with_reason() {
        let dir = temp_dir("fallback");
        let store = MemoryKeyStore::new();
        let key = [0x42u8; 32];
        let enc = super::super::encrypt_with_aad(&key, b"[]", &[]).unwrap();
        std::fs::write(dir.join("vault.dat"), &enc).unwrap();
        seed_fallback_file(&dir, &VAULT_KEY_SPEC, &key).expect("写入降级密钥文件");

        let status = inspect_domain(
            "vault",
            &dir,
            &VAULT_KEY_SPEC,
            &store,
            &[dir.join("vault.dat")],
            &[],
        );
        assert_eq!(status.availability, ProtectionAvailability::Available);
        assert!(status.fallback_file_exists);
        assert!(status.fallback_reason.is_some(), "降级必须有原因文案");
        assert_eq!(
            store.write_attempts(),
            0,
            "查询保护状态不得把降级密钥登记进系统密钥库（查询无写副作用）"
        );
        if native_backend_available() {
            assert_eq!(status.backend, ProtectionBackend::FileFallback);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密文在、两处密钥都对不上 → locked + unavailable，且文案指向恢复入口
    #[test]
    fn locked_when_no_candidate_matches() {
        let dir = temp_dir("locked");
        let store = MemoryKeyStore::new();
        store.write(VAULT_KEY_SPEC.account, &[0x51u8; 32]).unwrap();
        let enc = super::super::encrypt_with_aad(&[0x99u8; 32], b"[]", &[]).unwrap();
        std::fs::write(dir.join("vault.dat"), &enc).unwrap();

        let status = inspect_domain(
            "vault",
            &dir,
            &VAULT_KEY_SPEC,
            &store,
            &[dir.join("vault.dat")],
            &[],
        );
        assert_eq!(status.availability, ProtectionAvailability::Locked);
        assert_eq!(status.backend, ProtectionBackend::Unavailable);
        assert!(status.ciphertext_exists);
        let reason = status.fallback_reason.unwrap_or_default();
        assert!(reason.contains("无法解锁"), "锁定原因应可展示: {reason}");
        assert!(
            !dir.join(VAULT_KEY_SPEC.fallback_file).exists(),
            "锁定状态不得新增降级密钥文件"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 备份也算密文证据：只剩 `.bak` 时不得被判成「尚未初始化」
    #[test]
    fn backup_only_counts_as_ciphertext_evidence() {
        let dir = temp_dir("backup-only");
        let store = MemoryKeyStore::new();
        let key = [0x61u8; 32];
        store.write(CREDENTIALS_KEY_SPEC.account, &key).unwrap();
        let enc = super::super::encrypt_with_aad(&key, br#"{"k":"v"}"#, &[]).unwrap();
        let backup = backup_path(&dir.join("database.enc"));
        std::fs::write(&backup, &enc).unwrap();

        let status = inspect_domain(
            "credentials",
            &dir,
            &CREDENTIALS_KEY_SPEC,
            &store,
            &[dir.join("database.enc")],
            &[],
        );
        assert!(status.ciphertext_exists, "备份应计入密文证据");
        assert_eq!(status.availability, ProtectionAvailability::Available);
        std::fs::remove_dir_all(&dir).ok();
    }
}
