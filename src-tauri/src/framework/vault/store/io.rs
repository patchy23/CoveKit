//! Vault 读改写：`vault.dat` 的加密读写（明文 = `Credential` 数组；目录参数便于单测）
//!
//! 读写两条路径都先按现有密文解析主密钥，保证写入不会在半途另生成新密钥、也不会覆盖既有密文的语义。
//! 密文与**空间 uid 数学绑定**（AES-GCM AAD = uid）：换空间即使拿到同一把主密钥也解不开；

use std::path::Path;

use super::paths::VAULT_FILE;
use crate::framework::secure_store::{
    ciphertext_evidence, encrypt_with_aad, load_verified, replace_file, resolve_master_key,
    MasterKeyStore, VAULT_KEY_SPEC,
};
use crate::framework::vault::models::Credential;

/// 读取全部凭证（解密；主文件与备份都不存在时返回空表；密文与空间 uid 绑定）
pub(crate) fn read_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
    space_id: &str,
) -> Result<Vec<Credential>, String> {
    let path = dir.join(VAULT_FILE);
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let aad = space_id.as_bytes();
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence, aad)?.key;
    let decode = |plain: &[u8]| -> Result<Vec<Credential>, String> {
        serde_json::from_slice(plain).map_err(|e| format!("凭证数据解析失败: {e}"))
    };
    // 读路径内完成择版：主文件可解就用主文件，只有备份可用则备份转正
    Ok(load_verified(&path, &key, aad, &decode)?.unwrap_or_default())
}

/// 写回全部凭证（加密落盘，原子替换 + 备份恢复；AAD = 空间 uid）
pub(crate) fn write_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
    credentials: &[Credential],
    space_id: &str,
) -> Result<(), String> {
    let path = dir.join(VAULT_FILE);
    // 先按现有密文解析主密钥：既不能在半途另生成新密钥，也不能覆盖既有密文的语义
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let aad = space_id.as_bytes();
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence, aad)?.key;
    let plain = serde_json::to_vec(credentials).map_err(|e| e.to_string())?;
    let out = encrypt_with_aad(&key, &plain, aad)?;
    replace_file(&path, &out)
}
