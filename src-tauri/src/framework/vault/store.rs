//! 存储与加密：主密钥（系统密钥库 + 降级文件）/ AES-256-GCM 密文文件 / 原子写与备份恢复
//! - 主密钥：首次 OsRng 生成 32B，存系统密钥库（keyring crate：Windows Credential Manager /
//!   macOS Keychain / Linux Secret Service）；keyring 不可用时降级 app_data_dir/vault-master.key
//!   并告警。keyring 只存这把主密钥，凭证本体不进 keyring（Credential Manager 单条 ~2.5KB 上限）。
//! - 凭证密文：AES-256-GCM，每次加密新 nonce，落盘 nonce(12B)‖ciphertext → app_data_dir/vault.dat；
//!   解密先校验 ≥28B（12 nonce + 16 认证标签），损坏即报错不 panic。
//! - 原子写 + 备份恢复：replace_file / recover_backup 原语下沉自 plugins/ssh/credential.rs；
//!   SSH 原手工凭据文件继续保留，公共 Vault 作为可选来源。
//! - 安全边界：防「凭证明文落盘、文件被拷走即泄密」；不防「已登录当前系统账户的恶意进程」。
//!   stronghold（内存隔离）与主密码解锁为后续升级项，接入时只换存储/解锁层，数据模型不变。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use tauri::AppHandle;

use super::models::{Credential, CredentialSummary};

/// 凭证密文文件名（app_data_dir 下）
pub(crate) const VAULT_FILE: &str = "vault.dat";
/// 主密钥降级文件名（keyring 不可用时回退）
pub(crate) const FALLBACK_KEY_FILE: &str = "vault-master.key";
/// 系统密钥库 service 名（应用 identifier）
const KEYRING_SERVICE: &str = "com.patchy23.patchybox";
/// 系统密钥库 account 名
const KEYRING_ACCOUNT: &str = "vault-master-key";

/// vault 全量读改写进程内互斥锁（防并发丢更新）
pub(crate) fn vault_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 凭证分区目录（`<root>/vault`；命令层用，测试走 *_at 目录参数版本）
pub(crate) fn data_dir_of(app: &AppHandle) -> Result<PathBuf, String> {
    crate::framework::paths::vault_dir(app)
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
// 主密钥外部存储抽象（生产实现 = 系统密钥库；单测注入内存实现）
// ──────────────────────────────────────────────────────────────────────────

/// 主密钥外部存储抽象（隔离 keyring 便于单测；实现方必须能表达「不存在」与「调用失败」）
pub(crate) trait MasterKeyStore {
    /// 读取主密钥（Ok(None) = 密钥库中没有记录）
    fn read(&self) -> Result<Option<[u8; 32]>, String>;
    /// 写入主密钥
    fn write(&self, key: &[u8; 32]) -> Result<(), String>;
}

/// 系统密钥库实现（keyring crate；各平台原生后端）
pub(crate) struct KeyringStore;

impl MasterKeyStore for KeyringStore {
    fn read(&self) -> Result<Option<[u8; 32]>, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        match entry.get_secret() {
            Ok(bytes) => {
                let key: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "密钥库中的主密钥长度不是 32 字节".to_string())?;
                Ok(Some(key))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("密钥库读取失败: {e}")),
        }
    }

    fn write(&self, key: &[u8; 32]) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        entry
            .set_secret(key)
            .map_err(|e| format!("密钥库写入失败: {e}"))
    }
}

// ──────────────────────────────────────────────────────────────────────────
// 主密钥解析（keyring 优先 → 降级文件 → 首次生成；vault.dat 存在而无密钥时宁可锁死）
// ──────────────────────────────────────────────────────────────────────────

/// 生成 32B 随机主密钥并写入外部存储；keyring 写失败或写后回读校验不过时降级本地文件并告警
fn create_master_key(dir: &Path, store: &dyn MasterKeyStore) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    // 写入后立刻回读校验：Windows 凭据管理器存在「写返回成功但条目没落库」的静默丢失场景，
    // 不校验的话 vault.dat 会用一把没存住的密钥加密，下次启动永远无法解锁（死数据）
    let persisted = match store.write(&key) {
        Ok(()) => match store.read() {
            Ok(Some(readback)) if readback == key => true,
            other => {
                eprintln!(
                    "[vault] 密钥库写入后回读校验失败（{other:?}），主密钥降级为本地文件存储"
                );
                false
            }
        },
        Err(e) => {
            eprintln!("[vault] 密钥库写入失败（{e}），主密钥降级为本地文件存储");
            false
        }
    };
    if !persisted {
        replace_file(&dir.join(FALLBACK_KEY_FILE), &key)
            .map_err(|e| format!("降级主密钥写入失败: {e}"))?;
    }
    Ok(key)
}

