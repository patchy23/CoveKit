//! 数据库连接凭据 · 框架公共凭证模块薄封装
//! 存储实现（AES-GCM 加密文件、主密钥、原子写）全部在 framework::credentials，
//! 本模块负责公共 Vault 引用解析、旧命名空间兼容和可恢复迁移。
//! 迁移链：db.stronghold（早期）→ db-secrets.enc/db-master.key（AES-GCM 阶段）
//! → credentials/database.enc（旧公共命名空间）→ 公共 Vault 引用。

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
                "旧主密钥文件损坏（长度不是 32 字节），旧文件已保留；请恢复正确密钥或重新填写凭据"
                    .into(),
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
        eprintln!("[database] 检测到旧 stronghold 快照，已保留原文件；请重新输入所需连接凭据");
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
    credentials::save_secret(app, NAMESPACE, conn_id, &serde_json::json!(password))
}

/// 保存事务的旧值快照；只读取密文，不将待补录连接视为可连接。
pub(super) fn secret_snapshot(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
) -> Result<Option<String>, String> {
    secrets(app, state)?;
    credentials::get_secret(app, NAMESPACE, conn_id)?
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "连接凭据格式损坏".into())
        })
        .transpose()
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
    use tauri::Manager;
    let store = app.state::<super::store::StoreState>();
    if let Some(mut config) = super::store::list_connections(app, &store)?
        .into_iter()
        .find(|config| config.id == conn_id)
    {
        let username = config.username.clone();
        if let Some(password) = referenced_password(app, &mut config)? {
            if config.username != username {
                return Err("共享凭证的用户名已改变，请编辑并重新保存数据库连接".into());
            }
            return Ok(password);
        }
    }
    Ok(secret_snapshot(app, state, conn_id)?.unwrap_or_default())
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

/// 串行化配置与凭证间的同步提交；锁不跨 await。
pub(super) static CONFIG_CREDENTIAL_LOCK: Mutex<()> = Mutex::new(());

/// 解析公共凭证，不把秘密返回前端。共享凭证用户名变化时要求重新保存连接。
pub(super) fn referenced_password(
    app: &tauri::AppHandle,
    config: &mut super::models::ConnConfig,
) -> Result<Option<String>, String> {
    use crate::framework::vault::{self, CredentialFields};
    let Some(id) = config.credential_id.as_deref().filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let credential = vault::resolve(app, id)?;
    match credential.fields {
        CredentialFields::Password { username, password } => {
            config.username = username;
            Ok(Some(password))
        }
        CredentialFields::ApiToken { token } if config.db_type == super::models::DbType::Redis => {
            config.username.clear();
            Ok(Some(token))
        }
        _ => Err("数据库需要用户名密码凭证；Redis 也可使用单 token 密码凭证".into()),
    }
}

/// 幂等迁移：公共库写入、回读相等、保存引用后才允许清理旧密文。
/// 写凭证后配置保存失败时，重试按来源标记和完整字段匹配复用，不覆盖共享凭证。
pub(super) fn import_password(
    app: &tauri::AppHandle,
    config: &super::models::ConnConfig,
    password: &str,
) -> Result<String, String> {
    use crate::framework::vault::{self, CredentialFields, CredentialSavePayload};
    let fields = if config.username.is_empty() && config.db_type == super::models::DbType::Redis {
        CredentialFields::ApiToken {
            token: password.into(),
        }
    } else {
        CredentialFields::Password {
            username: config.username.clone(),
            password: password.into(),
        }
    };
    let note = format!("CoveKit 数据库连接迁移：{}", config.id);
    for summary in vault::vault_list(app.clone())? {
        if summary.note == note && vault::resolve(app, &summary.id)?.fields == fields {
            return Ok(summary.id);
        }
    }
    let summary = vault::vault_save(
        app.clone(),
        CredentialSavePayload {
            id: None,
            name: format!("数据库 · {}", config.label),
            kind: fields.kind(),
            fields: fields.clone(),
            note,
        },
    )?;
    if vault::resolve(app, &summary.id)?.fields != fields {
        return Err("数据库凭证迁移回读不一致，旧凭证已保留".into());
    }
    Ok(summary.id)
}

/// 迁移旧凭据为公共 Vault 引用；读回验证后保存引用并保留旧副本。
pub(super) fn migrate_references(
    app: &tauri::AppHandle,
    state: &SecretsState,
    store: &super::store::StoreState,
) -> Result<Vec<super::models::ConnConfig>, String> {
    let _lock = CONFIG_CREDENTIAL_LOCK.lock().map_err(|e| e.to_string())?;
    let mut configs = super::store::list_connections(app, store)?;
    for config in &mut configs {
        if config.credential_id.is_some()
            || config.db_type.is_sqlite()
            || super::transfer::credential_pending(app, &config.id)?
        {
            continue;
        }
        if let Some(password) =
            secret_snapshot(app, state, &config.id)?.filter(|password| !password.is_empty())
        {
            config.credential_id = Some(import_password(app, config, &password)?);
            super::store::save_connection(app, store, config)?;
            // 旧命名空间副本保留到显式修改/删除连接；失败不会丢失唯一恢复数据。
        }
    }
    Ok(configs)
}
