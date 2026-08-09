//! SSH 插件 · 凭证管理（AES-256-GCM 本地加密）
//! v1 实现：主密钥（随机 32B）存 app_data_dir/ssh-master.key，凭证 JSON 加密存 ssh-credentials.json；
//! 每次读写全量解密→增删改→加密写回。stronghold 引擎集成列为 P2
//! （iota_stronghold 2.x 为 procedures 架构，Rust 侧胶水成本高；插件仅暴露前端命令层）。

use std::{collections::HashMap, path::PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use tauri::{AppHandle, Manager};

use crate::plugins::ssh::models::SshActionResult;

/// 主密钥文件（不存在则生成 32 随机字节）
fn master_key(app: &AppHandle) -> Result<[u8; 32], String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let path = dir.join("ssh-master.key");
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }
    // 生成新主密钥（一次性，之后常驻）
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    std::fs::write(&path, key).map_err(|e| format!("主密钥写入失败: {e}"))?;
    Ok(key)
}

/// 凭证文件路径
fn creds_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))?;
    Ok(dir.join("ssh-credentials.json"))
}

/// 读取全部凭证（解密；无文件时返回空表）
fn read_all(app: &AppHandle) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = creds_path(app)?;
    let Ok(data) = std::fs::read(&path) else {
        return Ok(HashMap::new());
    };
    let key = master_key(app)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    // 载荷格式：nonce(12B) || ciphertext
    let (nonce, ct) = data.split_at(12);
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| "凭证解密失败（主密钥不匹配或数据损坏）")?;
    serde_json::from_slice(&plain).map_err(|e| format!("凭证数据解析失败: {e}"))
}

/// 写回全部凭证（加密落盘）
fn write_all(app: &AppHandle, map: &HashMap<String, serde_json::Value>) -> Result<(), String> {
    let key = master_key(app)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let plain = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain.as_ref())
        .map_err(|_| "凭证加密失败")?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    std::fs::write(creds_path(app)?, out).map_err(|e| format!("凭证写入失败: {e}"))
}

/// 保存凭证（profile 级；字段与契约 Payloads.ssh_credential_save 对应）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_credential_save(
    app: AppHandle,
    payload: crate::plugins::ssh::models::SshCredentialSavePayload,
) -> Result<SshActionResult, String> {
    let mut map = read_all(&app)?;
    map.insert(
        payload.profile.id,
        serde_json::json!({
            "authMethod": payload.profile.auth_method,
            "password": payload.password,
            "privateKey": payload.private_key,
            "passphrase": payload.passphrase,
        }),
    );
    write_all(&app, &map)?;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

/// 读取凭证（解密返回；无记录返回空对象）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_credential_get(
    app: AppHandle,
    profile_id: String,
) -> Result<serde_json::Value, String> {
    let map = read_all(&app)?;
    Ok(map
        .get(&profile_id)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({})))
}

/// 删除凭证
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_credential_delete(
    app: AppHandle,
    profile_id: String,
) -> Result<SshActionResult, String> {
    let mut map = read_all(&app)?;
    map.remove(&profile_id);
    write_all(&app, &map)?;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 固定密钥加密（返回 nonce||ciphertext）
    fn encrypt_with(key: [u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), plain)
            .map_err(|_| "加密失败")?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// 固定密钥解密
    fn decrypt_with(key: [u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let (n, c) = data.split_at(12);
        cipher
            .decrypt(Nonce::from_slice(n), c)
            .map_err(|_| "解密失败".into())
    }

    #[test]
    fn 加解密往返一致() {
        let key = [7u8; 32];
        let plain = b"{\"password\":\"s3cret\"}";
        let ct = encrypt_with(key, plain).unwrap();
        let back = decrypt_with(key, &ct).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn 密钥错误解密失败() {
        let key = [7u8; 32];
        let plain = b"hello";
        let ct = encrypt_with(key, plain).unwrap();
        let wrong = [8u8; 32];
        assert!(decrypt_with(wrong, &ct).is_err());
    }
}
