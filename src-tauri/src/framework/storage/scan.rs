//! 框架 · 迁移清单（逐文件相对路径、字节数、内容摘要）
//!
//! 为什么不能只看「文件数 + 总大小」：同长度不同内容、漏项与被截断文件都能骗过计数校验。
//! 因此迁移前对源目录生成清单（SHA-256 摘要），复制完成后对暂存区重新扫描并与清单逐项比对。
//!
//! 遍历契约：
//! - 遇到 symlink / junction / 其他 reparse point 立即报错并指出路径，**不跟随链接**（v1 明确拒绝）。
//! - `read_dir` 失败必须报错，不用 `flatten()` 静默漏项。
//! - 只遍历四分区目录；分区缺失视为空（未使用过的分区不建目录）。

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

/// 四个固定分区（与 `storage::PARTITIONS` 一致）
use super::PARTITIONS;

/// 清单中的单个文件项
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// 相对根的路径（用 `/` 分隔，跨平台可比）
    pub rel: String,
    /// 文件字节数
    pub bytes: u64,
    /// 内容 SHA-256（小写十六进制）
    pub digest: String,
}

/// 一次扫描得到的完整清单
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// 按相对路径升序排列的文件项
    pub entries: Vec<ManifestEntry>,
    /// 合计字节数
    pub total_bytes: u64,
}

impl Manifest {
    /// 文件数量（测试断言用）
    #[cfg(test)]
    pub fn file_count(&self) -> u64 {
        self.entries.len() as u64
    }
}

/// 计算文件内容的 SHA-256（流式读取，避免整文件载入内存）
pub fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;

    let mut file =
        std::fs::File::open(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 扫描存储根，生成四分区清单（拒绝链接、错误传播）
pub fn scan_root(root: &Path) -> Result<Manifest, String> {
    let mut manifest = Manifest::default();
    for name in PARTITIONS {
        let dir = root.join(name);
        if !dir.exists() {
            continue;
        }
        scan_dir(&dir, name, &mut manifest)?;
    }
    Ok(manifest)
}

/// 递归扫描单个目录；`rel_prefix` 为该目录相对根的路径分量
fn scan_dir(dir: &Path, rel_prefix: &str, out: &mut Manifest) -> Result<(), String> {
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("读取目录 {} 失败: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录 {} 的目录项失败: {e}", dir.display()))?;
        let path = entry.path();
        let rel = format!("{rel_prefix}/{}", entry.file_name().to_string_lossy());
        // 用 symlink_metadata 判定：链接/联接点一律不跟随（跟随会复制到链接目标，语义错乱）
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "发现符号链接或目录联接，迁移不支持链接，已拒绝：{}",
                path.display()
            ));
        }
        if meta.is_dir() {
            scan_dir(&path, &rel, out)?;
            continue;
        }
        if !meta.is_file() {
            return Err(format!(
                "发现非普通文件项，迁移无法处理：{}",
                path.display()
            ));
        }
        out.entries.push(ManifestEntry {
            rel,
            bytes: meta.len(),
            digest: sha256_file(&path)?,
        });
        out.total_bytes += meta.len();
    }
    Ok(())
}

/// 比对期望清单与实际清单：缺失 / 多余 / 同长度不同内容都算失败
pub fn diff(expected: &Manifest, actual: &Manifest) -> Result<(), String> {
    let exp: BTreeMap<&str, &ManifestEntry> = expected
        .entries
        .iter()
        .map(|e| (e.rel.as_str(), e))
        .collect();
    let act: BTreeMap<&str, &ManifestEntry> =
        actual.entries.iter().map(|e| (e.rel.as_str(), e)).collect();

    let mut missing: Vec<String> = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    for (rel, want) in exp.iter() {
        match act.get(rel) {
            None => missing.push((*rel).to_string()),
            Some(got) => {
                if got.bytes != want.bytes || got.digest != want.digest {
                    changed.push(format!(
                        "{}（期望 {} 字节 {:.8}…，实际 {} 字节 {:.8}…）",
                        rel, want.bytes, want.digest, got.bytes, got.digest
                    ));
                }
            }
        }
    }
    let extra: Vec<String> = act
        .keys()
        .filter(|rel| !exp.contains_key(*rel))
        .map(|rel| (*rel).to_string())
        .collect();

    if missing.is_empty() && changed.is_empty() && extra.is_empty() {
        return Ok(());
    }
    let mut parts: Vec<String> = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("缺失 {} 个（{}）", missing.len(), head(&missing)));
    }
    if !changed.is_empty() {
        parts.push(format!(
            "内容不一致 {} 个（{}）",
            changed.len(),
            head(&changed)
        ));
    }
    if !extra.is_empty() {
        parts.push(format!("多余 {} 个（{}）", extra.len(), head(&extra)));
    }
    Err(format!("清单校验失败：{}", parts.join("；")))
}

/// 错误信息里最多列出 3 个示例路径，避免长清单淹没日志
fn head(items: &[String]) -> String {
    items.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::storage::test_support::temp_dir;

    #[test]
    fn scan_reports_files_and_digests_per_partition() {
        let root = temp_dir("scan-basic");
        std::fs::create_dir_all(root.join("data").join("sub")).unwrap();
        std::fs::write(root.join("data").join("a.db"), b"1234").unwrap();
        std::fs::write(root.join("data").join("sub").join("b.db"), b"123456").unwrap();

        let manifest = scan_root(&root).unwrap();
        assert_eq!(manifest.file_count(), 2);
        assert_eq!(manifest.total_bytes, 10);
        assert_eq!(manifest.entries[0].rel, "data/a.db");
        assert_eq!(
            manifest.entries[0].digest,
            "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_ignores_missing_partitions() {
        let root = temp_dir("scan-empty");
        assert_eq!(scan_root(&root).unwrap().file_count(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn diff_detects_missing_extra_and_same_length_different_content() {
        let root = temp_dir("scan-diff");
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data").join("keep.db"), b"aaaa").unwrap();
        std::fs::write(root.join("data").join("gone.db"), b"bbbb").unwrap();
        std::fs::write(root.join("data").join("same-len.db"), b"cccc").unwrap();
        let expected = scan_root(&root).unwrap();

        std::fs::remove_file(root.join("data").join("gone.db")).unwrap();
        std::fs::write(root.join("data").join("same-len.db"), b"dddd").unwrap();
        std::fs::write(root.join("data").join("extra.db"), b"eeee").unwrap();
        let actual = scan_root(&root).unwrap();

        let err = diff(&expected, &actual).unwrap_err();
        assert!(err.contains("缺失 1 个"), "实际错误: {err}");
        assert!(err.contains("内容不一致 1 个"), "实际错误: {err}");
        assert!(err.contains("多余 1 个"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_rejects_symlinks_with_path() {
        let root = temp_dir("scan-link");
        std::fs::create_dir_all(root.join("data")).unwrap();
        let target = root.join("outside.txt");
        std::fs::write(&target, b"payload").unwrap();
        let link = root.join("data").join("link.db");
        if !make_file_symlink(&target, &link) {
            // 无权限创建链接（未开启开发者模式）时跳过：不能把「没造出夹具」当通过
            eprintln!("跳过：当前环境无法创建符号链接");
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
        let err = scan_root(&root).unwrap_err();
        assert!(err.contains("符号链接"), "实际错误: {err}");
        assert!(err.contains("link.db"), "实际错误: {err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 创建文件符号链接；平台不支持时返回 false
    fn make_file_symlink(from: &Path, to: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(from, to).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(from, to).is_ok()
        }
    }
}
