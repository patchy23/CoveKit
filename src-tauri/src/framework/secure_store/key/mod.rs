//! 主密钥来源：系统密钥库（keyring 原生后端）→ 本地降级密钥文件 → 首次生成
//!
//! 核心规则（T04/T05）：
//! 1. **有既有密文时以密文为准**：候选密钥（密钥库 / 降级文件）必须能通过 GCM 认证才被采用，
//!    绝不因为「密钥库优先」而掩盖本地降级文件里的正确旧密钥；
//! 2. **绝不生成会覆盖既有密文的新密钥**：两处候选都对不上既有密文时返回锁死错误，
//!    由用户走备份导入或确认后重新初始化；
//! 3. 降级文件里的正确旧密钥会被**登记**进系统密钥库（写后回读校验），但**不删除降级文件**；
//! 4. 诊断文案只描述「有没有记录 / 长度对不对 / 读了还是写失败」，不打印任何密钥字节。

/// 系统密钥库 service 前缀（历史默认空间的条目就在这个裸 service 下，由空间化迁移搬走）。
pub(crate) const KEYRING_SERVICE: &str = "com.patchy23.patchybox";

/// 指定空间的系统密钥库 service 名（`<前缀>.<空间 uid>`）。
///
/// 每个空间独立分服务：目录分开了但条目共用一把密钥 = 伪隔离。空间 uid 由迁移与
/// 校验链路保证合法（小写 UUIDv4），这里不做二次校验。
pub(crate) fn keyring_service(space_id: &str) -> String {
    format!("{KEYRING_SERVICE}.{space_id}")
}

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
/// 生产按空间作用域构造（`keyring_store_for`）；测试用专用 service
/// （如 `com.patchy23.patchybox.tests`），避免测试条目混进用户真实凭据管理器。
/// target 名由 keyring 拼成 `{account}.{service}`。
pub(crate) struct ScopedKeyringStore {
    /// 系统密钥库中的 service 名（按空间作用域，构造后不变）
    service: String,
}

impl ScopedKeyringStore {
    /// 构造（只绑定 service 名，不接触密钥材料）
    pub(crate) fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// 删除条目（空间化迁移与测试清理用）。条目不存在不算失败，重复清理不应报错。
    pub(crate) fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(&self.service, account)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("密钥库条目删除失败: {e}")),
        }
    }
}

impl MasterKeyStore for ScopedKeyringStore {
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        let entry = keyring::Entry::new(&self.service, account)
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
        let entry = keyring::Entry::new(&self.service, account)
            .map_err(|e| format!("密钥库初始化失败: {e}"))?;
        entry
            .set_secret(key)
            .map_err(|e| format!("密钥库写入失败: {e}"))
    }
}

