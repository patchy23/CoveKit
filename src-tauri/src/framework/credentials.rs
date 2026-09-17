//! 框架级本地凭证管理（按「命名空间 + 键」读写任意 JSON 值）
//! - 存储：`<数据目录>/credentials/<namespace>.enc`，明文为 `JSON map<String, Value>`；
//!   加解密、主密钥来源、原子替换与备份恢复统一走 `framework/secure_store`（T05：与 Vault 共用同一套原语）
//! - 主密钥：系统密钥库 account `credentials-master-key` → 兼容本地文件 `credentials-master.key` → 首次生成；
//!   兼容既有密文时以密文为准（文件里的旧密钥能解开既有 `.enc` 就采用它并把来源如实上报），
//!   两处候选都解不开则锁死报错，**绝不生成会覆盖既有密文的新密钥**
//! - 安全性：命名空间白名单防路径穿越；写路径唯一临时名 + fsync + 旧文件转 `.bak`；
//!   读路径先恢复中断遗留备份再判空，只有主文件与备份都不存在才当空数据

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use crate::framework::secure_store::{
    ciphertext_evidence, encrypt_with_aad, load_with_binding, replace_file, resolve_master_key,
    MasterKeyStore, CREDENTIALS_KEY_SPEC,
};

/// 密文备份路径（与 secure_store 的原子替换协议一致：`X.enc` → `X.bak`）
pub(crate) use crate::framework::secure_store::backup_path;

/// 凭证目录（数据分区下）
const CREDENTIALS_DIR: &str = "credentials";

/// 命名空间合法字符（防路径穿越）
fn valid_namespace(namespace: &str) -> bool {
    !namespace.is_empty()
        && namespace.len() <= 32
        && namespace
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 凭证文件进程内互斥锁（读改写整体串行，避免并发丢更新；全局一把锁最简单）
fn credential_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ──────────────────────────────────────────────────────────────────────────
// 核心实现（以目录为参数，便于单测）
// ──────────────────────────────────────────────────────────────────────────

/// 命名空间密文文件路径（校验命名空间）
fn secrets_file_at(dir: &Path, namespace: &str) -> Result<PathBuf, String> {
    if !valid_namespace(namespace) {
        return Err(format!("凭证命名空间非法：{namespace}"));
    }
    Ok(dir.join(CREDENTIALS_DIR).join(format!("{namespace}.enc")))
}

/// 解析该命名空间的主密钥（有既有密文时以能否解开密文为准；`aad` = 空间 uid 绑定）
fn key_for_at(
    dir: &Path,
    path: &Path,
    store: &dyn MasterKeyStore,
    aad: &[u8],
) -> Result<[u8; 32], String> {
    let evidence = ciphertext_evidence(&[path.to_path_buf()])?;
    Ok(resolve_master_key(dir, &CREDENTIALS_KEY_SPEC, store, &evidence, aad)?.key)
}

/// 读取命名空间全部凭证（解密；主文件与备份都不存在时返回空表；旧格式就地升级重写）
fn read_map_at(
    dir: &Path,
    namespace: &str,
    store: &dyn MasterKeyStore,
    space_id: &str,
) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = secrets_file_at(dir, namespace)?;
    let aad = space_id.as_bytes();
    let key = key_for_at(dir, &path, store, aad)?;
    let decode = |plain: &[u8]| {
        serde_json::from_slice::<HashMap<String, serde_json::Value>>(plain)
            .map_err(|e| format!("凭证数据解析失败: {e}"))
    };
    let encode = |map: &HashMap<String, serde_json::Value>| {
        serde_json::to_vec(map).map_err(|e| format!("凭证序列化失败: {e}"))
    };
    Ok(load_with_binding(&path, &key, aad, &decode, &encode)?.unwrap_or_default())
}

/// 写回命名空间全部凭证（加密落盘，唯一临时名 + 原子替换；AAD = 空间 uid）
fn write_map_at(
    dir: &Path,
    namespace: &str,
    store: &dyn MasterKeyStore,
    map: &HashMap<String, serde_json::Value>,
    space_id: &str,
) -> Result<(), String> {
    let path = secrets_file_at(dir, namespace)?;
    let aad = space_id.as_bytes();
    let key = key_for_at(dir, &path, store, aad)?;
    let plain = serde_json::to_vec(map).map_err(|e| format!("凭证序列化失败: {e}"))?;
    let out = encrypt_with_aad(&key, &plain, aad)?;
    replace_file(&path, &out)
}

