//! Vault 读改写：`vault.dat` 的加密读写（明文 = `Credential` 数组；目录参数便于单测）
//!
//! 读写两条路径都先按现有密文解析主密钥，保证写入不会在半途另生成新密钥、也不会覆盖既有密文的语义。

use std::path::Path;

use super::paths::VAULT_FILE;
use crate::framework::secure_store::{
    ciphertext_evidence, encrypt_payload, load_verified, replace_file, resolve_master_key,
    MasterKeyStore, VAULT_KEY_SPEC,
};
use crate::framework::vault::models::Credential;

/// 读取全部凭证（解密；主文件与备份都不存在时返回空表）
pub(crate) fn read_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
) -> Result<Vec<Credential>, String> {
    let path = dir.join(VAULT_FILE);
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence)?.key;
    let decode = |plain: &[u8]| -> Result<Vec<Credential>, String> {
        serde_json::from_slice(plain).map_err(|e| format!("凭证数据解析失败: {e}"))
    };
    // 读路径内完成择版：主文件可解就用主文件，只有备份可用则备份转正（见 secure_store::load_verified）
    Ok(load_verified(&path, &key, &decode)?.unwrap_or_default())
}

/// 写回全部凭证（加密落盘，原子替换 + 备份恢复）
pub(crate) fn write_all_at(
    dir: &Path,
    store: &dyn MasterKeyStore,
    credentials: &[Credential],
) -> Result<(), String> {
    let path = dir.join(VAULT_FILE);
    // 先按现有密文解析主密钥：既不能在半途另生成新密钥，也不能覆盖既有密文的语义
    let evidence = ciphertext_evidence(std::slice::from_ref(&path))?;
    let key = resolve_master_key(dir, &VAULT_KEY_SPEC, store, &evidence)?.key;
    let plain = serde_json::to_vec(credentials).map_err(|e| e.to_string())?;
    let out = encrypt_payload(&key, &plain)?;
    replace_file(&path, &out)
}
