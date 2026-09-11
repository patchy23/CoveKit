//! 框架 · 存储迁移的文件复制与统计
//!
//! 只做「读源 → 写目标」，不做删除（迁移后原目录保留，由用户确认后清理）。

use std::path::Path;

/// 递归复制目录（保留相对路径），返回 `(文件数, 字节数)`
pub fn copy_tree(from: &Path, to: &Path) -> Result<(u64, u64), String> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    copy_recursive(from, to, &mut files, &mut bytes)?;
    Ok((files, bytes))
}

/// 递归复制实现：文件直接复制，目录创建后递归
fn copy_recursive(from: &Path, to: &Path, files: &mut u64, bytes: &mut u64) -> Result<(), String> {
    let meta = std::fs::metadata(from).map_err(|e| format!("读取 {} 失败: {e}", from.display()))?;
    if meta.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
        }
        std::fs::copy(from, to).map_err(|e| format!("复制 {} 失败: {e}", from.display()))?;
        *files += 1;
        *bytes += meta.len();
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("创建 {} 失败: {e}", to.display()))?;
    for entry in std::fs::read_dir(from)
        .map_err(|e| format!("读取目录 {} 失败: {e}", from.display()))?
        .flatten()
    {
        copy_recursive(&entry.path(), &to.join(entry.file_name()), files, bytes)?;
    }
    Ok(())
}

/// 统计目录下文件数量（不含目录本身；不存在返回 0）
pub fn count_files(dir: &Path) -> Result<u64, String> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0u64;
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("读取目录 {} 失败: {e}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() => count += 1,
            Ok(_) => count += count_files(&path)?,
            Err(_) => {}
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 建立本测试专用临时目录
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("patchybox-transfer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copy_tree_preserves_structure_and_counts() {
        let dir = temp_dir("copy");
        let src = dir.join("data");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("a.db"), b"1234").unwrap();
        std::fs::write(src.join("nested").join("b.db"), b"123456").unwrap();

        let dst = dir.join("target").join("data");
        let (files, bytes) = copy_tree(&src, &dst).unwrap();
        assert_eq!(files, 2);
        assert_eq!(bytes, 10);
        assert_eq!(count_files(&dst).unwrap(), 2);
        // 源保留（迁移只复制不删）
        assert!(src.join("a.db").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn count_files_returns_zero_for_missing_dir() {
        let dir = temp_dir("count-missing");
        assert_eq!(count_files(&dir.join("nope")).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
