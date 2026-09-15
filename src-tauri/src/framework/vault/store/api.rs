//! Vault 读取入口（AppHandle 级）：给门面命令与插件建连使用的两个入口
//!
//! - `read_all`：锁内读全量，供列表、导出等只读消费方
//! - `resolve`：按 credentialId 解析单条明文，插件在 Rust 侧建连时使用（明文不过 IPC、不到前端）

use tauri::AppHandle;

use super::io::read_all_at;
use super::paths::{data_dir_of, vault_lock};
use crate::framework::vault::models::Credential;

/// 锁内读全量（AppHandle 封装）
pub(crate) fn read_all(app: &AppHandle) -> Result<Vec<Credential>, String> {
    let _guard = vault_lock().lock().map_err(|e| e.to_string())?;
    read_all_at(
        &data_dir_of(app)?,
        &crate::framework::space::keyring_store(),
    )
}

/// 插件命令在 Rust 侧解析 credentialId → 凭证明文（crate 内 API，不做成 Tauri 命令；
/// 明文不过 IPC、不到前端，插件解析后直接用于建连）
pub fn resolve(app: &AppHandle, credential_id: &str) -> Result<Credential, String> {
    let all = read_all(app)?;
    all.into_iter()
        .find(|c| c.id == credential_id)
        .ok_or_else(|| format!("凭证不存在或已删除（id: {credential_id}）"))
}
