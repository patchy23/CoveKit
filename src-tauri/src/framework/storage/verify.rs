//! 框架 · 存储迁移的校验（复制完成后、提交新根之前执行）
//!
//! 校验项（任务书 T02）：
//! 1. **逐文件清单比对**：相对路径、字节数、SHA-256 摘要逐项一致；缺失 / 多余 / 同长度不同内容都算失败。
//! 2. 迁移过来的每个 `.db` 跑 `PRAGMA quick_check`（SQLite 完整性检查，能发现截断/半写）。
//! 3. `vault/vault.dat` 长度语义检查（AES-GCM 密文至少 12B nonce + 16B tag = 28B）；
//!    **不生成任何新主密钥**，只看文件本身。
//!
//! 说明（T02-5/6）：迁移在启动维护阶段执行，此时所有插件数据库尚未打开，
//! 属于 SQLite 的「完整离线处理」：`.db` 与其 `-wal` / `-shm` 伴随文件一并复制并进入清单，
//! 因此不需要在线备份 API（[SQLite Backup API](https://www.sqlite.org/backup.html) 针对运行中快照）。
//!
//! 任一项不过即返回错误，调用方中止提交并且**不写配置**（原数据与现状不受影响）。

use std::path::Path;

use crate::framework::storage::scan::{self, Manifest};
use crate::framework::storage::PARTITIONS;

/// 校验暂存区内容与源清单是否一致，并做数据库与 Vault 语义检查
///
/// `staged_root` 为暂存目录（结构与存储根相同）；`expected` 为复制前对源目录生成的清单。
pub fn verify_staging(staged_root: &Path, expected: &Manifest) -> Result<(), String> {
    let actual = scan::scan_root(staged_root)?;
    scan::diff(expected, &actual)?;
    check_sqlite_dbs(&staged_root.join("data"))?;
    check_vault_file(&staged_root.join("vault"))?;
    Ok(())
}

/// 校验目标根在重试场景下是否已经是源的完整副本（避免把半截目标当成功）
pub fn verify_existing_target(target_root: &Path, expected: &Manifest) -> Result<(), String> {
    let actual = scan::scan_root(target_root)?;
    scan::diff(expected, &actual)?;
    check_sqlite_dbs(&target_root.join("data"))?;
    check_vault_file(&target_root.join("vault"))?;
    Ok(())
}

/// 逐个校验目标数据分区下的 SQLite 库（`PRAGMA quick_check`）
fn check_sqlite_dbs(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| format!("读取 {} 失败: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("读取 {} 的目录项失败: {e}", dir.display()))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
        if meta.is_dir() {
            check_sqlite_dbs(&path)?;
            continue;
        }
        if meta.file_type().is_symlink() {
            return Err(format!("校验失败：暂存区出现符号链接 {}", path.display()));
        }
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let conn = rusqlite::Connection::open(&path)
            .map_err(|e| format!("校验失败：无法打开 {}（{e}）", path.display()))?;
        let result: Result<String, _> = conn.query_row("PRAGMA quick_check", [], |row| row.get(0));
        match result {
            Ok(v) if v == "ok" => {}
            Ok(v) => {
                return Err(format!(
                    "校验失败：{} 完整性检查未通过（{v}）",
                    path.display()
                ))
            }
            Err(e) => {
                return Err(format!(
                    "校验失败：{} 完整性检查无法执行（{e}）",
                    path.display()
                ))
            }
        }
    }
    Ok(())
}

/// 校验目标凭证分区中的 vault.dat（长度语义：至少 28 字节）
fn check_vault_file(dir: &Path) -> Result<(), String> {
    let path = dir.join("vault.dat");
    if !path.exists() {
        return Ok(());
    }
    let len = std::fs::metadata(&path)
        .map_err(|e| format!("校验失败：读取 {} 失败（{e}）", path.display()))?
        .len();
    if len < 28 {
        return Err(format!(
            "校验失败：{} 长度异常（{len} 字节，密文至少 28 字节）",
            path.display()
        ));
    }
    Ok(())
}

