//! 框架 · 存储迁移的文件复制与统计
//!
//! 只做「读源 → 写暂存区」，不做删除（迁移后原目录保留，由用户确认后清理）。
//!
//! 安全契约（T02）：
//! - 遇到 symlink / junction / 其他 reparse point 立即报错，**不跟随链接**（跟随会写到链接目标之外）。
//! - `read_dir` 失败必须报错，不用 `flatten()` 静默漏项（漏掉的文件在清单校验前不可见）。
//! - 每个文件复制后按回调上报进度，便于按文件节流而非按分区。

use std::path::Path;

/// 复制进度回调：`(已复制文件数, 已复制字节数, 当前相对路径)`
pub type CopyProgress<'a> = &'a mut dyn FnMut(u64, u64, &str);

/// 递归复制目录（保留相对路径），返回 `(文件数, 字节数)`；生产链路用带进度的 `copy_tree_with`
#[cfg(test)]
pub fn copy_tree(from: &Path, to: &Path) -> Result<(u64, u64), String> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    copy_recursive(
        from,
        to,
        &mut files,
        &mut bytes,
        &mut |_files: u64, _bytes: u64, _rel: &str| {},
    )?;
    Ok((files, bytes))
}

/// 带进度回调的递归复制
pub fn copy_tree_with(
    from: &Path,
    to: &Path,
    progress: CopyProgress<'_>,
) -> Result<(u64, u64), String> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    copy_recursive(from, to, &mut files, &mut bytes, progress)?;
    Ok((files, bytes))
}

/// 递归复制实现：文件直接复制，目录创建后递归；链接与非普通文件项一律拒绝
fn copy_recursive(
    from: &Path,
    to: &Path,
    files: &mut u64,
    bytes: &mut u64,
    progress: CopyProgress<'_>,
) -> Result<(), String> {
    // 用 symlink_metadata：metadata 会跟随链接，先把链接目标当成普通文件复制
    let meta = std::fs::symlink_metadata(from)
        .map_err(|e| format!("读取 {} 失败: {e}", from.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "发现符号链接或目录联接，迁移不支持链接，已拒绝：{}",
            from.display()
        ));
    }
    if meta.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
        }
        std::fs::copy(from, to).map_err(|e| format!("复制 {} 失败: {e}", from.display()))?;
        *files += 1;
        *bytes += meta.len();
        progress(*files, *bytes, &from.to_string_lossy());
        return Ok(());
    }
    if !meta.is_dir() {
        return Err(format!(
            "发现非普通文件项，迁移无法处理：{}",
            from.display()
        ));
    }
    std::fs::create_dir_all(to).map_err(|e| format!("创建 {} 失败: {e}", to.display()))?;
    for entry in
        std::fs::read_dir(from).map_err(|e| format!("读取目录 {} 失败: {e}", from.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录 {} 的目录项失败: {e}", from.display()))?;
        copy_recursive(
            &entry.path(),
            &to.join(entry.file_name()),
            files,
            bytes,
            progress,
        )?;
    }
    Ok(())
}

/// 统计目录下文件数量（不含目录本身；不存在返回 0）
pub fn count_files(dir: &Path) -> Result<u64, String> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0u64;
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("读取目录 {} 失败: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录 {} 的目录项失败: {e}", dir.display()))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
        count += match meta.is_file() {
            true => 1,
            false if meta.is_dir() => count_files(&path)?,
            // 链接等非普通项：迁移路径不会走到这里（扫描阶段已拒绝），计数时按 1 个项计
            false => 1,
        };
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::storage::test_support::temp_dir;

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
    fn copy_tree_reports_every_file_through_progress() {
        let dir = temp_dir("copy-progress");
        let src = dir.join("data");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.db"), b"1").unwrap();
        std::fs::write(src.join("b.db"), b"22").unwrap();

        let mut seen: Vec<u64> = Vec::new();
        let mut last_bytes = 0u64;
        let (files, bytes) = copy_tree_with(&src, &dir.join("dst"), &mut |f, b, _rel| {
            seen.push(f);
            last_bytes = b;
        })
        .unwrap();
        assert_eq!((files, bytes), (2, 3));
        // 两个文件各回调一次（不只在分区结束时上报一次）
        assert_eq!(seen, vec![1, 2]);
        assert_eq!(last_bytes, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_tree_refuses_links() {
        let dir = temp_dir("copy-link");
        let src = dir.join("data");
        std::fs::create_dir_all(&src).unwrap();
        let real = src.join("real.db");
        std::fs::write(&real, b"payload").unwrap();
        let link = src.join("link.db");
        let created = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&real, &link).is_ok()
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(&real, &link).is_ok()
            }
        };
        if !created {
            eprintln!("跳过：当前环境无法创建符号链接");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let err = copy_tree(&src, &dir.join("dst")).unwrap_err();
        assert!(err.contains("符号链接"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn count_files_returns_zero_for_missing_dir() {
        let dir = temp_dir("count-missing");
        assert_eq!(count_files(&dir.join("nope")).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
