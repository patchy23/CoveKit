//! Vault 存储布局与互斥：分区解析（含旧布局回落）、进程内读改写锁、损坏文件留档

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

/// 凭证密文文件名（vault 分区下）
pub(crate) const VAULT_FILE: &str = "vault.dat";

/// vault 全量读改写进程内互斥锁（防并发丢更新）
pub(crate) fn vault_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Vault 目录解析（含旧布局回落；纯函数便于单测）
///
/// 新布局：`<空间根>/vault/vault.dat`（主密钥同目录）；旧布局：`<空间根>/vault.dat`。
/// 只有「vault 分区不存在、空间根下仍有旧 vault.dat」时才回落：布局迁移可能整组保留原位，
/// 此时必须按旧位置读写，否则会表现为凭证库为空、甚至用新密钥覆盖旧密文。
///
/// 两个参数都取自**同一个**存储位置描述符（分区 vault 目录 + 空间根），因此非默认空间
/// 天然落在自己的 `spaces/<空间 id>/generations/<代际>` 之下，不会读到别的空间的凭证库。
pub(crate) fn resolve_vault_dir(vault: &Path, space_root: &Path) -> PathBuf {
    if !vault.exists() && space_root.join(VAULT_FILE).exists() {
        return space_root.to_path_buf();
    }
    vault.to_path_buf()
}

/// 凭证分区目录（描述符里的 vault 分区；命令层用，测试走 *_at 目录参数版本）
pub(crate) fn data_dir_of(app: &AppHandle) -> Result<PathBuf, String> {
    let location = crate::framework::paths::current_location(app)?;
    Ok(resolve_vault_dir(&location.vault, &location.root))
}

/// 将无法读取的 vault.dat 改名留档（密钥丢失/文件损坏时导入的前置保护，绝不静默清空）
pub(crate) fn orphan_vault_file(dir: &Path) -> Result<(), String> {
    let path = dir.join(VAULT_FILE);
    if !path.exists() {
        return Ok(());
    }
    let orphan = dir.join(format!(
        "vault.dat.unreadable-{}.bak",
        chrono::Utc::now().timestamp()
    ));
    std::fs::rename(&path, &orphan)
        .map_err(|e| format!("旧凭证库留档失败（未做任何清除，原文件仍在）: {e}"))
}