/// 从降级文件读取主密钥（存在则必须恰好 32B）
fn read_fallback_key(dir: &Path) -> Result<Option<[u8; 32]>, String> {
    let path = dir.join(FALLBACK_KEY_FILE);
    recover_backup(&path)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("降级主密钥读取失败: {e}"))?;
    let key: [u8; 32] = bytes.try_into().map_err(|_| {
        "降级主密钥文件损坏（长度不是 32 字节），为避免凭证丢失已停止操作".to_string()
    })?;
    Ok(Some(key))
}

/// 解析主密钥：密钥库 → 降级文件 → 首次生成。
/// 密钥丢失但 vault.dat 存在时明确报「无法解锁凭证库」，绝不重新生成密钥覆盖语义（防误毁数据）。
pub(crate) fn master_key_at(dir: &Path, store: &dyn MasterKeyStore) -> Result<[u8; 32], String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    // 1) 系统密钥库优先；读取失败仅告警继续降级（无桌面环境等场景）
    match store.read() {
        Ok(Some(key)) => return Ok(key),
        Ok(None) => {}
        Err(e) => eprintln!("[vault] {e}，尝试降级主密钥文件"),
    }
    // 2) 降级文件
    if let Some(key) = read_fallback_key(dir)? {
        return Ok(key);
    }
    // 3) 两处都没有：密文还在就是密钥丢失，宁可锁死不可误删
    if dir.join(VAULT_FILE).exists() {
        return Err(
            "无法解锁凭证库：系统密钥库中找不到主密钥（可能原因：换机 / 重装 / 密钥库被清空）。\
             可通过「导入备份」恢复，或手动删除数据目录下的 vault.dat 重新初始化（原文件未被清除）"
                .into(),
        );
    }
    // 4) 首次启动：生成新主密钥
    create_master_key(dir, store)
}

// ──────────────────────────────────────────────────────────────────────────
// 加密原语（AES-256-GCM，nonce(12B)‖ciphertext；与 ssh/credential.rs 同源下沉）
// ──────────────────────────────────────────────────────────────────────────

/// 使用主密钥解密 nonce(12B)||ciphertext，先校验最小长度避免损坏文件触发 panic。
pub(crate) fn decrypt_payload(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    // AES-GCM 密文至少包含 12 字节 nonce 与 16 字节认证标签。
    if data.len() < 28 {
        return Err("凭证文件损坏（密文长度不足）".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let (nonce, ct) = data.split_at(12);
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| "凭证解密失败（主密钥不匹配或数据损坏）".into())
}

/// 使用主密钥加密明文，返回 nonce(12B)||ciphertext（每次新 nonce）。
pub(crate) fn encrypt_payload(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|_| "凭证加密失败")?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

// ──────────────────────────────────────────────────────────────────────────
// 原子写 + 备份恢复（原语下沉自 plugins/ssh/credential.rs；SSH 手工路径继续兼容）
// ──────────────────────────────────────────────────────────────────────────

/// 先完整写入临时文件并刷盘，再替换目标（旧文件改 .bak，替换成功后删除），避免半截密文。
pub(crate) fn replace_file(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建凭证目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("临时凭证文件创建失败: {e}"))?;
    file.write_all(content)
        .map_err(|e| format!("临时凭证文件写入失败: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("临时凭证文件刷盘失败: {e}"))?;
    if !path.exists() {
        return std::fs::rename(&tmp, path).map_err(|e| format!("凭证文件写入失败: {e}"));
    }
    let backup = path.with_extension("bak");
    if backup.exists() {
        std::fs::remove_file(&backup).map_err(|e| format!("旧凭证备份清理失败: {e}"))?;
    }
    std::fs::rename(path, &backup).map_err(|e| format!("旧凭证文件备份失败: {e}"))?;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let restore = std::fs::rename(&backup, path);
        return match restore {
            Ok(()) => Err(format!("凭证文件替换失败，已恢复旧数据: {error}")),
            Err(restore_error) => Err(format!(
                "凭证文件替换与恢复均失败: {error}; {restore_error}（旧数据位于 {}）",
                backup.display()
            )),
        };
    }
    if let Err(error) = std::fs::remove_file(&backup) {
        eprintln!("[vault] 凭证已保存，但备份清理失败: {error}");
    }
    Ok(())
}

/// 恢复进程中断遗留的备份；主文件存在时仅清理已过期备份。
pub(crate) fn recover_backup(path: &Path) -> Result<(), String> {
    let backup = path.with_extension("bak");
    match (path.exists(), backup.exists()) {
        (false, true) => {
            std::fs::rename(&backup, path).map_err(|e| format!("凭证备份恢复失败: {e}"))
        }
        (true, true) => std::fs::remove_file(&backup).map_err(|e| format!("凭证备份清理失败: {e}")),
        _ => Ok(()),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// vault 读改写（目录参数便于单测；明文 = JSON 数组）
// ──────────────────────────────────────────────────────────────────────────

/// 读取全部凭证（解密；无文件时返回空表；先恢复中断遗留备份）
pub(crate) fn read_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
) -> Result<Vec<Credential>, String> {
    let path = dir.join(VAULT_FILE);
    recover_backup(&path)?;
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("凭证文件读取失败: {error}")),
    };
    let key = master_key_at(dir, store)?;
    let plain = decrypt_payload(&key, &data)?;
    serde_json::from_slice(&plain).map_err(|e| format!("凭证数据解析失败: {e}"))
}

