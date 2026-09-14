//! Vault 与 credentials 的保护状态判定（T04-5）：主密钥实际来源与可用性，供设置页展示
//!
//! 两个域共用一套判定：vault 走 vault 分区，credentials 走数据分区，避免把 vault 目录当成凭据目录去扫。

use std::path::Path;

use tauri::AppHandle;

use super::paths::{data_dir_of, VAULT_FILE};
use crate::framework::credentials;
use crate::framework::secure_store::{
    inspect_domain, native_backend_available, KeyringStore, MasterKeyStore, ProtectionStatus,
    CREDENTIALS_KEY_SPEC, VAULT_KEY_SPEC,
};

/// 凭证保护状态（目录参数版本，供单测；无副作用：不生成密钥、不写密钥库、不落降级文件）
pub(crate) fn protection_status_at(
    vault_dir: &Path,
    data_dir: &Path,
    store: &dyn MasterKeyStore,
) -> Result<ProtectionStatus, String> {
    let vault_files = [vault_dir.join(VAULT_FILE)];
    let credential_files = credentials::all_namespace_files(data_dir)?;
    Ok(ProtectionStatus {
        native_backend: native_backend_available(),
        domains: vec![
            inspect_domain("vault", vault_dir, &VAULT_KEY_SPEC, store, &vault_files),
            inspect_domain(
                "credentials",
                data_dir,
                &CREDENTIALS_KEY_SPEC,
                store,
                &credential_files,
            ),
        ],
    })
}

/// 凭证保护状态（AppHandle 封装；命令层入口）
///
/// 两个域各自解析目录（都带旧布局回落）：vault 走 vault 分区，
/// credentials 走数据分区，避免把 vault 目录当成凭据目录去扫。
pub(crate) fn protection_status(app: &AppHandle) -> Result<ProtectionStatus, String> {
    let vault_dir = data_dir_of(app)?;
    let credential_dir = credentials::resolved_data_dir(app)?;
    protection_status_at(&vault_dir, &credential_dir, &KeyringStore)
}
