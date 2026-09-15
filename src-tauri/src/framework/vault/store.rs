//! Vault 凭证库存储层：布局与锁、读写、保护状态、脱敏摘要、读取入口
//! - 加解密、主密钥来源、原子替换与备份恢复统一来自 `framework/secure_store`（T05：与 credentials 共用一套原语）。
//!   T13 拆分后本模块只保留 Vault 自己的职责，原先内联的「主密钥解析 / AES-GCM 加解密」已下沉 secure_store，
//!   不再有第二份实现；子模块：`paths`（分区解析 / 旧布局回落 / 损坏文件留档 / 进程内锁）、
//!   `io`（明文 JSON 数组读写）、`status`（保护状态判定）、`summary`（脱敏摘要）、`api`（AppHandle 级读取入口）。
//! - 主密钥：系统密钥库 account `vault-master-key` → 兼容本地文件 `vault-master.key` → 首次生成；
//!   有既有密文时以能否解开密文为准（旧降级密钥是迁移输入，不允许「密钥库优先」掩盖正确旧密钥）
//! - 凭证密文：`<vault 分区>/vault.dat`，明文为 `Credential` 数组；读路径先恢复中断遗留备份，
//!   只有主文件与备份都不存在才当空库
//! - 安全边界：防「凭证明文落盘、文件被拷走即泄密」；不防「已登录当前系统账户的恶意进程」，
//!   也不提供抗离线解密能力（主密钥与密文同目录被整份复制时仍可解）。
//!   stronghold（内存隔离）与主密码解锁为后续升级项，接入时只换存储/解锁层，数据模型不变。

mod api;
mod io;
mod paths;
mod status;
mod summary;

pub(crate) use api::read_all;
// 插件命令在 Rust 侧解析 credentialId 的入口（crate 内 API，不做成 Tauri 命令）
pub use api::resolve;
pub(crate) use io::{read_all_at, write_all_at};
pub(crate) use paths::{data_dir_of, orphan_vault_file, vault_lock};
pub(crate) use status::protection_status;
pub(crate) use summary::summary_of;

