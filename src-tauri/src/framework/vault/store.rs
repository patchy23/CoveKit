//! Vault 凭证库存储层：凭证条目模型 + 读写 + 汇总/引用统计
//! - 加解密、主密钥来源、原子替换与备份恢复统一来自 `framework/secure_store`（T05：与 credentials 共用一套原语）
//! - 主密钥：系统密钥库 account `vault-master-key` → 兼容本地文件 `vault-master.key` → 首次生成；
//!   有既有密文时以能否解开密文为准（旧降级密钥是迁移输入，不允许「密钥库优先」掩盖正确旧密钥）
//! - 凭证密文：`<vault 分区>/vault.dat`，明文为 `Credential` 数组；读路径先恢复中断遗留备份，
//!   只有主文件与备份都不存在才当空库
//! - 安全边界：防「凭证明文落盘、文件被拷走即泄密」；不防「已登录当前系统账户的恶意进程」，
//!   也不提供抗离线解密能力（主密钥与密文同目录被整份复制时仍可解）。
//!   stronghold（内存隔离）与主密码解锁为后续升级项，接入时只换存储/解锁层，数据模型不变。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use super::models::{Credential, CredentialSummary};
use crate::framework::credentials;
use crate::framework::secure_store::{
    ciphertext_evidence, encrypt_payload, inspect_domain, load_verified, native_backend_available,
    replace_file, resolve_master_key, ProtectionStatus, CREDENTIALS_KEY_SPEC, VAULT_KEY_SPEC,
};
// 密钥库实现与安全原语统一来自 `framework/secure_store`（T05：本文件不再重复实现）
pub(crate) use crate::framework::secure_store::{KeyringStore, MasterKeyStore};

/// 凭证密文文件名（vault 分区下）
pub(crate) const VAULT_FILE: &str = "vault.dat";

/// vault 全量读改写进程内互斥锁（防并发丢更新）
pub(crate) fn vault_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Vault 目录解析（含旧布局回落；纯函数便于单测）
///
/// 新布局：`<root>/vault/vault.dat`（主密钥同目录）；旧布局：`<root>/vault.dat`。
/// 只有「vault 分区不存在、根下仍有旧 vault.dat」时才回落：布局迁移可能整组保留原位，
/// 此时必须按旧位置读写，否则会表现为凭证库为空、甚至用新密钥覆盖旧密文。
pub(crate) fn resolve_vault_dir(root: &Path) -> PathBuf {
    let dir = root.join("vault");
    if !dir.exists() && root.join(VAULT_FILE).exists() {
        return root.to_path_buf();
    }
    dir
}

/// 凭证分区目录（`<root>/vault`；命令层用，测试走 *_at 目录参数版本）
pub(crate) fn data_dir_of(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_vault_dir(&crate::framework::paths::storage_root(
        app,
    )?))
}

/// 将无法读取的 vault.dat 改名留档（密钥丢失/文件损坏时导入的前置保护，绝不静默清空）
pub(crate) fn orphan_vault_file(dir: &Path) -> Result<(), String> {
    let path = dir.join(VAULT_FILE);
    if !path.exists() {
        return Ok(());
    }
    let orphan = dir.join(format!(
        "vault.dat.unreadable-{}.bak",
        chrono::Utc::now().timestamp()
    ));
    std::fs::rename(&path, &orphan)
        .map_err(|e| format!("旧凭证库留档失败（未做任何清除，原文件仍在）: {e}"))
}

// ──────────────────────────────────────────────────────────────────────────
// 安全原语：统一来自 `framework/secure_store`（T05）
// 此处原先内联的「主密钥外部存储抽象 / 主密钥解析 / AES-GCM 加解密 / 原子写与备份恢复」
// 已收敛为一份实现，避免 Vault 与 credentials 各写一套导致修复只落到一边：
// - 主密钥来源与降级登记：`secure_store::resolve_master_key`（有既有密文时以密文为准）
// - 加解密：`secure_store::encrypt_payload` / `load_verified`
// - 原子替换与择版恢复：`secure_store::replace_file` / `secure_store::load_verified`
// 本文件只保留 Vault 自己的凭证条目模型、路径、锁与引用统计。
// ──────────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────────
// vault 读改写（目录参数便于单测；明文 = JSON 数组）
// ──────────────────────────────────────────────────────────────────────────

