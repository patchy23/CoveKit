//! 框架级本地凭证管理（AES-256-GCM 加密存储）
//! 设计：插件不关心凭证怎么存怎么取——按「命名空间 + 键」读写即可；
//! 存储：app_data_dir/credentials/<namespace>.enc（每命名空间一个文件，
//! nonce(12B)||ciphertext，明文为 JSON map<String, Value>），
//! 主密钥 app_data_dir/credentials-master.key（32B，一次性生成）。
//! 后续若更换存储实现（如系统钥匙串/stronghold），只需改本模块。
//! 安全性：命名空间白名单防路径穿越；原子替换 + 备份恢复防半截密文。

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use tauri::{AppHandle, Manager};

/// 凭证目录（app_data_dir 下）
const CREDENTIALS_DIR: &str = "credentials";
/// 框架主密钥文件名（32 字节随机）
const MASTER_KEY_FILE: &str = "credentials-master.key";

/// 命名空间合法字符（防路径穿越）
fn valid_namespace(namespace: &str) -> bool {
    !namespace.is_empty()
        && namespace.len() <= 32
        && namespace
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 凭证文件进程内互斥锁（并发写防丢更新；全局一把锁最简单）
fn credential_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ──────────────────────────────────────────────────────────────────────────
// 核心实现（以目录为参数，便于单测）
// ──────────────────────────────────────────────────────────────────────────

/// 主密钥：已存在则读取，否则生成 32 随机字节落盘（一次性）
fn master_key_at(dir: &Path) -> Result<[u8; 32], String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let path = dir.join(MASTER_KEY_FILE);
    if path.exists() {
        let bytes = std::fs::read(&path).map_err(|e| format!("主密钥读取失败: {e}"))?;
        if bytes.len() != 32 {
            return Err("主密钥文件损坏（长度不是 32 字节），为避免凭证丢失已停止操作".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    replace_file(&path, &key).map_err(|e| format!("主密钥写入失败: {e}"))?;
    Ok(key)
}

/// 凭证文件路径（校验命名空间）
fn secrets_file_at(dir: &Path, namespace: &str) -> Result<PathBuf, String> {
    if !valid_namespace(namespace) {
        return Err(format!("凭证命名空间非法：{namespace}"));
    }
    Ok(dir.join(CREDENTIALS_DIR).join(format!("{namespace}.enc")))
}

/// 使用主密钥解密 nonce(12B)||ciphertext，先校验最小长度避免损坏文件触发 panic。
fn decrypt_payload(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
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

/// 使用主密钥加密明文，返回 nonce(12B)||ciphertext。
fn encrypt_payload(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
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

/// 先完整写入临时文件并刷盘，再替换目标，避免进程中断留下半截密文。
fn replace_file(path: &Path, content: &[u8]) -> Result<(), String> {
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
        eprintln!("[credentials] 凭证已保存，但备份清理失败: {error}");
    }
    Ok(())
}

/// 读取命名空间全部凭证（解密；无文件时返回空表）
fn read_map_at(dir: &Path, namespace: &str) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = secrets_file_at(dir, namespace)?;
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(format!("凭证文件读取失败: {error}")),
    };
    let key = master_key_at(dir)?;
    let plain = decrypt_payload(&key, &data)?;
    serde_json::from_slice(&plain).map_err(|e| format!("凭证数据解析失败: {e}"))
}

/// 写回命名空间全部凭证（加密落盘）
fn write_map_at(
    dir: &Path,
    namespace: &str,
    map: &HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let key = master_key_at(dir)?;
    let plain = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    let out = encrypt_payload(&key, &plain)?;
    replace_file(&secrets_file_at(dir, namespace)?, &out)
}

// ──────────────────────────────────────────────────────────────────────────
// 对外 API（AppHandle 封装）
// ──────────────────────────────────────────────────────────────────────────

/// 应用数据目录
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))
}

/// 凭证文件路径（供前端展示/诊断）
pub fn secrets_file(app: &AppHandle, namespace: &str) -> Result<PathBuf, String> {
    secrets_file_at(&app_data_dir(app)?, namespace)
}

/// 保存/更新凭证（命名空间 + 键 + 任意 JSON 值）
pub fn save_secret(
    app: &AppHandle,
    namespace: &str,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let _guard = credential_lock().lock().map_err(|e| e.to_string())?;
    let dir = app_data_dir(app)?;
    let mut map = read_map_at(&dir, namespace)?;
    map.insert(key.to_string(), value.clone());
    write_map_at(&dir, namespace, &map)
}

/// 读取凭证（无记录返回 None）
pub fn get_secret(
    app: &AppHandle,
    namespace: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let _guard = credential_lock().lock().map_err(|e| e.to_string())?;
    let dir = app_data_dir(app)?;
    Ok(read_map_at(&dir, namespace)?.get(key).cloned())
}

/// 删除凭证
pub fn delete_secret(app: &AppHandle, namespace: &str, key: &str) -> Result<(), String> {
    let _guard = credential_lock().lock().map_err(|e| e.to_string())?;
    let dir = app_data_dir(app)?;
    let mut map = read_map_at(&dir, namespace)?;
    map.remove(key);
    write_map_at(&dir, namespace, &map)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 加解密往返 + 错误密钥/损坏密文校验
    #[test]
    fn 加解密往返() {
        let key = [7u8; 32];
        let plain = b"{\"conn-1\":\"s3cret\"}";
        let ct = encrypt_payload(&key, plain).unwrap();
        assert_eq!(decrypt_payload(&key, &ct).unwrap(), plain);
        let wrong = [8u8; 32];
        assert!(decrypt_payload(&wrong, &ct).is_err());
        assert!(decrypt_payload(&key, b"short").is_err());
    }

    /// 命名空间白名单
    #[test]
    fn 命名空间白名单() {
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

    /// 完整链路：主密钥生成复用 + 保存/读取/删除往返 + 命名空间隔离
    #[test]
    fn 凭证保存读取删除往返() {
        let dir = std::env::temp_dir().join(format!("cred-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // 主密钥：首次生成，二次复用
        let k1 = master_key_at(&dir).unwrap();
        let k2 = master_key_at(&dir).unwrap();
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);

        // 保存/读取
        write_map_at(
            &dir,
            "database",
            &HashMap::from([("conn-1".into(), serde_json::json!("s3cret"))]),
        )
        .unwrap();
        let map = read_map_at(&dir, "database").unwrap();
        assert_eq!(map.get("conn-1").and_then(|v| v.as_str()), Some("s3cret"));

        // 命名空间隔离：另一命名空间为空
        assert!(read_map_at(&dir, "ssh").unwrap().is_empty());

        // 删除
        let mut m = read_map_at(&dir, "database").unwrap();
        m.remove("conn-1");
        write_map_at(&dir, "database", &m).unwrap();
        assert!(read_map_at(&dir, "database").unwrap().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 损坏主密钥报错
    #[test]
    fn 损坏主密钥报错() {
        let dir = std::env::temp_dir().join(format!("cred-badkey-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MASTER_KEY_FILE), b"short").unwrap();
        assert!(master_key_at(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