// 密钥库实现与安全原语统一来自 `framework/secure_store`（T05：本模块不再重复实现）

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::secure_store::seed_fallback_file;
    use crate::framework::secure_store::test_support::{promotion_test_guard, MemoryKeyStore};
    use crate::framework::secure_store::VAULT_KEY_SPEC;

    use super::super::models::{Credential, CredentialFields, CredentialKind};
    use super::paths::{resolve_vault_dir, VAULT_FILE};
    use super::status::protection_status_at;
    use super::summary::mask_secret;

    use std::path::PathBuf;

    /// 旧布局回落：vault 分区不存在而空间根下仍有旧 vault.dat 时按空间根解析；
    /// 分区布局（非默认空间）下两个参数不同，回落仍只发生在同一空间内部
    #[test]
    fn vault_dir_falls_back_to_root_for_legacy_layout() {
        let dir = std::env::temp_dir().join(format!(
            "vault-legacy-dir-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VAULT_FILE), b"legacy").unwrap();
        assert_eq!(
            resolve_vault_dir(&dir, &dir),
            dir,
            "vault 分区不存在时应回落到根"
        );

        std::fs::create_dir_all(dir.join("vault")).unwrap();
        assert_eq!(
            resolve_vault_dir(&dir.join("vault"), &dir),
            dir.join("vault"),
            "vault 分区存在时用分区"
        );

        // 分区布局：空间根与 vault 分区都在 spaces/<id>/generations/1 之下，
        // 回落目标必须是本空间根，而不是设备根
        let space_root = dir
            .join("spaces")
            .join("space-a")
            .join("generations")
            .join("1");
        std::fs::create_dir_all(&space_root).unwrap();
        std::fs::write(space_root.join(VAULT_FILE), b"legacy-in-space").unwrap();
        assert_eq!(
            resolve_vault_dir(&space_root.join("vault"), &space_root),
            space_root,
            "空间内旧位置有 vault.dat 时回落到该空间根"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 构造测试凭证
    fn sample_credential(id: &str) -> Credential {
        let now = chrono::Utc::now().timestamp();
        Credential {
            id: id.into(),
            name: "生产 MySQL".into(),
            kind: CredentialKind::Password,
            fields: CredentialFields::Password {
                username: "root".into(),
                password: "s3cret".into(),
            },
            note: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vault-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn interrupted_write_recovers_backup_on_read() {
        let dir = temp_dir("vault-bak-recover");
        let store = MemoryKeyStore::with_key(VAULT_KEY_SPEC.account, [0x21u8; 32]);
        let credential = sample_credential("id-bak");
        write_all_at(&dir, &store, std::slice::from_ref(&credential)).unwrap();

        // 模拟崩溃点：旧文件已改名为 .bak，新文件尚未就位
        let path = dir.join(VAULT_FILE);
        let backup = path.with_extension("bak");
        std::fs::rename(&path, path.with_extension("bak")).unwrap();
        assert!(!path.exists());

        assert_eq!(read_all_at(&dir, &store).unwrap(), vec![credential]);
        assert!(path.exists(), "读取后备份应已转正为主文件");
        assert!(!backup.exists(), "转正后残留备份应清理");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 主文件可解时清理残留 `.bak`，不把过期版本当数据源
    #[test]
    fn stale_backup_is_cleaned_when_main_is_readable() {
        let dir = temp_dir("vault-stale-bak");
        let store = MemoryKeyStore::with_key(VAULT_KEY_SPEC.account, [0x22u8; 32]);
        let credential = sample_credential("id-stale");
        write_all_at(&dir, &store, std::slice::from_ref(&credential)).unwrap();

        let path = dir.join(VAULT_FILE);
        let backup = path.with_extension("bak");
        std::fs::write(&backup, b"stale").unwrap();

        assert_eq!(read_all_at(&dir, &store).unwrap(), vec![credential]);
        assert!(!backup.exists(), "主文件可读时应清理过期备份");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 主文件损坏而备份完好 → 改用备份，坏文件留档不删除
    #[test]
    fn unreadable_main_falls_back_to_backup_and_archives_corrupt() {
        let dir = temp_dir("vault-corrupt-main");
        let store = MemoryKeyStore::with_key(VAULT_KEY_SPEC.account, [0x23u8; 32]);
        let credential = sample_credential("id-corrupt");
        write_all_at(&dir, &store, std::slice::from_ref(&credential)).unwrap();

        let path = dir.join(VAULT_FILE);
        let backup = path.with_extension("bak");
        std::fs::copy(&path, &backup).unwrap();
        // 主文件被写坏（长度足够但认证不过），备份仍是好的
        std::fs::write(&path, vec![0xABu8; 64]).unwrap();

        assert_eq!(read_all_at(&dir, &store).unwrap(), vec![credential]);
        let archived = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"));
        assert!(archived, "坏文件应改名留档");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥丢失宁可锁死：密文在而密钥库与降级文件都没有密钥 → 报「无法解锁」，
    /// 不生成降级密钥文件、不改动密文
    #[test]
    fn missing_key_locks_instead_of_regenerating() {
        let dir = temp_dir("vault-locked");
        let store_with_key = MemoryKeyStore::new();
        let credential = sample_credential("id-locked");
        write_all_at(&dir, &store_with_key, std::slice::from_ref(&credential)).unwrap();
        let blob = std::fs::read(dir.join(VAULT_FILE)).unwrap();

        // 新空密钥库（换机 / 重装 / 密钥库被清空）
        let empty_store = MemoryKeyStore::new();
        let error = read_all_at(&dir, &empty_store).unwrap_err();
        assert!(error.contains("无法解锁"), "错误应指向解锁失败: {error}");
        assert!(
            !dir.join(VAULT_KEY_SPEC.fallback_file).exists(),
            "锁死时不得偷偷生成降级密钥文件"
        );
        assert_eq!(std::fs::read(dir.join(VAULT_FILE)).unwrap(), blob);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 写入路径同样不许生成新密钥：密文在而密钥不可用 → 写失败且密文原样保留
    #[test]
    fn write_refuses_when_key_is_missing() {
        let dir = temp_dir("vault-write-locked");
        let store_with_key = MemoryKeyStore::new();
        write_all_at(
            &dir,
            &store_with_key,
            std::slice::from_ref(&sample_credential("id-1")),
        )
        .unwrap();
        let blob = std::fs::read(dir.join(VAULT_FILE)).unwrap();

        let empty_store = MemoryKeyStore::new();
        assert!(write_all_at(&dir, &empty_store, &[]).is_err());
        assert_eq!(
            std::fs::read(dir.join(VAULT_FILE)).unwrap(),
            blob,
            "写失败不得覆盖既有密文"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 状态查询无副作用：不生成密钥、不写密钥库、不落降级文件，且两个域都要报告
    #[test]
    fn protection_status_has_no_side_effects() {
        let dir = temp_dir("vault-status");
        let store = MemoryKeyStore::new();
        let status = protection_status_at(&dir, &dir, &store).unwrap();
        let domains: Vec<&str> = status.domains.iter().map(|d| d.domain.as_str()).collect();
        assert_eq!(domains, vec!["vault", "credentials"]);
        assert!(status.domains.iter().all(|d| !d.ciphertext_exists));
        assert!(!dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        assert_eq!(store.write_attempts(), 0, "查询状态不应写密钥库");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 降级密钥是迁移输入：密钥库为空但本地密钥文件能解开既有密文 → 照常读取
    #[test]
    fn fallback_key_file_still_unlocks_existing_ciphertext() {
        let dir = temp_dir("vault-fallback-unlock");
        // 本用例会走「降级密钥登记」分支（进程级共享记录），与同类用例串行
        let _serialize = promotion_test_guard();
        let key = [0x24u8; 32];
        let credential = sample_credential("id-fallback");
        // 用降级文件里的密钥写入密文，密钥库为空
        seed_fallback_file(&dir, &VAULT_KEY_SPEC, &key).expect("写入降级密钥文件");
        let store = MemoryKeyStore::with_key(VAULT_KEY_SPEC.account, key);
        write_all_at(&dir, &store, std::slice::from_ref(&credential)).unwrap();

        let empty_store = MemoryKeyStore::new();
        assert_eq!(read_all_at(&dir, &empty_store).unwrap(), vec![credential]);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 读写全量往返：损坏 vault.dat 报错不 panic
    #[test]
    fn read_write_roundtrip_and_corrupt_file() {
        let dir = temp_dir("read-write");
        let store = MemoryKeyStore::with_key(VAULT_KEY_SPEC.account, [0x25u8; 32]);
        let credential = sample_credential("id-rt");
        write_all_at(&dir, &store, std::slice::from_ref(&credential)).unwrap();
        assert_eq!(read_all_at(&dir, &store).unwrap(), vec![credential]);

        // 损坏 vault.dat（内容过短）→ 报错不 panic
        std::fs::write(dir.join(VAULT_FILE), b"broken").unwrap();
        assert!(read_all_at(&dir, &store).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 脱敏摘要：各类型不含秘密明文，掩码格式正确
    #[test]
    fn summary_masks_secrets() {
        // 样例值运行期拼装：本文件不出现疑似真实密钥的字面量，断言只关心掩码格式（前 3 + **** + 后 3）
        let sample = format!("AKIA{}PLE", "X".repeat(10));
        assert_eq!(mask_secret(&sample), "AKI****PLE");
        assert_eq!(mask_secret("abc"), "••••");
        let credential = Credential {
            id: "id-2".into(),
            name: "腾讯云 CAM".into(),
            kind: CredentialKind::AccessKeyPair,
            fields: CredentialFields::AccessKeyPair {
                access_key_id: format!("AKIA{}PLE", "X".repeat(10)),
                access_key_secret: "topsecret".into(),
            },
            note: String::new(),
            created_at: 0,
            updated_at: 0,
        };
        let summary = summary_of(&credential);
        assert_eq!(summary.masked, "AKI****PLE");
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("topsecret"));
    }
}