/// 锁内读改写（调用方不必自己拼「读 → 改 → 写」，并发保存不同键都不丢）
fn update_map_at(
    dir: &Path,
    namespace: &str,
    store: &dyn MasterKeyStore,
    space_id: &str,
    update: impl FnOnce(&mut HashMap<String, serde_json::Value>),
) -> Result<(), String> {
    let _guard = credential_lock().lock().map_err(|e| e.to_string())?;
    let mut map = read_map_at(dir, namespace, store, space_id)?;
    update(&mut map);
    write_map_at(dir, namespace, store, &map, space_id)
}

// ──────────────────────────────────────────────────────────────────────────
// 对外 API（AppHandle 封装）
// ──────────────────────────────────────────────────────────────────────────

/// 框架数据分区目录（经 `framework::paths` 统一解析；空间化后无旧布局回落——
/// 旧位置的读取责任由启动维护窗口的布局迁移承担，不在读路径上猜）
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let location = crate::framework::paths::current_location(app)?;
    Ok(location.data.clone())
}

/// 凭据数据目录（含旧布局回落；保护状态查询用）
pub(crate) fn resolved_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app_data_dir(app)
}

/// 凭证文件路径（供前端展示/诊断）
pub fn secrets_file(app: &AppHandle, namespace: &str) -> Result<PathBuf, String> {
    secrets_file_at(&app_data_dir(app)?, namespace)
}

/// 该命名空间是否已有落盘凭证（主文件或中断遗留的备份都算，避免把「只剩备份」当没数据）
pub fn has_stored_data(app: &AppHandle, namespace: &str) -> Result<bool, String> {
    let path = secrets_file(app, namespace)?;
    Ok(path.exists() || backup_path(&path).exists())
}

