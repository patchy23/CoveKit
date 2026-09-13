//! 框架 · 旧布局迁移的搬运引擎（文件/目录级原语）
//!
//! 由 `layout` 模块调用，不对外暴露：暂存复制、摘要比对、内容一致性与整组回滚
//! 都集中在这里，保证「先暂存、校验、再提交」这套顺序只有一份实现。

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::framework::paths;

use super::groups::ItemKind;
use super::report::{ItemOutcome, ItemState};

/// 单个成员的暂存复制：复制到 `<目标>.partial-<随机>` → 校验 → 改名到位。
pub(crate) struct StagedCopy {
    /// 最终目标路径
    target: PathBuf,
    /// 临时路径
    partial: PathBuf,
}

impl StagedCopy {
    /// 开始一次暂存复制（不覆盖已存在的临时文件）
    pub(crate) fn begin(target: &Path, token: &str) -> Result<Self, String> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
        }
        let partial = target.with_file_name(format!(
            "{}.partial-{token}",
            target.file_name().unwrap_or_default().to_string_lossy()
        ));
        Ok(Self {
            target: target.to_path_buf(),
            partial,
        })
    }

    /// 复制数据到临时位置
    pub(crate) fn copy(&self, from: &Path) -> Result<(), String> {
        copy_tree(from, &self.partial)
    }

    /// 校验临时副本与源一致（字节数 + 摘要）
    pub(crate) fn verify_against(&self, from: &Path) -> Result<(), String> {
        verify_same(from, &self.partial)
    }

    /// 提交：改名到位（同卷原子操作）
    pub(crate) fn commit(self) -> Result<(), String> {
        std::fs::rename(&self.partial, &self.target)
            .map_err(|e| format!("{} 改名失败: {e}", self.partial.display()))
    }

    /// 放弃：清理本次任务的临时文件（失败路径不得留下半截数据）
    pub(crate) fn discard(&self) {
        let _ = remove_tree(&self.partial);
    }
}

/// 目标已存在时的处理结论
pub(crate) enum Conflict {
    /// 内容一致：旧位置可以直接清理
    Identical,
    /// 内容不一致：新位置需留档后改用旧位置数据
    Divergent,
}

/// 递归复制（文件或目录）；目录项读取失败立即报错，不用 `flatten` 静默漏项
pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(from)
        .map_err(|e| format!("读取 {} 失败: {e}", from.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "{} 是符号链接/目录联接，布局迁移拒绝跟随链接",
            from.display()
        ));
    }
    if meta.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
        }
        std::fs::copy(from, to).map_err(|e| format!("复制 {} 失败: {e}", from.display()))?;
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("创建 {} 失败: {e}", to.display()))?;
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("读取目录 {} 失败: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录 {} 项失败: {e}", from.display()))?;
        copy_tree(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

/// 递归删除（仅用于清理本次任务创建的临时文件）
pub(crate) fn remove_tree(path: &Path) -> Result<(), String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("读取 {} 失败: {error}", path.display())),
    };
    if meta.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("删除 {} 失败: {e}", path.display()))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("删除 {} 失败: {e}", path.display()))
    }
}

/// 单文件摘要（流式读取，避免整文件进内存）
pub(crate) fn digest_of(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("打开 {} 失败: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 目录的结构化描述：相对路径 → (字节数, 摘要)
pub(crate) fn dir_entries(root: &Path) -> Result<Vec<(String, u64, String)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|e| format!("读取目录 {} 失败: {e}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取目录 {} 项失败: {e}", current.display()))?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path)
                .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "{} 是符号链接/目录联接，布局迁移拒绝跟随链接",
                    path.display()
                ));
            }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, meta.len(), digest_of(&path)?));
        }
    }
    out.sort();
    Ok(out)
}

/// 比较两个路径内容是否一致（文件比字节数 + 摘要；目录比条目集合）
pub(crate) fn verify_same(from: &Path, to: &Path) -> Result<(), String> {
    let from_meta = std::fs::symlink_metadata(from)
        .map_err(|e| format!("读取 {} 失败: {e}", from.display()))?;
    if from_meta.is_dir() {
        let left = dir_entries(from)?;
        let right = dir_entries(to)?;
        if left != right {
            return Err(format!(
                "目录内容不一致：源 {} 项，暂存 {} 项（有缺失、多余或内容不同）",
                left.len(),
                right.len()
            ));
        }
        return Ok(());
    }
    let to_meta =
        std::fs::symlink_metadata(to).map_err(|e| format!("读取 {} 失败: {e}", to.display()))?;
    if from_meta.len() != to_meta.len() {
        return Err(format!(
            "字节数不一致：源 {}，暂存 {}",
            from_meta.len(),
            to_meta.len()
        ));
    }
    let left = digest_of(from)?;
    let right = digest_of(to)?;
    if left != right {
        return Err("内容摘要不一致（同长度但内容不同）".into());
    }
    Ok(())
}

