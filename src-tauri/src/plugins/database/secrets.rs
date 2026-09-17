//! 数据库连接凭据 · 框架公共凭证模块薄封装
//! 存储实现（AES-GCM 加密文件、主密钥、原子写）全部在 framework::credentials，
//! 本模块只负责：命名空间「database」的读写 + 旧存储的一次性迁移。
//! 迁移链：db.stronghold（早期）→ db-secrets.enc/db-master.key（AES-GCM 阶段）
//! → credentials/database.enc（框架公共模块）。

use std::sync::Mutex;

use aes_gcm::aead::{Aead, KeyInit};

use crate::framework::credentials;

/// 数据库插件在公共凭证库中的命名空间
const NAMESPACE: &str = "database";

/// 凭据库 State（惰性执行迁移）
pub struct SecretsState(pub Mutex<Option<std::sync::Arc<()>>>);

/// 取（或首次初始化）凭据库：首次调用时执行旧存储迁移（幂等）
fn secrets(app: &tauri::AppHandle, state: &SecretsState) -> Result<std::sync::Arc<()>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        migrate_legacy(app)?;
        *guard = Some(std::sync::Arc::new(()));
    }
    guard
        .clone()
        .ok_or_else(|| "初始化未完成（内部状态异常）".to_string())
}

/// 一次性迁移旧格式凭据到框架公共凭证库（新库已有数据则跳过）
fn migrate_legacy(app: &tauri::AppHandle) -> Result<(), String> {
    let dir = crate::framework::paths::data_dir(app)?;

    // 迁移 2：AES-GCM 阶段（db-secrets.enc + db-master.key）→ 公共库
    let legacy_file = dir.join("db-secrets.enc");
    let legacy_key = dir.join("db-master.key");
    if legacy_file.exists() && legacy_key.exists() && !credentials::has_stored_data(app, NAMESPACE)?
    {
        let key_bytes = std::fs::read(&legacy_key).map_err(|e| format!("旧主密钥读取失败: {e}"))?;
        if key_bytes.len() != 32 {
            return Err(
                "旧主密钥文件损坏（长度不是 32 字节），请手动删除 db-master.key 后重试".into(),
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        let data = std::fs::read(&legacy_file).map_err(|e| format!("旧凭据文件读取失败: {e}"))?;
        // 解密旧密文（格式与公共模块一致：nonce(12B)||ciphertext）
        let plain = decrypt_legacy(&key, &data)?;
        let map: std::collections::HashMap<String, String> =
            serde_json::from_slice(&plain).map_err(|e| format!("旧凭据数据解析失败: {e}"))?;

        // 全量读入 → 全量写入（一次原子替换）→ 解密回读确认 → 才清理旧文件
        let values: std::collections::HashMap<String, serde_json::Value> = map
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::json!(v)))
            .collect();
        credentials::save_secrets(app, NAMESPACE, &values)?;
        let read_back = |key: &str| -> Result<Option<String>, String> {
            Ok(credentials::get_secret(app, NAMESPACE, key)?
                .and_then(|v| v.as_str().map(|s| s.to_string())))
        };
        verify_migrated(&map, &read_back)?;

        // 回读确认全部通过后才清理旧文件（确认失败会提前返回，旧文件原样保留）
        std::fs::remove_file(&legacy_file).ok();
        std::fs::remove_file(&legacy_key).ok();
        std::fs::remove_file(dir.join("db-secrets.bak")).ok();
        eprintln!("[database] 旧凭据已迁移到公共凭证库（{} 条）", map.len());
    }

    // 迁移 1：stronghold 阶段（db.stronghold）→ 公共库
    // 早期版本用 iota_stronghold 快照；因 debug 模式下 commit/load 需数十秒而弃用，
    // 且快照格式与当前实现不兼容，直接提示用户重新输入（测试阶段数据量小）。
    let stronghold_file = dir.join("db.stronghold");
    if stronghold_file.exists() {
        std::fs::remove_file(&stronghold_file).ok();
        std::fs::remove_file(dir.join("db-client.snapshot")).ok();
        eprintln!("[database] 已清理旧 stronghold 快照（凭据需重新输入）");
    }
    Ok(())
}

/// 迁移回读校验：旧表每一条都必须能在新库读回且值一致
///
/// 为什么要回读：写入成功不代表解密可读（主密钥、命名空间、序列化任一环节出问题都会静默丢数据），
/// 只有逐条回读一致才允许清理旧文件，否则保留旧数据并可重试。
fn verify_migrated(
    expected: &std::collections::HashMap<String, String>,
    read_back: &dyn Fn(&str) -> Result<Option<String>, String>,
) -> Result<(), String> {
    for (key, want) in expected {
        match read_back(key)? {
            Some(got) if got == *want => {}
            Some(_) => {
                return Err(format!(
                    "旧凭据迁移回读校验失败（{key} 的值不一致），旧文件已保留，请重试"
                ))
            }
            None => {
                return Err(format!(
                    "旧凭据迁移回读校验失败（{key} 读不到），旧文件已保留，请重试"
                ))
            }
        }
    }
    Ok(())
}

/// 解密旧格式密文（AES-GCM，nonce(12B)||ciphertext，与公共模块同格式）
fn decrypt_legacy(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 28 {
        return Err("旧凭据文件损坏（密文长度不足）".into());
    }
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let (nonce, ct) = data.split_at(12);
    cipher
        .decrypt(aes_gcm::Nonce::from_slice(nonce), ct)
        .map_err(|_| "旧凭据解密失败（主密钥不匹配或数据损坏）".into())
}

/// 保存连接密码（连接保存/更新时调用）
pub fn secret_save(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
    password: &str,
) -> Result<(), String> {
    secrets(app, state)?;
    credentials::save_secret(app, NAMESPACE, conn_id, &serde_json::json!(password))?;
    super::transfer::credential_saved(app, conn_id)
}

/// 读取连接密码（连接/测试连接时调用；无记录返回空串）
pub fn secret_get(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
) -> Result<String, String> {
    secrets(app, state)?;
    if super::transfer::credential_pending(app, conn_id)? {
        return Err("导入的连接尚未配置凭证，请编辑连接并重新保存密码".into());
    }
    Ok(credentials::get_secret(app, NAMESPACE, conn_id)?
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default())
}

/// 删除连接密码（删除连接时调用）
pub fn secret_delete(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
) -> Result<(), String> {
    secrets(app, state)?;
    credentials::delete_secret(app, NAMESPACE, conn_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 全部一致时通过
    #[test]
    fn verify_migrated_accepts_matching_values() {
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), "1".to_string());
        expected.insert("b".to_string(), "2".to_string());
        let read = |key: &str| -> Result<Option<String>, String> { Ok(expected.get(key).cloned()) };
        assert!(verify_migrated(&expected, &read).is_ok());
    }

    /// 值不一致（写入串了）必须报失败，不能当迁移完成
    #[test]
    fn verify_migrated_rejects_mismatched_value() {
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), "1".to_string());
        let read = |_: &str| -> Result<Option<String>, String> { Ok(Some("9".to_string())) };
        let error = verify_migrated(&expected, &read).unwrap_err();
        assert!(error.contains("值不一致"), "{error}");
    }

    /// 读不到（静默丢条目）同样必须报失败
    #[test]
    fn verify_migrated_rejects_missing_entry() {
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), "1".to_string());
        let read = |_: &str| -> Result<Option<String>, String> { Ok(None) };
        let error = verify_migrated(&expected, &read).unwrap_err();
        assert!(error.contains("读不到"), "{error}");
    }
}