/// 读取全部凭证（解密；主文件与备份都不存在时返回空表）
pub(crate) fn read_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
) -> Result<Vec<Credential>, String> {
    let path = dir.join(VAULT_FILE);
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence)?.key;
    let decode = |plain: &[u8]| -> Result<Vec<Credential>, String> {
        serde_json::from_slice(plain).map_err(|e| format!("凭证数据解析失败: {e}"))
    };
    // 读路径内完成择版：主文件可解就用主文件，只有备份可用则备份转正（见 secure_store::load_verified）
    Ok(load_verified(&path, &key, &decode)?.unwrap_or_default())
}

/// 写回全部凭证（加密落盘，原子替换 + 备份恢复）
pub(crate) fn write_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
    credentials: &[Credential],
) -> Result<(), String> {
    let path = dir.join(VAULT_FILE);
    // 先按现有密文解析主密钥：既不能在半途另生成新密钥，也不能覆盖既有密文的语义
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence)?.key;
    let plain = serde_json::to_vec(credentials).map_err(|e| e.to_string())?;
    let out = encrypt_payload(&key, &plain)?;
    replace_file(&path, &out)
}

/// 凭证保护状态（T04-5）：vault 与 credentials 两个域共用一套判定，供设置页展示
pub(crate) fn protection_status_at(
    vault_dir: &Path,
    data_dir: &Path,
    store: &dyn MasterKeyStore,
) -> Result<ProtectionStatus, String> {
    let vault_files = [vault_dir.join(VAULT_FILE)];
    let credential_files = credentials::all_namespace_files(data_dir)?;
    Ok(ProtectionStatus {
        native_backend: native_backend_available(),
        domains: vec![
            inspect_domain("vault", vault_dir, &VAULT_KEY_SPEC, store, &vault_files),
            inspect_domain(
                "credentials",
                data_dir,
                &CREDENTIALS_KEY_SPEC,
                store,
                &credential_files,
            ),
        ],
    })
}

/// 凭证保护状态（AppHandle 封装；命令层入口）
///
/// 两个域各自解析目录（都带旧布局回落）：vault 走 vault 分区，
/// credentials 走数据分区，避免把 vault 目录当成凭据目录去扫。
pub(crate) fn protection_status(app: &AppHandle) -> Result<ProtectionStatus, String> {
    let vault_dir = data_dir_of(app)?;
    let credential_dir = credentials::resolved_data_dir(app)?;
    protection_status_at(&vault_dir, &credential_dir, &KeyringStore)
}

/// 锁内读全量（AppHandle 封装）
pub(crate) fn read_all(app: &AppHandle) -> Result<Vec<Credential>, String> {
    let _guard = vault_lock().lock().map_err(|e| e.to_string())?;
    read_all_at(&data_dir_of(app)?, &KeyringStore)
}

// ──────────────────────────────────────────────────────────────────────────
// crate 内解析 API 与脱敏摘要
// ──────────────────────────────────────────────────────────────────────────

/// 插件命令在 Rust 侧解析 credentialId → 凭证明文（crate 内 API，不做成 Tauri 命令；
/// 明文不过 IPC、不到前端，插件解析后直接用于建连）
pub fn resolve(app: &AppHandle, credential_id: &str) -> Result<Credential, String> {
    let all = read_all(app)?;
    all.into_iter()
        .find(|c| c.id == credential_id)
        .ok_or_else(|| format!("凭证不存在或已删除（id: {credential_id}）"))
}

/// 秘密值掩码：≤6 字符全掩码；否则前 3 + **** + 后 3（如 AKI****xyz）
fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 6 {
        return "••••".into();
    }
    let head: String = chars.iter().take(3).collect();
    let tail: String = chars.iter().skip(chars.len() - 3).collect();
    format!("{head}****{tail}")
}