/// 判断新位置内容是否与旧位置一致
pub(crate) fn compare(from: &Path, to: &Path) -> Conflict {
    match verify_same(from, to) {
        Ok(()) => Conflict::Identical,
        Err(_) => Conflict::Divergent,
    }
}
/// 处理一个集合成员
pub(crate) fn migrate_member(
    root: &Path,
    group: &'static str,
    name: &str,
    partition: &str,
    token: &str,
    kind: ItemKind,
) -> ItemOutcome {
    let from = root.join(name);
    let to = root.join(partition).join(name);
    let target_label = format!("{partition}/{name}");
    let mut outcome = ItemOutcome {
        group,
        name: name.to_string(),
        target: target_label,
        state: ItemState::Missing,
        detail: String::new(),
    };

    if !from.exists() {
        // 旧位置没有：新位置已有数据也算到位
        if to.exists() {
            outcome.state = ItemState::AlreadyCurrent;
            outcome.detail = "旧位置无该文件，新位置已有数据".into();
        }
        return outcome;
    }

    if to.exists() {
        return match compare(&from, &to) {
            Conflict::Identical => {
                if let Err(error) = remove_tree(&from) {
                    outcome.state = ItemState::Failed;
                    outcome.detail = format!("新位置内容一致，但旧位置清理失败: {error}");
                    return outcome;
                }
                outcome.state = ItemState::AlreadyCurrent;
                outcome.detail = "新位置内容一致，已清理旧位置".into();
                outcome
            }
            Conflict::Divergent => {
                // 新位置不完整/不一致：改名留档后改用旧位置数据，绝不静默覆盖
                let archived = to.with_file_name(format!(
                    "{}.incomplete-{}",
                    to.file_name().unwrap_or_default().to_string_lossy(),
                    paths::now_stamp()
                ));
                if let Err(error) = std::fs::rename(&to, &archived) {
                    outcome.state = ItemState::Failed;
                    outcome.detail = format!("新位置内容不一致且留档失败: {error}");
                    return outcome;
                }
                match staged_move(&from, &to, token, kind) {
                    Ok(()) => {
                        outcome.state = ItemState::ConflictResolved;
                        outcome.detail = format!(
                            "新位置内容不一致，已留档为 {}，改用旧位置数据",
                            archived.file_name().unwrap_or_default().to_string_lossy()
                        );
                    }
                    Err(error) => {
                        // 搬入失败：把留档改回原名，保持新位置原状
                        let _ = std::fs::rename(&archived, &to);
                        outcome.state = ItemState::Failed;
                        outcome.detail = format!("改用旧位置数据失败: {error}");
                    }
                }
                outcome
            }
        };
    }

    match staged_move(&from, &to, token, kind) {
        Ok(()) => {
            outcome.state = ItemState::Migrated;
            outcome.detail = String::new();
        }
        Err(error) => {
            outcome.state = ItemState::Failed;
            outcome.detail = error;
        }
    }
    outcome
}

/// 暂存复制 + 校验 + 改名到位；失败时清理临时文件并保留旧位置
pub(crate) fn staged_move(
    from: &Path,
    to: &Path,
    token: &str,
    kind: ItemKind,
) -> Result<(), String> {
    // 期望类型校验：库里是文件、目录是目录；类型不符说明现场被人为改动过，
    // 直接报失败让整组保持原位，不做「顺手搬走一个目录」这种猜测式处理
    let meta = std::fs::symlink_metadata(from)
        .map_err(|e| format!("读取 {} 属性失败: {e}", from.display()))?;
    let actual_is_dir = meta.is_dir();
    if actual_is_dir != matches!(kind, ItemKind::Dir) {
        return Err(format!(
            "{} 的类型与预期不符（预期{}，实际{}）",
            from.display(),
            if matches!(kind, ItemKind::Dir) {
                "目录"
            } else {
                "文件"
            },
            if actual_is_dir { "目录" } else { "文件" }
        ));
    }
    let staged = StagedCopy::begin(to, token)?;
    if let Err(error) = staged.copy(from) {
        staged.discard();
        return Err(error);
    }
    if let Err(error) = staged.verify_against(from) {
        staged.discard();
        return Err(format!("暂存校验失败: {error}"));
    }
    if let Err(error) = staged.commit() {
        // 改名失败：清理临时文件，旧位置保持不变
        let staged = StagedCopy::begin(to, token)?;
        staged.discard();
        return Err(error);
    }
    if let Err(error) = remove_tree(from) {
        // 新位置已就绪，旧位置清理失败只告警（下次启动按一致内容再清理）
        eprintln!(
            "[storage] {} 已搬入新位置，但旧位置清理失败（下次启动重试）: {error}",
            from.display()
        );
    }
    Ok(())
}