/// 数据分区下全部命名空间密文文件（保护状态查询用：目录不存在返回空清单）
pub(crate) fn all_namespace_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = dir.join(CREDENTIALS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("凭证目录读取失败（{}）: {e}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("凭证目录读取失败（{}）: {e}", dir.display()))?;
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// 保存/更新凭证（命名空间 + 键 + 任意 JSON 值）
pub fn save_secret(
    app: &AppHandle,
    namespace: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    let value = value.clone();
    update_map_at(
        &dir,
        namespace,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
        move |map| {
            map.insert(key.to_string(), value);
        },
    )
}

/// 批量保存凭证（一次读改写、一次原子替换）
///
/// 用途：旧存储一次性迁移。逐条 `save_secret` 会在中途失败时留下「新库已有部分数据」的中间态，
/// 迁移逻辑再判断「新库有数据就跳过」就会永久丢掉剩余条目，因此迁移必须走批量写入。
pub fn save_secrets(
    app: &AppHandle,
    namespace: &str,
    values: &HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    let values = values.clone();
    update_map_at(
        &dir,
        namespace,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
        move |map| {
            for (key, value) in values {
                map.insert(key, value);
            }
        },
    )
}

/// 读取凭证（无记录返回 None）
pub fn get_secret(
    app: &AppHandle,
    namespace: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let dir = app_data_dir(app)?;
    let _guard = credential_lock().lock().map_err(|e| e.to_string())?;
    Ok(read_map_at(
        &dir,
        namespace,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
    )?
    .get(key)
    .cloned())
}

/// 删除凭证
pub fn delete_secret(app: &AppHandle, namespace: &str, key: &str) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    update_map_at(
        &dir,
        namespace,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
        move |map| {
            map.remove(key);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::secure_store::test_support::MemoryKeyStore;
    use crate::framework::secure_store::{
        authenticates, encrypt_payload, promotion_test_guard, reset_promotion_attempts,
        seed_fallback_file, CREDENTIALS_KEY_SPEC,
    };

    /// 测试用临时目录（进程 id + 随机名唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cred-store-test-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 命名空间白名单
    #[test]
    fn namespace_whitelist() {
        assert!(valid_namespace("database"));
        assert!(valid_namespace("ssh-profile"));
        assert!(valid_namespace("a_b-c1"));
        assert!(!valid_namespace(""));
        assert!(!valid_namespace(".."));
        assert!(!valid_namespace("a/b"));
        assert!(!valid_namespace("a\\b"));
        assert!(!valid_namespace("中文"));
        assert!(!valid_namespace(&"x".repeat(33)));
    }

    /// 完整链路：保存/读取/删除往返 + 命名空间隔离 + 空表只在两个文件都不存在时出现
    #[test]
    fn save_read_delete_roundtrip() {
        let dir = temp_dir("roundtrip");
        let store = MemoryKeyStore::new();

        write_map_at(
            &dir,
            "database",
            &store,
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        let map = read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(map.get("conn-1").and_then(|v| v.as_str()), Some("s3cret"));

        // 命名空间隔离：另一命名空间为空（且不产生文件）
        assert!(
            read_map_at(&dir, "ssh", &store, "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23")
                .unwrap()
                .is_empty()
        );
        assert!(!dir.join(CREDENTIALS_DIR).join("ssh.enc").exists());

        // 删除后回到空表
        update_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
            |map| {
                map.remove("conn-1");
            },
        )
        .unwrap();
        assert!(read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23"
        )
        .unwrap()
        .is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 并发保存不同键都不丢（读改写整体串行）
    #[test]
    fn concurrent_saves_do_not_lose_updates() {
        let dir = temp_dir("concurrent");
        let mut handles = Vec::new();
        for index in 0..8 {
            let dir = dir.clone();
            let store = MemoryKeyStore::with_key(CREDENTIALS_KEY_SPEC.account, [0x77u8; 32]);
            handles.push(std::thread::spawn(move || {
                update_map_at(
                    &dir,
                    "database",
                    &store,
                    "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
                    |map| {
                        map.insert(format!("conn-{index}"), serde_json::json!(index));
                    },
                )
            }));
        }
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let map = read_map_at(
            &dir,
            "database",
            &MemoryKeyStore::with_key(CREDENTIALS_KEY_SPEC.account, [0x77u8; 32]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(map.len(), 8, "并发保存不同键不应互相覆盖: {map:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃恢复：主文件已转 `.bak` 而新文件未转正 → 读回旧完整版本，不能当空表
    #[test]
    fn recovers_backup_instead_of_empty_table() {
        let dir = temp_dir("recover");
        let store = MemoryKeyStore::new();
        write_map_at(
            &dir,
            "database",
            &store,
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        // 模拟「旧文件已改名成 .bak、新文件尚未转正」的中断现场
        std::fs::rename(&path, backup_path(&path)).unwrap();

        let map = read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(
            map.get("conn-1").and_then(|v| v.as_str()),
            Some("s3cret"),
            "只剩备份时必须读回旧数据而不是空表"
        );
        assert!(path.exists(), "备份应被转正");
        assert!(!backup_path(&path).exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 主文件损坏而备份可解 → 采用备份，坏文件留档，绝不直接删备份
    #[test]
    fn adopts_backup_when_main_corrupt() {
        let dir = temp_dir("adopt");
        let store = MemoryKeyStore::new();
        write_map_at(
            &dir,
            "database",
            &store,
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        std::fs::copy(&path, backup_path(&path)).unwrap();
        std::fs::write(&path, b"broken").unwrap();

        let map = read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(map.get("conn-1").and_then(|v| v.as_str()), Some("s3cret"));
        let archives: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".corrupt-"))
            .collect();
        assert_eq!(archives.len(), 1, "坏文件应留档: {archives:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 既有密文用降级密钥文件里的旧密钥加密 → 仍然读得出来（不能用新生成的密钥覆盖语义）
    #[test]
    fn existing_ciphertext_wins_over_new_key() {
        let dir = temp_dir("existing-wins");
        let old_key = [0x21u8; 32];
        seed_fallback_file(&dir, &CREDENTIALS_KEY_SPEC, &old_key).expect("写入降级密钥文件");
        let plain = serde_json::to_vec(&HashMap::from([(
            "conn-1".to_string(),
            serde_json::json!("legacy"),
        )]))
        .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, encrypt_payload(&old_key, &plain).unwrap()).unwrap();

        // 系统密钥库里是另一把（不对应密文）的密钥；断言「旧密钥被登记」要与其它登记用例串行，
        // 并先清掉进程内的一次性记录，否则结果取决于用例执行顺序
        let store = MemoryKeyStore::with_key(CREDENTIALS_KEY_SPEC.account, [0x22u8; 32]);
        let _serialize = promotion_test_guard();
        reset_promotion_attempts();
        let map = read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(map.get("conn-1").and_then(|v| v.as_str()), Some("legacy"));
        // 正确旧密钥被登记进系统密钥库，降级文件保留
        assert_eq!(
            store.key_of(CREDENTIALS_KEY_SPEC.account).unwrap(),
            Some(old_key)
        );
        assert!(dir.join(CREDENTIALS_KEY_SPEC.fallback_file).exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密文在而两处密钥都对不上 → 锁死：报错、不生成新密钥、密文摘要不变
    #[test]
    fn locked_when_no_key_matches_ciphertext() {
        let dir = temp_dir("locked");
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let blob = encrypt_payload(&[0x33u8; 32], b"{\"conn-1\":\"secret\"}").unwrap();
        std::fs::write(&path, &blob).unwrap();

        let store = MemoryKeyStore::new();
        let error = read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap_err();
        assert!(error.contains("无法解锁"), "应报锁死: {error}");
        // 没有生成会覆盖既有密文的新密钥
        assert!(!dir.join(CREDENTIALS_KEY_SPEC.fallback_file).exists());
        assert_eq!(store.write_attempts(), 0, "锁死时不得写密钥库");
        assert_eq!(std::fs::read(&path).unwrap(), blob, "密文必须保持原样");

        // 写入路径同样锁死，不覆盖既有密文
        let write_error = update_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
            |map| {
                map.insert("conn-2".into(), serde_json::json!("x"));
            },
        )
        .unwrap_err();
        assert!(write_error.contains("无法解锁"));
        assert_eq!(std::fs::read(&path).unwrap(), blob);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 密文被篡改 → 报错且原文件摘要不变（不落空表、不改写文件）
    #[test]
    fn tampered_ciphertext_errors_without_touching_file() {
        let dir = temp_dir("tampered");
        let store = MemoryKeyStore::new();
        write_map_at(
            &dir,
            "database",
            &store,
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        let digest_before = std::fs::read(&path).unwrap();
        let mut tampered = digest_before.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        std::fs::write(&path, &tampered).unwrap();

        assert!(read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23"
        )
        .is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            tampered,
            "读失败不得改写文件"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 损坏文件不 panic：长度不足与截断密文都只报错
    #[test]
    fn short_ciphertext_errors_no_panic() {
        let dir = temp_dir("short");
        let store = MemoryKeyStore::new();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"short").unwrap();
        assert!(read_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23"
        )
        .is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 中断残留的临时文件不参与读取，也不影响下一次写入
    #[test]
    fn stale_tmp_files_are_ignored() {
        let dir = temp_dir("stale-tmp");
        let store = MemoryKeyStore::new();
        write_map_at(
            &dir,
            "database",
            &store,
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        let stale = path.with_file_name("database.enc.tmp-00000000-0000-0000-0000-000000000000");
        std::fs::write(&stale, b"garbage").unwrap();

        assert_eq!(
            read_map_at(
                &dir,
                "database",
                &store,
                "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23"
            )
            .unwrap()
            .get("conn-1")
            .and_then(|v| v.as_str()),
            Some("s3cret")
        );
        update_map_at(
            &dir,
            "database",
            &store,
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
            |map| {
                map.insert("conn-2".into(), serde_json::json!("second"));
            },
        )
        .unwrap();
        // 既有残留不被当成新内容，也不阻碍写入
        assert_eq!(std::fs::read(&stale).unwrap(), b"garbage");
        assert_eq!(
            read_map_at(
                &dir,
                "database",
                &store,
                "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23"
            )
            .unwrap()
            .len(),
            2
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 备份里的密文同样纳入密钥候选判定：只有备份时也能读出正确密钥
    #[test]
    fn backup_only_environment_uses_existing_key() {
        let dir = temp_dir("backup-only");
        let _serialize = promotion_test_guard();
        let key = [0x44u8; 32];
        seed_fallback_file(&dir, &CREDENTIALS_KEY_SPEC, &key).expect("写入降级密钥文件");
        let plain = serde_json::to_vec(&HashMap::from([("k".to_string(), serde_json::json!("v"))]))
            .unwrap();
        let path = dir.join(CREDENTIALS_DIR).join("database.enc");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let blob = encrypt_payload(&key, &plain).unwrap();
        assert!(authenticates(&key, &blob));
        std::fs::write(backup_path(&path), &blob).unwrap();

        let map = read_map_at(
            &dir,
            "database",
            &MemoryKeyStore::new(),
            "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23",
        )
        .unwrap();
        assert_eq!(map.get("k").and_then(|v| v.as_str()), Some("v"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
