//! 主密钥来源：系统密钥库（keyring 原生后端）→ 本地降级密钥文件 → 首次生成
//!
//! 核心规则（T04/T05）：
//! 1. **有既有密文时以密文为准**：候选密钥（密钥库 / 降级文件）必须能通过 GCM 认证才被采用，
//!    绝不因为「密钥库优先」而掩盖本地降级文件里的正确旧密钥；
//! 2. **绝不生成会覆盖既有密文的新密钥**：两处候选都对不上既有密文时返回锁死错误，
//!    由用户走备份导入或确认后重新初始化；
//! 3. 降级文件里的正确旧密钥会被**登记**进系统密钥库（写后回读校验），但**不删除降级文件**；
//! 4. 诊断文案只描述「有没有记录 / 长度对不对 / 读了还是写失败」，不打印任何密钥字节。

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::crypto::authenticates;
use super::file::{replace_file, restrict_to_current_user};
use rand::rngs::OsRng;
use rand::RngCore;

/// 系统密钥库 service 名（应用 identifier；修改会导致既有凭证不可访问，未经单独需求禁止改动）
pub(crate) const KEYRING_SERVICE: &str = "com.patchy23.patchybox";

/// 一个主密钥域：系统密钥库 account + 兼容用的本地降级密钥文件
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeySpec {
    /// 该域在系统密钥库中的 account 名
    pub(crate) account: &'static str,
    /// 降级/兼容密钥文件名（相对该域的数据目录）
    pub(crate) fallback_file: &'static str,
    /// 诊断日志前缀（不带秘密）
    pub(crate) scope: &'static str,
}

/// Vault 凭证库主密钥域
pub(crate) const VAULT_KEY_SPEC: KeySpec = KeySpec {
    account: "vault-master-key",
    fallback_file: "vault-master.key",
    scope: "vault",
};

/// credentials 兼容 KV 命名空间主密钥域（历史密钥文件名为 credentials-master.key）
pub(crate) const CREDENTIALS_KEY_SPEC: KeySpec = KeySpec {
    account: "credentials-master-key",
    fallback_file: "credentials-master.key",
    scope: "credentials",
};

/// 主密钥外部存储抽象（按 account 读写，避免「实现里的 account 与域不一致」的静默错配）
pub(crate) trait MasterKeyStore {
    /// 读取主密钥（`Ok(None)` = 密钥库中该 account 没有记录）
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String>;
    /// 写入主密钥
    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String>;
}

/// 指定 service 的系统密钥库实现。
///
/// 生产用 `KEYRING_SERVICE`；测试用专用 service（如 `com.patchy23.patchybox.tests`），
/// 避免测试条目混进用户真实凭据管理器。target 名由 keyring 拼成 `{account}.{service}`。
pub(crate) struct ScopedKeyringStore<'a> {
    /// 系统密钥库中的 service 名
    service: &'a str,
}

impl<'a> ScopedKeyringStore<'a> {
    /// 构造（只绑定 service 名，不接触密钥材料）
    pub(crate) fn new(service: &'a str) -> Self {
        Self { service }
    }

    /// 删除条目（测试清理用）。条目不存在不算失败，重复清理不应报错。
    #[cfg(test)]
    pub(crate) fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(self.service, account)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("密钥库条目删除失败: {e}")),
        }
    }
}

impl MasterKeyStore for ScopedKeyringStore<'_> {
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        let entry = keyring::Entry::new(self.service, account)
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

    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String> {
        let entry = keyring::Entry::new(self.service, account)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        entry
            .set_secret(key)
            .map_err(|e| format!("密钥库写入失败: {e}"))
    }
}

/// 生产用系统密钥库实现（keyring crate；Windows Credential Manager / macOS Keychain）
pub(crate) struct KeyringStore;

impl MasterKeyStore for KeyringStore {
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        ScopedKeyringStore::new(KEYRING_SERVICE).read(account)
    }

    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String> {
        ScopedKeyringStore::new(KEYRING_SERVICE).write(account, key)
    }
}

/// 本平台是否编译进了系统密钥库原生后端。
/// 两个原生 feature 在 `Cargo.toml` 中按平台启用；非 Windows/macOS 目标只有 keyring 的 mock，不能算系统保护。
pub(crate) fn native_backend_available() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

/// 主密钥来源（决定设置页展示的保护状态）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeySource {
    /// 系统密钥库
    Keyring,
    /// 本地降级密钥文件
    FallbackFile,
    /// 本次首次生成
    CreatedNow,
}

