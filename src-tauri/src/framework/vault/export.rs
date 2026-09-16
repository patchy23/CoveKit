//! 导出 / 导入备份（.pbvault）：用户设一次性密码 → Argon2id 派生密钥 → AES-256-GCM 加密全量 vault
//! 用途：换机迁移 + 灾难恢复。备份文件为 JSON 文本（字段 hex 编码），含 KDF 参数自描述，
//! 导入端按文件内参数重新派生，未来调参不影响旧备份解开。

use std::path::Path;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Params;
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// 备份格式当前版本
const BACKUP_VERSION: u32 = 1;
/// Argon2id salt 长度（16B，RFC 9106 推荐下限）
const SALT_LEN: usize = 16;

/// KDF 参数（自描述写入备份文件，导入时按此重新派生）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    /// 算法标识（当前恒 "argon2id"）
    pub algo: String,
    /// 盐（hex，16B）
    pub salt: String,
    /// 内存成本（KiB）
    pub m_cost: u32,
    /// 迭代次数
    pub t_cost: u32,
    /// 并行度
    pub p_cost: u32,
}

/// .pbvault 备份文件结构（JSON 文本，二进制字段一律 hex 编码）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultBackupFile {
    /// 格式版本（当前 1）
    pub version: u32,
    /// KDF 参数
    pub kdf: KdfParams,
    /// AES-GCM nonce（hex，12B）
    pub nonce: String,
    /// 密文（hex，明文 = vault JSON 数组）
    pub ciphertext: String,
}

/// 用一次性密码 + 参数派生 32B 加密密钥（Argon2id；派生实现走框架唯一原语）
fn derive_key(password: &str, params: &KdfParams) -> Result<[u8; 32], String> {
    if params.algo != "argon2id" {
        return Err(format!("不支持的 KDF 算法：{}", params.algo));
    }
    let salt = hex::decode(&params.salt).map_err(|e| format!("备份盐解码失败: {e}"))?;
    crate::framework::secure_store::derive_key_argon2id(
        password,
        &salt,
        params.m_cost,
        params.t_cost,
        params.p_cost,
    )
}

/// 导出加密：明文 → 备份文件结构（随机 salt + 随机 nonce，默认 Argon2id 参数）
pub(crate) fn encrypt_backup(password: &str, plain: &[u8]) -> Result<VaultBackupFile, String> {
    if password.is_empty() {
        return Err("导出密码不能为空".into());
    }
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let kdf = KdfParams {
        algo: "argon2id".into(),
        salt: hex::encode(salt),
        m_cost: Params::DEFAULT_M_COST,
        t_cost: Params::DEFAULT_T_COST,
        p_cost: Params::DEFAULT_P_COST,
    };
    let key = derive_key(password, &kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|_| "备份加密失败")?;
    Ok(VaultBackupFile {
        version: BACKUP_VERSION,
        kdf,
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ct),
    })
}

/// 导入解密：备份文件结构 + 一次性密码 → 明文（密码错误 / 文件损坏均返回 Err 不 panic）
pub(crate) fn decrypt_backup(password: &str, backup: &VaultBackupFile) -> Result<Vec<u8>, String> {
    if backup.version != BACKUP_VERSION {
        return Err(format!(
            "不支持的备份版本 {}（当前支持 {BACKUP_VERSION}）",
            backup.version
        ));
    }
    let nonce = hex::decode(&backup.nonce).map_err(|e| format!("备份 nonce 解码失败: {e}"))?;
    if nonce.len() != 12 {
        return Err("备份文件损坏（nonce 长度不是 12 字节）".into());
    }
    let ct = hex::decode(&backup.ciphertext).map_err(|e| format!("备份密文解码失败: {e}"))?;
    if ct.len() < 16 {
        return Err("备份文件损坏（密文长度不足）".into());
    }
    let key = derive_key(password, &backup.kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), ct.as_slice())
        .map_err(|_| "备份解密失败（密码错误或文件损坏）".into())
}

/// 读取并解析 .pbvault 文件（JSON）
pub(crate) fn read_backup_file(path: &Path) -> Result<VaultBackupFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("备份文件读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("备份文件解析失败（不是有效的 .pbvault）: {e}"))
}

/// 将备份结构写入 .pbvault 文件
pub(crate) fn write_backup_file(path: &Path, backup: &VaultBackupFile) -> Result<(), String> {
    let text = serde_json::to_string_pretty(backup).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("备份文件写入失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 导出/导入 roundtrip：同密码解开还原明文；错误密码报 Err
    #[test]
    fn export_import_roundtrip() {
        let plain = r#"[{"id":"c1","name":"生产 MySQL"}]"#.as_bytes();
        let backup = encrypt_backup("一次性密码", plain).unwrap();
        assert_eq!(decrypt_backup("一次性密码", &backup).unwrap(), plain);
        let err = decrypt_backup("错误密码", &backup).unwrap_err();
        assert!(err.contains("密码错误"));
    }

    /// 同明文两次导出 salt/nonce 不同（密文不同）
    #[test]
    fn export_randomized() {
        let plain = b"[]";
        let a = encrypt_backup("pw", plain).unwrap();
        let b = encrypt_backup("pw", plain).unwrap();
        assert_ne!(a.kdf.salt, b.kdf.salt);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    /// 损坏备份报错不 panic：版本不符 / nonce 长度错 / 密文过短 / 篡改密文 / 非法 JSON
    #[test]
    fn corrupt_backup_errors_no_panic() {
        let plain = b"[]";
        let backup = encrypt_backup("pw", plain).unwrap();

        let mut bad_version = backup.clone();
        bad_version.version = 99;
        assert!(decrypt_backup("pw", &bad_version).is_err());

        let mut bad_nonce = backup.clone();
        bad_nonce.nonce = "ab".into();
        assert!(decrypt_backup("pw", &bad_nonce).is_err());

        let mut bad_ct = backup.clone();
        bad_ct.ciphertext = "0011".into();
        assert!(decrypt_backup("pw", &bad_ct).is_err());

        let mut tampered = backup.clone();
        let mut raw = hex::decode(&tampered.ciphertext).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        tampered.ciphertext = hex::encode(raw);
        assert!(decrypt_backup("pw", &tampered).is_err());

        // 文件级：非法 JSON 报解析错误
        let dir = std::env::temp_dir().join(format!("vault-backup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.pbvault");
        std::fs::write(&path, "not json").unwrap();
        assert!(read_backup_file(&path).is_err());

        // 文件级 roundtrip
        let good = dir.join("good.pbvault");
        write_backup_file(&good, &backup).unwrap();
        let parsed = read_backup_file(&good).unwrap();
        assert_eq!(decrypt_backup("pw", &parsed).unwrap(), plain);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空密码导出被拒
    #[test]
    fn empty_password_rejected() {
        assert!(encrypt_backup("", b"[]").is_err());
    }
}