/// 生成列表用脱敏摘要（无明文秘密；用户名等非秘密字段可直接展示）
pub(crate) fn summary_of(credential: &Credential) -> CredentialSummary {
    let masked = match &credential.fields {
        // 用户名不是秘密，直接展示；空用户名退回掩码
        super::models::CredentialFields::Password { username, password } => {
            if username.is_empty() {
                mask_secret(password)
            } else {
                username.clone()
            }
        }
        super::models::CredentialFields::SshKey { username, .. } => {
            if username.is_empty() {
                "私钥凭证".to_string()
            } else {
                username.clone()
            }
        }
        super::models::CredentialFields::ApiToken { token } => mask_secret(token),
        super::models::CredentialFields::AccessKeyPair { access_key_id, .. } => {
            mask_secret(access_key_id)
        }
        super::models::CredentialFields::Custom { entries } => {
            format!("{} 个字段", entries.len())
        }
    };
    CredentialSummary {
        id: credential.id.clone(),
        name: credential.name.clone(),
        kind: credential.kind,
        masked,
        note: credential.note.clone(),
        created_at: credential.created_at,
        updated_at: credential.updated_at,
    }
}

/// 凭证引用计数（供删除提示）。这里只扫描后端持久化配置；
/// SSH localStorage 引用由前端登记表补充，避免框架反向依赖插件前端实现。
pub(crate) fn reference_count(app: &AppHandle, credential_id: &str) -> usize {
    dns_reference_count(app, credential_id)
}

/// 统计 dns.db 中阿里云 / DNSPod / Cloudflare 配置对凭证的引用。
fn dns_reference_count(app: &AppHandle, credential_id: &str) -> usize {
    let Ok(path) = crate::framework::store::plugin_db_path(app, "dns") else {
        return 0;
    };
    if !path.exists() {
        return 0;
    }
    let Ok(conn) =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return 0;
    };
    count_dns_references(&conn, credential_id)
}

/// 在已打开的 DNS 数据库连接上统计引用；独立函数便于覆盖旧表结构与多引用单测。
fn count_dns_references(conn: &rusqlite::Connection, credential_id: &str) -> usize {
    conn.query_row(
        "SELECT COUNT(*) FROM dns_config WHERE credential_ref = ?1",
        [credential_id],
        |row| row.get::<_, usize>(0),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::secure_store::seed_fallback_file;
    use crate::framework::secure_store::test_support::{promotion_test_guard, MemoryKeyStore};

    use super::super::models::{CredentialFields, CredentialKind};

    /// 旧布局回落：vault 分区不存在而根下仍有旧 vault.dat 时按存储根解析
    #[test]
    fn vault_dir_falls_back_to_root_for_legacy_layout() {
        let dir = std::env::temp_dir().join(format!(
            "vault-legacy-dir-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(VAULT_FILE), b"legacy").unwrap();
        assert_eq!(resolve_vault_dir(&dir), dir, "vault 分区不存在时应回落到根");

        std::fs::create_dir_all(dir.join("vault")).unwrap();
        assert_eq!(
            resolve_vault_dir(&dir),
            dir.join("vault"),
            "vault 分区存在时用分区"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dns_reference_count_handles_current_and_legacy_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dns_config (
                platform TEXT PRIMARY KEY,
                credential_ref TEXT
            );
            INSERT INTO dns_config VALUES ('aliyun', 'credential-1');
            INSERT INTO dns_config VALUES ('dnspod', 'credential-1');",
        )
        .unwrap();
        assert_eq!(count_dns_references(&conn, "credential-1"), 2);

        let legacy = rusqlite::Connection::open_in_memory().unwrap();
        legacy
            .execute_batch("CREATE TABLE dns_config (platform TEXT PRIMARY KEY);")
            .unwrap();
        assert_eq!(count_dns_references(&legacy, "credential-1"), 0);
    }

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vault-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
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

    /// 崩溃恢复：写入提交前中断（只剩 `.bak`）→ 读路径把备份转正，数据不丢
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
        let now = chrono::Utc::now().timestamp();
        let credential = Credential {
            id: "id-1".into(),
            name: "生产 MySQL".into(),
            kind: CredentialKind::Password,
            fields: CredentialFields::Password {
                username: "root".into(),
                password: "s3cret".into(),
            },
            note: String::new(),
            created_at: now,
            updated_at: now,
        };
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
        assert_eq!(mask_secret("AKIAIOSFODNN7EXAMPLE"), "AKI****PLE");
        assert_eq!(mask_secret("abc"), "••••");
        let credential = Credential {
            id: "id-2".into(),
            name: "腾讯云 CAM".into(),
            kind: CredentialKind::AccessKeyPair,
            fields: CredentialFields::AccessKeyPair {
                access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
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
