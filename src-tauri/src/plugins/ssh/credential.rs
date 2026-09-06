//! SSH 插件 · 旧版手工凭证存储（AES-256-GCM 本地加密）
//! 历史背景：v1 手工凭证以 profileId 为键加密存 ssh-credentials.json（主密钥 ssh-master.key）。
//! 现凭证统一走公共 Vault（profile 只存 credentialRef），本模块仅保留：
//! 1) 读取旧文件用于一次性迁移进 Vault（store::ssh_profile_import）；
//! 2) 迁移成功后归档旧文件（改名保留，不删除）。
//!
//! 若旧文件不存在，全部函数为无害空操作。

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use tauri::{AppHandle, Manager};

/// 主密钥文件（不存在则生成 32 随机字节）
fn master_key(app: &AppHandle) -> Result<[u8; 32], String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let path = dir.join("ssh-master.key");
    recover_backup(&path)?;
    if path.exists() {
        let bytes = std::fs::read(&path).map_err(|e| format!("主密钥读取失败: {e}"))?;
        if bytes.len() != 32 {
            return Err("主密钥文件损坏（长度不是 32 字节），为避免凭证丢失已停止操作".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    // 生成新主密钥（一次性，之后常驻）
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    replace_file(&path, &key).map_err(|e| format!("主密钥写入失败: {e}"))?;
    Ok(key)
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

/// 先完整写入临时文件并刷盘，再替换目标，避免进程中断留下半截密文。
fn replace_file(path: &Path, content: &[u8]) -> Result<(), String> {
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
        eprintln!("[ssh] 凭证已保存，但备份清理失败: {error}");
    }
    Ok(())
}

/// 恢复进程中断遗留的备份；主文件存在时仅清理已过期备份。
fn recover_backup(path: &Path) -> Result<(), String> {
    let backup = path.with_extension("bak");
    match (path.exists(), backup.exists()) {
        (false, true) => {
            std::fs::rename(&backup, path).map_err(|e| format!("凭证备份恢复失败: {e}"))
        }
        (true, true) => std::fs::remove_file(&backup).map_err(|e| format!("凭证备份清理失败: {e}")),
        _ => Ok(()),
    }
}

/// 凭证文件路径
fn creds_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))?;
    Ok(dir.join("ssh-credentials.json"))
}

/// 读取全部旧凭证（解密；无文件时返回空表；供一次性迁移使用，明文不离开本模块的调用方）
pub(crate) fn read_all(app: &AppHandle) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = creds_path(app)?;
    recover_backup(&path)?;
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(format!("凭证文件读取失败: {error}")),
    };
    let key = master_key(app)?;
    let plain = decrypt_payload(&key, &data)?;
    serde_json::from_slice(&plain).map_err(|e| format!("凭证数据解析失败: {e}"))
}

/// 归档旧凭证文件（改名 .migrated.bak 保留）；文件不存在时为无害空操作。
pub(crate) fn archive_legacy_file(app: &AppHandle) -> Result<(), String> {
    let path = creds_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let backup = path.with_extension("migrated.bak");
    std::fs::rename(&path, backup).map_err(|e| format!("旧凭证文件归档失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupted_ciphertext_returns_error() {
        assert!(decrypt_payload(&[7u8; 32], b"short").is_err());
    }
}
