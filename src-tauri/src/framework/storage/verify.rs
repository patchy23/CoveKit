//! 框架 · 存储迁移的校验（复制完成后、写配置之前执行）
//!
//! 校验项（任务书 §3.4 第 3 步）：
//! 1. 四分区**文件数与总字节数**逐分区一致。
//! 2. 目标侧每个 `.db` 跑 `PRAGMA quick_check`（SQLite 完整性检查，能发现截断/半写）。
//! 3. `vault/vault.dat` 长度语义检查（AES-GCM 密文至少 12B nonce + 16B tag = 28B）。
//!
//! 任一项不过即返回错误，调用方中止迁移并且**不写配置**（原数据与现状不受影响）。

use std::path::Path;

use crate::framework::paths;
use crate::framework::storage::transfer;

/// 四分区名称（与 mod.rs 保持一致）
const PARTITIONS: [&str; 4] = ["data", "vault", "logs", "cache"];

/// 校验复制结果；失败返回含具体差异的错误信息
pub fn verify_copy(src_root: &Path, dst_root: &Path) -> Result<(), String> {
    for name in PARTITIONS {
        let src = src_root.join(name);
        if !src.exists() {
            continue;
        }
        let dst = dst_root.join(name);
        let src_files = transfer::count_files(&src)?;
        let dst_files = transfer::count_files(&dst)?;
        if src_files != dst_files {
            return Err(format!(
                "校验失败：{name} 分区文件数不一致（源 {src_files}，目标 {dst_files}）"
            ));
        }
        let src_bytes = paths::dir_size(&src)?;
        let dst_bytes = paths::dir_size(&dst)?;
        if src_bytes != dst_bytes {
            return Err(format!(
                "校验失败：{name} 分区字节数不一致（源 {src_bytes}，目标 {dst_bytes}）"
            ));
        }
    }
    check_sqlite_dbs(&dst_root.join("data"))?;
    check_vault_file(&dst_root.join("vault"))?;
    Ok(())
}

/// 逐个校验目标数据分区下的 SQLite 库（`PRAGMA quick_check`）
fn check_sqlite_dbs(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("读取 {} 失败: {e}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            check_sqlite_dbs(&path)?;
            continue;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 建立本测试专用临时目录
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("patchybox-verify-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn verify_passes_for_identical_copy() {
        let dir = temp_dir("ok");
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(src.join("data")).unwrap();
        // 造一个真实可用的 SQLite 库，验证 quick_check 分支
        let conn = rusqlite::Connection::open(src.join("data").join("ssh.db")).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER);").unwrap();
        drop(conn);
        std::fs::create_dir_all(src.join("vault")).unwrap();
        std::fs::write(src.join("vault").join("vault.dat"), vec![0u8; 32]).unwrap();

        let (_, _) = transfer::copy_tree(&src.join("data"), &dst.join("data")).unwrap();
        let (_, _) = transfer::copy_tree(&src.join("vault"), &dst.join("vault")).unwrap();
        assert!(verify_copy(&src, &dst).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_file_count_mismatch() {
        let dir = temp_dir("count");
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(src.join("data")).unwrap();
        std::fs::create_dir_all(dst.join("data")).unwrap();
        std::fs::write(src.join("data").join("a.db"), b"x").unwrap();
        // 目标少一个文件 → 应报错
        let err = verify_copy(&src, &dst).unwrap_err();
        assert!(err.contains("文件数不一致"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_truncated_vault() {
        let dir = temp_dir("vault");
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(src.join("vault")).unwrap();
        std::fs::create_dir_all(dst.join("vault")).unwrap();
        std::fs::write(src.join("vault").join("vault.dat"), vec![0u8; 8]).unwrap();
        std::fs::write(dst.join("vault").join("vault.dat"), vec![0u8; 8]).unwrap();
        let err = verify_copy(&src, &dst).unwrap_err();
        assert!(err.contains("长度异常"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_fails_on_corrupted_db() {
        let dir = temp_dir("db");
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(src.join("data")).unwrap();
        std::fs::create_dir_all(dst.join("data")).unwrap();
        // 两侧同样的垃圾内容：字节数一致，但 quick_check 应该失败
        let garbage = b"this is not a sqlite database at all";
        std::fs::write(src.join("data").join("bad.db"), garbage).unwrap();
        std::fs::write(dst.join("data").join("bad.db"), garbage).unwrap();
        let err = verify_copy(&src, &dst).unwrap_err();
        assert!(err.contains("校验失败"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