/// 写回全部凭证（加密落盘，原子写）
pub(crate) fn write_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
    credentials: &[Credential],
) -> Result<(), String> {
    let key = master_key_at(dir, store)?;
    let plain = serde_json::to_vec(credentials).map_err(|e| e.to_string())?;
    let out = encrypt_payload(&key, &plain)?;
    replace_file(&dir.join(VAULT_FILE), &out)
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
    use std::cell::RefCell;

    use super::super::models::{CredentialFields, CredentialKind};

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

    /// 内存密钥库桩（可注入读/写失败，模拟无桌面环境）
    struct MemStore {
        /// 已存储的密钥（None = 密钥库中无记录）
        key: RefCell<Option<[u8; 32]>>,
        /// 注入读失败
        fail_read: bool,
        /// 注入写失败
        fail_write: bool,
        /// 注入静默丢写（write 返回 Ok 但不落库，模拟 Windows 凭据管理器丢失场景）
        lose_writes: bool,
    }

    impl MemStore {
        /// 构造空密钥库桩
        fn new() -> Self {
            MemStore {
                key: RefCell::new(None),
                fail_read: false,
                fail_write: false,
                lose_writes: false,
            }
        }
    }

    impl MasterKeyStore for MemStore {
        fn read(&self) -> Result<Option<[u8; 32]>, String> {
            if self.fail_read {
                return Err("注入的读失败".into());
            }
            Ok(*self.key.borrow())
        }
        fn write(&self, key: &[u8; 32]) -> Result<(), String> {
            if self.fail_write {
                return Err("注入的写失败".into());
            }
            if !self.lose_writes {
                *self.key.borrow_mut() = Some(*key);
            }
            Ok(())
        }
    }

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vault-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 加解密往返 + 错误密钥/损坏密文校验
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [7u8; 32];
        let plain = br#"[{"id":"a"}]"#;
        let ct = encrypt_payload(&key, plain).unwrap();
        assert_eq!(decrypt_payload(&key, &ct).unwrap(), plain);
        assert!(decrypt_payload(&[8u8; 32], &ct).is_err());
    }

    /// nonce 唯一性：同明文同密钥两次加密密文必须不同（否则 AES-GCM  nonce 重用是安全事故）
    #[test]
    fn nonce_uniqueness() {
        let key = [7u8; 32];
        let plain = b"same plaintext";
        let a = encrypt_payload(&key, plain).unwrap();
        let b = encrypt_payload(&key, plain).unwrap();
        assert_ne!(a, b);
        // 且两次都能解出同一明文
        assert_eq!(decrypt_payload(&key, &a).unwrap(), plain);
        assert_eq!(decrypt_payload(&key, &b).unwrap(), plain);
    }

    /// 损坏文件报错不 panic：长度过短与篡改密文都必须返回 Err
    #[test]
    fn corrupted_ciphertext_errors_no_panic() {
        let key = [7u8; 32];
        assert!(decrypt_payload(&key, b"short").is_err());
        assert!(decrypt_payload(&key, &[0u8; 27]).is_err());
        let mut ct = encrypt_payload(&key, b"hello vault").unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0xFF; // 篡改认证标签
        assert!(decrypt_payload(&key, &ct).is_err());
    }

    /// 原子写 + 备份恢复：正常替换清理 .bak；中断遗留 .bak 时启动恢复
    #[test]
    fn replace_and_recover_backup() {
        let dir = temp_dir("replace-recover");
        let path = dir.join("vault.dat");

        // 首次写入（无 .bak 产生）
        replace_file(&path, b"v1").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v1");

        // 二次写入：旧文件改 .bak，成功后清理
        replace_file(&path, b"v2").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v2");
        assert!(!path.with_extension("bak").exists());

        // 模拟中断：只剩 .bak，无主文件 → recover_backup 恢复
        std::fs::rename(&path, path.with_extension("bak")).unwrap();
        recover_backup(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v2");
        assert!(!path.with_extension("bak").exists());

        // 主文件存在且 .bak 残留 → 清理 .bak
        std::fs::write(path.with_extension("bak"), b"stale").unwrap();
        recover_backup(&path).unwrap();
        assert!(!path.with_extension("bak").exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"v2");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 主密钥：首次生成入密钥库并复用；密钥库读失败时走降级文件
    #[test]
    fn master_key_generate_reuse_and_fallback() {
        let dir = temp_dir("master-key");
        let store = MemStore::new();

        // 首次：生成并写入密钥库；二次：复用同一把
        let k1 = master_key_at(&dir, &store).unwrap();
        let k2 = master_key_at(&dir, &store).unwrap();
        assert_eq!(k1, k2);
        assert_eq!(store.key.borrow().unwrap(), k1);

        // 密钥库读失败 + 降级文件存在 → 走降级文件
        let dir2 = temp_dir("master-key-fallback");
        let fallback_key = [9u8; 32];
        replace_file(&dir2.join(FALLBACK_KEY_FILE), &fallback_key).unwrap();
        let failing = MemStore {
            key: RefCell::new(None),
            fail_read: true,
            fail_write: false,
            lose_writes: false,
        };
        assert_eq!(master_key_at(&dir2, &failing).unwrap(), fallback_key);

        // 降级文件损坏（长度不足）→ 明确报错
        std::fs::write(dir2.join(FALLBACK_KEY_FILE), b"short").unwrap();
        assert!(master_key_at(&dir2, &failing).is_err());

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&dir2).ok();
    }

    /// 密钥库静默丢写（write 返回 Ok 但回读无记录）→ 回读校验失败必须降级文件，
    /// 否则 vault.dat 会用没存住的密钥加密成死数据（Windows 凭据管理器真实场景回归）
    #[test]
    fn silent_write_loss_falls_back_to_file() {
        let dir = temp_dir("silent-loss");
        let store = MemStore {
            lose_writes: true,
            ..MemStore::new()
        };

        // 生成主密钥：密钥库没存住 → 降级文件兜底，且两次解析拿到同一把
        let k1 = master_key_at(&dir, &store).unwrap();
        assert!(dir.join(FALLBACK_KEY_FILE).exists());
        let k2 = master_key_at(&dir, &store).unwrap();
        assert_eq!(k1, k2);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥丢失宁可锁死：vault.dat 存在而密钥库与降级文件都没有密钥 → 报「无法解锁」，不重新生成
    #[test]
    fn lost_key_never_regenerates() {
        let dir = temp_dir("lost-key");
        // 用一把密钥写好 vault.dat，再丢掉密钥（新空密钥库）
        let store_with_key = MemStore::new();
        let key = master_key_at(&dir, &store_with_key).unwrap();
        let blob = encrypt_payload(&key, b"[]").unwrap();
        replace_file(&dir.join(VAULT_FILE), &blob).unwrap();

        let empty_store = MemStore::new();
        let err = master_key_at(&dir, &empty_store).unwrap_err();
        assert!(err.contains("无法解锁凭证库"));
        // 没有偷偷生成降级文件覆盖语义
        assert!(!dir.join(FALLBACK_KEY_FILE).exists());
        // vault.dat 原样保留
        assert_eq!(std::fs::read(dir.join(VAULT_FILE)).unwrap(), blob);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 读写全量往返：含备份恢复入口 + 损坏 vault.dat 报错不 panic
    #[test]
    fn read_write_roundtrip_and_corrupt_file() {
        let dir = temp_dir("read-write");
        let store = MemStore::new();
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