/// 目标根下是否只存在本次任务自己的暂存目录（用于判断目标是否「空」）
pub fn only_own_staging(root: &Path, staging_name: &str) -> Result<bool, String> {
    if !root.exists() {
        return Ok(true);
    }
    for entry in
        std::fs::read_dir(root).map_err(|e| format!("读取 {} 失败: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("读取 {} 的目录项失败: {e}", root.display()))?;
        if entry.file_name().to_string_lossy() == staging_name {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

/// 列出四分区中在目标根下已存在的分区名（提交前用于冲突提示）
pub fn existing_partitions(root: &Path) -> Vec<String> {
    PARTITIONS
        .iter()
        .filter(|name| root.join(name).exists())
        .map(|name| (*name).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::storage::test_support::temp_dir;
    use crate::framework::storage::transfer;

    /// 造一份含真实 SQLite 与 vault 密文的源目录
    fn make_source(dir: &Path) {
        std::fs::create_dir_all(dir.join("data")).unwrap();
        let conn = rusqlite::Connection::open(dir.join("data").join("ssh.db")).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER);").unwrap();
        drop(conn);
        std::fs::create_dir_all(dir.join("vault")).unwrap();
        std::fs::write(dir.join("vault").join("vault.dat"), vec![0u8; 32]).unwrap();
    }

    #[test]
    fn verify_passes_for_identical_copy() {
        let dir = temp_dir("verify-ok");
        let src = dir.join("src");
        let dst = dir.join("staging");
        make_source(&src);
        let manifest = scan::scan_root(&src).unwrap();
        transfer::copy_tree(&src.join("data"), &dst.join("data")).unwrap();
        transfer::copy_tree(&src.join("vault"), &dst.join("vault")).unwrap();
        assert!(verify_staging(&dst, &manifest).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_missing_file() {
        let dir = temp_dir("verify-missing");
        let src = dir.join("src");
        let dst = dir.join("staging");
        make_source(&src);
        let manifest = scan::scan_root(&src).unwrap();
        std::fs::create_dir_all(dst.join("data")).unwrap();
        std::fs::write(dst.join("data").join("ssh.db"), b"x").unwrap();
        let err = verify_staging(&dst, &manifest).unwrap_err();
        assert!(err.contains("缺失"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_same_length_different_content() {
        let dir = temp_dir("verify-samelen");
        let src = dir.join("src");
        let dst = dir.join("staging");
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::create_dir_all(dst.join("logs")).unwrap();
        std::fs::write(src.join("logs").join("app.log"), b"aaaa").unwrap();
        // 同长度不同内容：字节数一致，摘要不同 → 必须失败（旧实现会放过）
        std::fs::write(dst.join("logs").join("app.log"), b"bbbb").unwrap();
        let manifest = scan::scan_root(&src).unwrap();
        let err = verify_staging(&dst, &manifest).unwrap_err();
        assert!(err.contains("内容不一致"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_extra_file() {
        let dir = temp_dir("verify-extra");
        let src = dir.join("src");
        let dst = dir.join("staging");
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::create_dir_all(dst.join("logs")).unwrap();
        std::fs::write(src.join("logs").join("app.log"), b"aaaa").unwrap();
        std::fs::write(dst.join("logs").join("app.log"), b"aaaa").unwrap();
        std::fs::write(dst.join("logs").join("stray.log"), b"zz").unwrap();
        let manifest = scan::scan_root(&src).unwrap();
        let err = verify_staging(&dst, &manifest).unwrap_err();
        assert!(err.contains("多余"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_truncated_vault() {
        let dir = temp_dir("verify-vault");
        let src = dir.join("src");
        let dst = dir.join("staging");
        std::fs::create_dir_all(src.join("vault")).unwrap();
        std::fs::create_dir_all(dst.join("vault")).unwrap();
        std::fs::write(src.join("vault").join("vault.dat"), vec![0u8; 8]).unwrap();
        std::fs::write(dst.join("vault").join("vault.dat"), vec![0u8; 8]).unwrap();
        let manifest = scan::scan_root(&src).unwrap();
        let err = verify_staging(&dst, &manifest).unwrap_err();
        assert!(err.contains("长度异常"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_corrupted_db() {
        let dir = temp_dir("verify-db");
        let src = dir.join("src");
        let dst = dir.join("staging");
        std::fs::create_dir_all(src.join("data")).unwrap();
        std::fs::create_dir_all(dst.join("data")).unwrap();
        // 两侧同样的垃圾内容：清单一致，但 quick_check 应该失败
        let garbage = b"this is not a sqlite database at all";
        std::fs::write(src.join("data").join("bad.db"), garbage).unwrap();
        std::fs::write(dst.join("data").join("bad.db"), garbage).unwrap();
        let manifest = scan::scan_root(&src).unwrap();
        let err = verify_staging(&dst, &manifest).unwrap_err();
        assert!(err.contains("校验失败"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_own_staging_detects_foreign_entries() {
        let root = temp_dir("verify-own");
        std::fs::write(root.join("other.txt"), b"x").unwrap();
        assert!(!only_own_staging(&root, ".covekit-staging-p1").unwrap());
        std::fs::remove_file(root.join("other.txt")).unwrap();
        std::fs::create_dir_all(root.join(".covekit-staging-p1")).unwrap();
        assert!(only_own_staging(&root, ".covekit-staging-p1").unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }
}