/// 生产用系统密钥库访问（按空间作用域；默认空间沿用历史 service）
pub(crate) fn keyring_store_for(space_id: &str) -> ScopedKeyringStore {
    ScopedKeyringStore::new(keyring_service(space_id))
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
mod resolve;

#[cfg(test)]
pub(crate) use resolve::reset_promotion_attempts;
pub(crate) use resolve::{resolve_master_key, resolve_master_key_readonly};

#[cfg(test)]
mod tests {

    use super::super::crypto::encrypt_payload;
    use super::super::test_support::{promotion_test_guard, MemoryKeyStore};
    use super::resolve::*;
    use super::*;
    use std::path::{Path, PathBuf};

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
        let first = resolve_master_key(&dir, &CREDENTIALS_KEY_SPEC, &store, &[], &[]).unwrap();
        assert_eq!(first.source, KeySource::CreatedNow);
        assert_eq!(
            store.key_of(CREDENTIALS_KEY_SPEC.account).unwrap(),
            Some(first.key)
        );
        let second = resolve_master_key(&dir, &CREDENTIALS_KEY_SPEC, &store, &[], &[]).unwrap();
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
        let resolved = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[], &[]).unwrap();
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
        let first = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[], &[]).unwrap();
        assert!(dir.join(VAULT_KEY_SPEC.fallback_file).exists());
        let second = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[], &[]).unwrap();
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

        let resolved = resolve_master_key(
            &dir,
            &VAULT_KEY_SPEC,
            &store,
            std::slice::from_ref(&blob),
            &[],
        )
        .unwrap();
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
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[blob], &[]).unwrap_err();
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
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[blob], &[]).unwrap_err();
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
        let error = resolve_master_key(&dir, &VAULT_KEY_SPEC, &store, &[], &[]).unwrap_err();
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
        let first = resolve_master_key(&dir, &PROMOTE_SPEC, &store, &[], &[]).unwrap();
        assert_eq!(store.write_attempts(), 1);
        let second = resolve_master_key(&dir, &PROMOTE_SPEC, &store, &[], &[]).unwrap();
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

    /// 密钥库 service 按空间作用域：所有空间（含默认空间）都是 `<前缀>.<uid>` 形态，
    /// 两空间不会落到同一组条目上；裸前缀只作历史默认空间条目的迁移来源
    #[test]
    fn keyring_service_is_scoped_per_space() {
        const SPACE_A: &str = "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23";
        const SPACE_B: &str = "7c1d0a94-2b6f-4e83-8f52-1a9de4c7b305";

        assert_eq!(
            keyring_service(SPACE_A),
            format!("{KEYRING_SERVICE}.{SPACE_A}")
        );
        assert_ne!(keyring_service(SPACE_A), keyring_service(SPACE_B));
        assert_ne!(keyring_service(SPACE_A), KEYRING_SERVICE);
        // account 不随空间变：同一空间里两个域的条目名保持稳定
        assert_eq!(VAULT_KEY_SPEC.account, "vault-master-key");
        assert_eq!(CREDENTIALS_KEY_SPEC.account, "credentials-master-key");
    }

    /// 空间密钥隔离（夹具构造第二空间）：A 空间密文用 B 空间密钥解不开，
    /// 且在 B 空间里解析 A 的密文必须锁死报错，不得生成新密钥冒充成功
    ///
    /// 只分目录不分密钥库 service 时，两空间会共用同一把主密钥，这条用例就会失败。
    #[test]
    fn space_keys_do_not_open_each_others_ciphertext() {
        const SPACE_A: &str = "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23";
        const SPACE_B: &str = "7c1d0a94-2b6f-4e83-8f52-1a9de4c7b305";

        let dir_a = temp_dir("space-a");
        let dir_b = temp_dir("space-b");
        // 两个空间各自的密钥库（生产环境里由 service 名字隔开）
        let store_a = MemoryKeyStore::new();
        let store_b = MemoryKeyStore::new();
        let key_a = resolve_master_key(&dir_a, &CREDENTIALS_KEY_SPEC, &store_a, &[], &[])
            .unwrap()
            .key;
        let blob = encrypt_payload(&key_a, b"{\"database\":\"secret\"}").unwrap();

        // A 空间能解开自己的密文
        let reopened = resolve_master_key(
            &dir_a,
            &CREDENTIALS_KEY_SPEC,
            &store_a,
            std::slice::from_ref(&blob),
            &[],
        )
        .unwrap();
        assert_eq!(reopened.key, key_a, "同一空间应能解开自己的密文");

        // B 空间：既没有这条条目，也不该用新密钥「成功」打开 A 的密文
        let error =
            resolve_master_key(&dir_b, &CREDENTIALS_KEY_SPEC, &store_b, &[blob], &[]).unwrap_err();
        assert!(error.contains("无法解锁"), "跨空间读取应锁死: {error}");
        assert_eq!(
            store_b.key_of(CREDENTIALS_KEY_SPEC.account).unwrap(),
            None,
            "跨空间解析不得在 B 空间登记任何密钥"
        );
        assert!(
            !dir_b.join(CREDENTIALS_KEY_SPEC.fallback_file).exists(),
            "跨空间解析不得在 B 空间生成降级密钥文件"
        );

        // 两空间各解析一次 → 拿到的是两把不同的密钥（目录分开 + 条目分开）
        let key_b = resolve_master_key(&dir_b, &CREDENTIALS_KEY_SPEC, &store_b, &[], &[])
            .unwrap()
            .key;
        assert_ne!(key_a, key_b, "两个空间必须持有各自的主密钥");
        assert_eq!(
            keyring_service(SPACE_A),
            format!("{KEYRING_SERVICE}.{SPACE_A}"),
            "所有空间（含默认空间）的 service 都是 <前缀>.<uid> 形态"
        );
        assert_ne!(keyring_service(SPACE_B), KEYRING_SERVICE);
        std::fs::remove_dir_all(&dir_a).ok();
        std::fs::remove_dir_all(&dir_b).ok();
    }
}