/// 主密钥解析结果
pub(crate) struct ResolvedKey {
    /// 解析出的主密钥
    pub(crate) key: [u8; 32],
    /// 来源
    pub(crate) source: KeySource,
    /// 过程中的诊断说明（无秘密，可直接进日志或返回前端）
    pub(crate) notes: Vec<String>,
}

/// 手写 Debug：**绝不输出密钥字节**（panic 信息与日志都可能带上 Debug 输出）
impl std::fmt::Debug for ResolvedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedKey")
            .field("key", &"[已隐去]")
            .field("source", &self.source)
            .field("notes", &self.notes)
            .finish()
    }
}

/// 已尝试过「降级密钥登记进系统密钥库」的 account（进程内只试一次，避免每次读都重写密钥库）
fn promotion_attempted() -> &'static Mutex<HashSet<&'static str>> {
    static ATTEMPTED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    ATTEMPTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 测试用：清掉「本进程已尝试登记」的记录。
/// 断言「旧密钥被登记进密钥库」的用例必须调用，否则结果取决于同进程内谁先跑（顺序依赖即假失败）。
/// 生产语义不变：同一进程内每个 account 仍只尝试一次。
#[cfg(test)]
pub(crate) fn reset_promotion_attempts() {
    if let Ok(mut guard) = promotion_attempted().lock() {
        guard.clear();
    }
}

/// 描述一次密钥库回读结果，**只含存在性与长度信息，不含任何密钥字节**。
/// 单独成函数是为了让「日志不泄露秘密」可以被单测直接断言。
pub(crate) fn describe_readback(scope: &str, result: &Result<Option<[u8; 32]>, String>) -> String {
    match result {
        Ok(Some(_)) => format!("[{scope}] 回读到的主密钥与写入值不一致（长度正确）"),
        Ok(None) => format!("[{scope}] 回读时找不到刚写入的记录"),
        Err(error) => format!("[{scope}] 回读失败：{error}"),
    }
}

/// 读降级密钥文件（不存在返回 None；长度不是 32 字节视为损坏并报错，文件保持原样）
fn read_fallback_key(dir: &Path, spec: &KeySpec) -> Result<Option<[u8; 32]>, String> {
    let path = dir.join(spec.fallback_file);
    let Some(bytes) = super::file::read_optional(&path)? else {
        return Ok(None);
    };
    let key: [u8; 32] = bytes.try_into().map_err(|_| {
        format!(
            "降级主密钥文件损坏（长度不是 32 字节，文件保持原样）：{}",
            path.display()
        )
    })?;
    Ok(Some(key))
}

/// 生成 32B 随机主密钥并写入系统密钥库；写失败或写后回读校验不过时降级本地文件（收紧权限）并告警
fn create_master_key(
    dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    // 写入后立刻回读校验：Windows 凭据管理器存在「写返回成功但条目没落库」的静默丢失场景，
    // 不校验的话密文会用一把没存住的密钥加密，下次启动永远无法解锁（死数据）
    let persisted = match store.write(spec.account, &key) {
        Ok(()) => match store.read(spec.account) {
            Ok(Some(readback)) if readback == key => true,
            other => {
                eprintln!(
                    "{}；主密钥降级为本地密钥文件",
                    describe_readback(spec.scope, &other)
                );
                false
            }
        },
        Err(e) => {
            eprintln!(
                "[{}] 密钥库写入失败（{e}）；主密钥降级为本地密钥文件",
                spec.scope
            );
            false
        }
    };
    if !persisted {
        let path = dir.join(spec.fallback_file);
        replace_file(&path, &key)?;
        if let Err(e) = restrict_to_current_user(&path) {
            eprintln!(
                "[{}] 降级密钥文件权限收紧失败（文件已写入，权限维持默认）: {e}",
                spec.scope
            );
        }
    }
    Ok(key)
}

/// 把已验证可用的降级密钥登记进系统密钥库（写后回读校验），不删除降级文件
fn promote_fallback_key(
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    key: &[u8; 32],
    notes: &mut Vec<String>,
) {
    let already = promotion_attempted()
        .lock()
        .map(|mut guard| !guard.insert(spec.account))
        .unwrap_or(false);
    if already {
        notes.push("本地降级密钥仍在使用（本次进程已尝试登记过系统密钥库）".into());
        return;
    }
    match store.write(spec.account, key) {
        Ok(()) => match store.read(spec.account) {
            Ok(Some(readback)) if readback == *key => {
                notes.push("已将本地降级密钥登记到系统密钥库，未删除本地密钥文件".into())
            }
            other => notes.push(describe_readback(spec.scope, &other)),
        },
        Err(e) => notes.push(format!("[{}] 降级密钥登记失败：{e}", spec.scope)),
    }
}

/// 锁死错误文案：区分「两处都没有密钥」「候选对不上密文」等情形，都指向备份恢复入口
fn locked_message(
    spec: &KeySpec,
    has_keyring: bool,
    has_fallback: bool,
    notes: &[String],
) -> String {
    let mut message = match (has_keyring, has_fallback) {
        (true, true) => format!(
            "无法解锁（{}）：系统密钥库与本地密钥文件中的主密钥都无法解密现有密文（可能密文损坏或被替换）。未生成新密钥，文件均保持原样；可用「凭证管理 → 导入」恢复备份",
            spec.scope
        ),
        (true, false) => format!(
            "无法解锁（{}）：系统密钥库中的主密钥无法解密现有密文。未生成新密钥，文件保持原样；可用「凭证管理 → 导入」恢复备份，或确认后删除密文重新初始化",
            spec.scope
        ),
        (false, true) => format!(
            "无法解锁（{}）：本地密钥文件中的主密钥无法解密现有密文。未生成新密钥，文件保持原样；可用「凭证管理 → 导入」恢复备份",
            spec.scope
        ),
        (false, false) => format!(
            "无法解锁（{}）：系统密钥库与本地都没有主密钥，而密文仍在（可能换机 / 重装 / 密钥被清空）。未生成新密钥；可用「凭证管理 → 导入」恢复备份，或确认后删除密文重新初始化",
            spec.scope
        ),
    };
    for note in notes {
        message.push('；');
        message.push_str(note);
    }
    message
}

/// 解析主密钥。`evidence` 是该域现有密文字节（主文件与备份；空表表示还没有密文）。
/// 降级密钥通过认证时会被登记进系统密钥库（见 `promote_fallback_key`）。
pub(crate) fn resolve_master_key(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
) -> Result<ResolvedKey, String> {
    resolve_master_key_inner(key_dir, spec, store, evidence, true)
}

/// 只读解析主密钥：与 `resolve_master_key` 同判据，但**不写系统密钥库**。
/// 供保护状态查询这类不该有写副作用的调用点使用（否则「打开设置页」会悄悄改密钥库）。
pub(crate) fn resolve_master_key_readonly(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
) -> Result<ResolvedKey, String> {
    resolve_master_key_inner(key_dir, spec, store, evidence, false)
}

/// 解析主密钥的统一实现；`allow_promotion` 决定是否允许把降级密钥登记进系统密钥库。
fn resolve_master_key_inner(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
    allow_promotion: bool,
) -> Result<ResolvedKey, String> {
    std::fs::create_dir_all(key_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let mut notes = Vec::new();
    let keyring = store.read(spec.account);
    if let Err(e) = &keyring {
        // 读取失败只告警继续降级（无桌面环境、密钥库被策略禁用等场景）
        notes.push(format!("[{}] {e}，已尝试本地密钥文件", spec.scope));
    }
    let fallback = read_fallback_key(key_dir, spec);
    if let Err(e) = &fallback {
        notes.push(e.clone());
    }
    let keyring_key = keyring.as_ref().ok().and_then(|option| *option);
    let fallback_key = fallback.as_ref().ok().and_then(|option| *option);
    let has_ciphertext = evidence.iter().any(|data| !data.is_empty());

    if has_ciphertext {
        // 有密文：以能否认证解出既有密文为准，绝不生成新密钥
        let worth = |key: [u8; 32]| evidence.iter().any(|data| authenticates(&key, data));
        if let Some(key) = keyring_key.filter(|key| worth(*key)) {
            return Ok(ResolvedKey {
                key,
                source: KeySource::Keyring,
                notes,
            });
        }
        if let Some(key) = fallback_key.filter(|key| worth(*key)) {
            // 密钥库里的版本不对或用不上时，用本地文件里认证通过的那把覆盖登记，
            // 否则下次读取仍要从降级路径兜底
            if allow_promotion {
                promote_fallback_key(spec, store, &key, &mut notes);
            }
            return Ok(ResolvedKey {
                key,
                source: KeySource::FallbackFile,
                notes,
            });
        }
        return Err(locked_message(
            spec,
            keyring_key.is_some(),
            fallback_key.is_some(),
            &notes,
        ));
    }

    // 无密文：可以安全初始化，顺序为密钥库 → 降级文件 → 首次生成
    if let Some(key) = keyring_key {
        return Ok(ResolvedKey {
            key,
            source: KeySource::Keyring,
            notes,
        });
    }
    if let Some(key) = fallback_key {
        if allow_promotion {
            promote_fallback_key(spec, store, &key, &mut notes);
        }
        return Ok(ResolvedKey {
            key,
            source: KeySource::FallbackFile,
            notes,
        });
    }
    // 降级文件存在但损坏时不要静默换一把新密钥：先把损坏事实暴露出来
    let _ = fallback?;
    let key = create_master_key(key_dir, spec, store)?;
    Ok(ResolvedKey {
        key,
        source: KeySource::CreatedNow,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::super::crypto::encrypt_payload;
    use super::super::test_support::{promotion_test_guard, MemoryKeyStore};
    use super::*;
    use std::path::PathBuf;

    /// 测试用临时目录（进程 id + 名称唯一）
    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "secure-store-key-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 测试辅助：写入降级密钥文件（模拟历史遗留的本地密钥）
    fn write_fallback(dir: &Path, spec: &KeySpec, key: &[u8; 32]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(spec.fallback_file), key).unwrap();
    }

    /// 测试用轻量 base64 编码（仅为断言「日志里没有 base64 形式的密钥」，不引入依赖）
    fn base64(data: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    /// 首次生成：无密文且两处无密钥 → 生成并写入密钥库；二次解析复用同一把
    #[test]
    fn creates_key_when_nothing_exists() {
        let dir = temp_dir("create");
        let store = MemoryKeyStore::new();
        let first = resolve_master_key(&dir, &CREDENTIALS_KEY_SPEC, &store, &[]).unwrap();
        assert_eq!(first.source, KeySource::CreatedNow);
        assert_eq!(
            store.key_of(CREDENTIALS_KEY_SPEC.account).unwrap(),
            Some(first.key)
        );
        let second = resolve_master_key(&dir, &CREDENTIALS_KEY_SPEC, &store, &[]).unwrap();
        assert_eq!(second.source, KeySource::Keyring);
        assert_eq!(second.key, first.key);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥库读失败（无桌面环境）→ 降级本地密钥文件，并收紧文件权限
    #[test]
    fn falls_back_to_local_file_when_keyring_fails() {
        let dir = temp_dir("fallback");
        let store = MemoryKeyStore::new();
        store.set_fail_read(true);
        let resolved = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[]).unwrap();
        assert_eq!(resolved.source, KeySource::CreatedNow);
        let path = dir.join(VAULT_KEY_SPEC.fallback_file);
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), resolved.key.to_vec());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "降级密钥文件应为 0600");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密钥库静默丢写（write 返回 Ok 但回读无记录）→ 回读校验失败必须降级文件，
    /// 否则密文会用没存住的密钥加密成死数据（Windows 凭据管理器真实场景回归）
    #[test]
    fn silent_write_loss_falls_back_to_file() {
        let dir = temp_dir("silent-loss");
        let store = MemoryKeyStore::new();
        store.set_lose_writes(true);
        let first = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[]).unwrap();
        assert!(dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        let second = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[]).unwrap();
        assert_eq!(first.key, second.key);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 关键回归：密钥库里有一把「新的」密钥、密文是用降级文件里的旧密钥加密的
    /// → 必须采用能认证密文的旧密钥，而不是「密钥库优先」把旧数据判成损坏
    #[test]
    fn existing_ciphertext_decides_between_candidates() {
        let dir = temp_dir("conflict");
        let old_key = [0x11u8; 32];
        let new_key = [0x22u8; 32];
        let blob = encrypt_payload(&old_key, b"existing").unwrap();
        write_fallback(&dir, &VAULT_KEY_SPEC, &old_key);
        let store = MemoryKeyStore::new();
        store.write(VAULT_KEY_SPEC.account, &new_key).unwrap();
        let _serialize = promotion_test_guard();
        reset_promotion_attempts();

        let resolved =
            resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, std::slice::from_ref(&blob)).unwrap();
        assert_eq!(resolved.key, old_key);
        assert_eq!(resolved.source, KeySource::FallbackFile);
        // 正确旧密钥被登记进密钥库（不删本地文件）
        assert_eq!(store.key_of(VAULT_KEY_SPEC.account).unwrap(), Some(old_key));
        assert!(dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密文存在而两处都没有密钥 → 锁死报错，不生成新密钥、不新增降级文件
    #[test]
    fn lost_key_never_regenerates() {
        let dir = temp_dir("lost-key");
        let key = [0x33u8; 32];
        let blob = encrypt_payload(&key, b"[]").unwrap();
        let store = MemoryKeyStore::new();
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[blob]).unwrap_err();
        assert!(error.contains("无法解锁"), "错误文案应说明锁死: {error}");
        assert!(!dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        assert_eq!(store.key_of(VAULT_KEY_SPEC.account).unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密文与两处候选都对不上 → 锁死，且明确列出两处都已尝试
    #[test]
    fn mismatched_candidates_lock_without_new_key() {
        let dir = temp_dir("mismatch");
        let blob = encrypt_payload(&[0x44u8; 32], b"[]").unwrap();
        let store = MemoryKeyStore::new();
        store.write(VAULT_KEY_SPEC.account, &[0x55u8; 32]).unwrap();
        write_fallback(&dir, &VAULT_KEY_SPEC, &[0x66u8; 32]);
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[blob]).unwrap_err();
        assert!(
            error.contains("系统密钥库与本地密钥文件"),
            "应说明两处都试过: {error}"
        );
        // 两处原样保留，没有互相覆盖
        assert_eq!(
            store.key_of(VAULT_KEY_SPEC.account).unwrap(),
            Some([0x55u8; 32])
        );
        assert_eq!(
            std::fs::read(dir.join(VAULT_KEY_SPEC.fallback_file)).unwrap(),
            [0x66u8; 32].to_vec()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 降级文件损坏 → 无密文时明确报错，不静默换一把新密钥
    #[test]
    fn corrupt_fallback_file_is_reported() {
        let dir = temp_dir("corrupt-fallback");
        std::fs::write(dir.join(VAULT_KEY_SPEC.fallback_file), b"short").unwrap();
        let store = MemoryKeyStore::new();
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[]).unwrap_err();
        assert!(error.contains("降级主密钥文件损坏"), "{error}");
        assert_eq!(
            std::fs::read(dir.join(VAULT_KEY_SPEC.fallback_file)).unwrap(),
            b"short"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 诊断文案不含秘密：不匹配的密钥不得以原字节 / 十六进制 / base64 形式出现在日志文案里
    #[test]
    fn readback_note_never_leaks_key_material() {
        let key = [0xABu8; 32];
        let note = describe_readback("vault", &Ok(Some(key)));
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        assert!(!note.contains(&hex), "文案泄露了十六进制密钥: {note}");
        assert!(
            !note.contains(&hex.to_uppercase()),
            "文案泄露了大写十六进制密钥"
        );
        assert!(!note.contains(&base64(&key)), "文案泄露了 base64 密钥");
        assert!(
            !note.contains(&format!("{key:?}")),
            "文案泄露了原始字节数组"
        );
        assert!(note.contains("长度正确"), "文案应只说明长度/存在性: {note}");
        // 读失败与无记录两种分支同样不带密钥材料
        let missing = describe_readback("vault", &Ok(None));
        assert!(missing.contains("找不到"));
        let failed = describe_readback("vault", &Err("密钥库读取失败: 拒绝访问".into()));
        assert!(failed.contains("拒绝访问"));
    }

    /// 降级密钥登记只尝试一次：第二次解析不再写系统密钥库
    /// （用独立 account，避免与其它用例共享进程内「只尝试一次」状态）
    #[test]
    fn promotion_attempted_only_once_per_process() {
        /// 本用例独占的密钥域
        const PROMOTE_SPEC: KeySpec = KeySpec {
            account: "test-promote-once-account",
            fallback_file: "test-promote-once.key",
            scope: "test",
        };
        let dir = temp_dir("promote-once");
        let key = [0x77u8; 32];
        write_fallback(&dir, &PROMOTE_SPEC, &key);
        let store = MemoryKeyStore::new();
        store.set_fail_write(true);
        let _serialize = promotion_test_guard();
        reset_promotion_attempts();
        let first = resolve_master_key(&dir, &PROMOTE_SPEC, &store, &[]).unwrap();
        assert_eq!(store.write_attempts(), 1);
        let second = resolve_master_key(&dir, &PROMOTE_SPEC, &store, &[]).unwrap();
        assert_eq!(store.write_attempts(), 1, "同一进程不应反复重写系统密钥库");
        assert_eq!(first.key, second.key);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 平台后端判定：只有 Windows/macOS 才算系统密钥库原生后端
    #[test]
    fn native_backend_flag_matches_target() {
        assert_eq!(
            native_backend_available(),
            cfg!(any(target_os = "windows", target_os = "macos"))
        );
    }
}
