//! 框架 · 旧布局集合迁移（可靠性 T03）
//!
//! 旧版本把数据直接放在存储根下，现布局分四分区（`data` / `vault` / `logs` / `cache`）。
//! 本模块负责把根下的旧文件搬进分区，规则（任务书 T03）：
//!
//! 1. **按集合迁移**：密文与其主密钥、数据库与其 `-wal` / `-shm` / `-journal` 属于同一集合，
//!    要么全部到位，要么全部保留原位（缺一半会造成「能解密但读不到数据」或反之）。
//! 2. **暂存后提交**：先复制到临时名并校验（字节数 + SHA-256），校验通过才改名进分区；
//!    复制中断不会在分区里留下半截文件，重试也不会因为「目标存在」而跳过。
//! 3. **目标存在 ≠ 目标完整**：新位置已有同名文件时做内容比对 —— 一致则清理旧位置；
//!    不一致（半截/损坏）把新位置改名留档为 `.incomplete-<时间戳>`，再把旧位置搬入，**绝不静默覆盖**。
//! 4. **版本只在全部必需项完成后推进**：任一项失败就保持原版本，下次启动继续修复（修复遍）。
//!    因此 `layoutVersion` 达到当前值时也仍然检查旧位置残留，不会把「已推进版本」当「无需修复」。

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::framework::paths;

/// 集合成员类型：文件、目录，或由扫描动态发现的一组数据库文件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// 普通文件
    File,
    /// 目录（整体搬入分区）
    Dir,
}

/// 集合成员：旧位置名 + 目标分区
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// 根下旧名字
    pub name: &'static str,
    /// 目标分区（data / vault / logs / cache）
    pub partition: &'static str,
    /// 文件或目录
    pub kind: ItemKind,
}

/// 迁移集合
#[derive(Debug, Clone, Copy)]
pub struct Group {
    /// 集合标识（报告与日志用）
    pub id: &'static str,
    /// 集合成员
    pub members: &'static [Member],
}

/// 构造文件的简写
const fn file(name: &'static str, partition: &'static str) -> Member {
    Member {
        name,
        partition,
        kind: ItemKind::File,
    }
}

/// 构造目录的简写
const fn dir(name: &'static str, partition: &'static str) -> Member {
    Member {
        name,
        partition,
        kind: ItemKind::Dir,
    }
}

const CREDENTIALS: [Member; 2] = [
    file("credentials-master.key", "data"),
    dir("credentials", "data"),
];

const VAULT: [Member; 2] = [
    file("vault.dat", "vault"),
    file("vault-master.key", "vault"),
];

const SSH_CREDENTIALS: [Member; 2] = [
    file("ssh-credentials.json", "data"),
    file("ssh-master.key", "data"),
];

const DB_SECRETS: [Member; 3] = [
    file("db-secrets.enc", "data"),
    file("db-master.key", "data"),
    file("db-secrets.bak", "data"),
];

const DB_STRONGHOLD: [Member; 2] = [
    file("db.stronghold", "data"),
    file("db-client.snapshot", "data"),
];

const SSH_KNOWN_HOSTS: [Member; 1] = [file("ssh-known-hosts", "data")];
const TTS_CACHE: [Member; 1] = [dir("tts", "cache")];
const AGENTS_CACHE: [Member; 1] = [dir("agents", "cache")];

/// 固定集合表（`*.db` 由扫描动态补充）
pub const GROUPS: [Group; 8] = [
    Group {
        id: "credentials",
        members: &CREDENTIALS,
    },
    Group {
        id: "vault",
        members: &VAULT,
    },
    Group {
        id: "ssh-credentials",
        members: &SSH_CREDENTIALS,
    },
    Group {
        id: "db-secrets",
        members: &DB_SECRETS,
    },
    Group {
        id: "db-stronghold",
        members: &DB_STRONGHOLD,
    },
    Group {
        id: "ssh-known-hosts",
        members: &SSH_KNOWN_HOSTS,
    },
    Group {
        id: "tts-cache",
        members: &TTS_CACHE,
    },
    Group {
        id: "agents-cache",
        members: &AGENTS_CACHE,
    },
];
/// 单项处理结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    /// 已从旧位置搬入分区
    Migrated,
    /// 新位置已有相同内容，旧位置已清理
    AlreadyCurrent,
    /// 旧位置没有该数据（无需处理）
    Missing,
    /// 新位置内容不一致，已留档并改用旧位置数据
    ConflictResolved,
    /// 处理失败（该项保持原位，版本不推进）
    Failed,
}

impl ItemState {
    /// 稳定字符串（报告/日志用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Migrated => "migrated",
            Self::AlreadyCurrent => "already-current",
            Self::Missing => "missing",
            Self::ConflictResolved => "conflict-resolved",
            Self::Failed => "failed",
        }
    }

    /// 是否表示旧位置已无残留
    pub fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Migrated | Self::AlreadyCurrent | Self::ConflictResolved
        )
    }
}

/// 单个集合成员的结果
#[derive(Debug, Clone)]
pub struct ItemOutcome {
    /// 集合标识
    pub group: &'static str,
    /// 旧位置名
    pub name: String,
    /// 目标路径（相对根）
    pub target: String,
    /// 处理结果
    pub state: ItemState,
    /// 失败原因或冲突说明
    pub detail: String,
}

/// 迁移报告
#[derive(Debug, Clone, Default)]
pub struct LayoutReport {
    /// 逐成员结果
    pub outcomes: Vec<ItemOutcome>,
    /// 本次是否推进了布局版本
    pub version_advanced: bool,
}

impl LayoutReport {
    /// 成功搬移（含冲突改用旧数据）的项数
    pub fn moved(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.state == ItemState::Migrated)
            .count()
    }

    /// 失败项
    pub fn failures(&self) -> Vec<&ItemOutcome> {
        self.outcomes
            .iter()
            .filter(|o| o.state == ItemState::Failed)
            .collect()
    }

    /// 是否存在失败项（有失败则版本不推进）
    pub fn has_failures(&self) -> bool {
        !self.failures().is_empty()
    }

    /// 一行摘要（启动日志用；失败项带上集合与状态，便于直接定位）
    pub fn summary(&self) -> String {
        let detail = self
            .failures()
            .iter()
            .map(|o| format!("{}[{}]={}", o.target, o.group, o.state.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut line = format!(
            "布局迁移：搬移 {} 项，一致清理 {} 项，冲突改用旧数据 {} 项，未处理 {} 项{}",
            self.moved(),
            self.outcomes
                .iter()
                .filter(|o| o.state == ItemState::AlreadyCurrent)
                .count(),
            self.outcomes
                .iter()
                .filter(|o| o.state == ItemState::ConflictResolved)
                .count(),
            self.outcomes
                .iter()
                .filter(|o| !o.state.is_settled())
                .count(),
            if self.version_advanced {
                "，版本已推进"
            } else {
                "，版本保持不变"
            }
        );
        if !detail.is_empty() {
            line.push_str(&format!("；失败/未处理：{detail}"));
        }
        line
    }
}

/// 数据库恢复文件后缀：与 `X.db` 同属一个集合，必须一起搬
const DB_COMPANIONS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// 扫描根下属于某个数据库集合的成员名（`X.db` 与其伴随文件，只列出实际存在的）
fn db_group_members(root: &Path, db_name: &str) -> Vec<String> {
    let mut names = vec![db_name.to_string()];
    for suffix in DB_COMPANIONS {
        let candidate = format!("{db_name}{suffix}");
        if root.join(&candidate).exists() {
            names.push(candidate);
        }
    }
    names
}

/// 列出根下所有动态数据库集合（`*.db`）
fn dynamic_db_groups(root: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| format!("读取存储根目录失败: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取存储根目录项失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_ascii_lowercase().ends_with(".db") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// 旧位置残留清单（修复遍判定：即使 `layoutVersion` 已达标也要检查）
pub fn leftovers(root: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for group in GROUPS {
        for member in group.members {
            if root.join(member.name).exists() {
                names.push(member.name.to_string());
            }
        }
    }
    names.extend(dynamic_db_groups(root)?);
    names.sort();
    names.dedup();
    Ok(names)
}
/// 单个成员的暂存复制：复制到 `<目标>.partial-<随机>` → 校验 → 改名到位。
struct StagedCopy {
    /// 最终目标路径
    target: PathBuf,
    /// 临时路径
    partial: PathBuf,
}

impl StagedCopy {
    /// 开始一次暂存复制（不覆盖已存在的临时文件）
    fn begin(target: &Path, token: &str) -> Result<Self, String> {
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
    fn copy(&self, from: &Path) -> Result<(), String> {
        copy_tree(from, &self.partial)
    }

    /// 校验临时副本与源一致（字节数 + 摘要）
    fn verify_against(&self, from: &Path) -> Result<(), String> {
        verify_same(from, &self.partial)
    }

    /// 提交：改名到位（同卷原子操作）
    fn commit(self) -> Result<(), String> {
        std::fs::rename(&self.partial, &self.target)
            .map_err(|e| format!("{} 改名失败: {e}", self.partial.display()))
    }

    /// 放弃：清理本次任务的临时文件（失败路径不得留下半截数据）
    fn discard(&self) {
        let _ = remove_tree(&self.partial);
    }
}

/// 目标已存在时的处理结论
enum Conflict {
    /// 内容一致：旧位置可以直接清理
    Identical,
    /// 内容不一致：新位置需留档后改用旧位置数据
    Divergent,
}

/// 递归复制（文件或目录）；目录项读取失败立即报错，不用 `flatten` 静默漏项
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
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
fn remove_tree(path: &Path) -> Result<(), String> {
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
fn digest_of(path: &Path) -> Result<String, String> {
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
fn dir_entries(root: &Path) -> Result<Vec<(String, u64, String)>, String> {
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
fn verify_same(from: &Path, to: &Path) -> Result<(), String> {
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
fn compare(from: &Path, to: &Path) -> Conflict {
    match verify_same(from, to) {
        Ok(()) => Conflict::Identical,
        Err(_) => Conflict::Divergent,
    }
}
/// 处理一个集合成员
fn migrate_member(
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
fn staged_move(from: &Path, to: &Path, token: &str, kind: ItemKind) -> Result<(), String> {
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

/// 旧布局迁移的纯实现（不需要 AppHandle，便于用临时目录单测）。
///
/// 语义：按集合处理，集合内任一成员失败即整组保留原位（已提交的成员回滚）。
pub fn migrate_layout_at(root: &Path) -> Result<LayoutReport, String> {
    let token = paths::now_stamp();
    let mut report = LayoutReport::default();

    // 1) 固定集合
    for group in GROUPS {
        let names = group_member_names(root, &group);
        let existing: Vec<String> = names
            .iter()
            .filter(|name| root.join(name).exists())
            .cloned()
            .collect();
        if existing.is_empty() {
            for name in &names {
                report.outcomes.push(ItemOutcome {
                    group: group.id,
                    name: name.clone(),
                    target: format!("{}/{name}", group.partition_of(name)),
                    state: ItemState::Missing,
                    detail: String::new(),
                });
            }
            continue;
        }
        let mut group_outcomes = Vec::new();
        for name in &names {
            let partition = group.partition_of(name);
            let kind = group.kind_of(name);
            group_outcomes.push(migrate_member(
                root, group.id, name, partition, &token, kind,
            ));
        }
        let failed = group_outcomes.iter().any(|o| o.state == ItemState::Failed);
        if failed {
            // 集合回滚：把已完成搬移的成员退回旧位置，保证「要么全到位要么全原位」
            for outcome in &group_outcomes {
                if outcome.state == ItemState::Migrated {
                    let to = root.join(&outcome.target);
                    let from = root.join(&outcome.name);
                    if let Err(error) = std::fs::rename(&to, &from) {
                        eprintln!(
                            "[storage] 集合 {} 回滚 {} 失败（新位置数据仍在）: {error}",
                            group.id, outcome.target
                        );
                    }
                }
            }
        }
        report.outcomes.extend(group_outcomes);
    }

    // 2) 动态数据库集合
    for db_name in dynamic_db_groups(root)? {
        let names = db_group_members(root, &db_name);
        let mut group_outcomes = Vec::new();
        for name in &names {
            group_outcomes.push(migrate_member(
                root,
                "plugin-db",
                name,
                "data",
                &token,
                ItemKind::File,
            ));
        }
        if group_outcomes.iter().any(|o| o.state == ItemState::Failed) {
            for outcome in &group_outcomes {
                if outcome.state == ItemState::Migrated {
                    let to = root.join(&outcome.target);
                    let from = root.join(&outcome.name);
                    if let Err(error) = std::fs::rename(&to, &from) {
                        eprintln!(
                            "[storage] 数据库集合 {db_name} 回滚 {} 失败: {error}",
                            outcome.target
                        );
                    }
                }
            }
        }
        report.outcomes.extend(group_outcomes);
    }

    Ok(report)
}
impl Group {
    /// 成员名对应的目标分区（数据库伴随文件跟随其 `.db` 成员）
    fn partition_of(&self, name: &str) -> &'static str {
        for member in self.members {
            if name == member.name || name.starts_with(&format!("{}.db", member.name)) {
                return member.partition;
            }
        }
        self.members[0].partition
    }

    /// 成员名对应的期望类型（数据库伴随文件一律按文件处理）
    fn kind_of(&self, name: &str) -> ItemKind {
        for member in self.members {
            if name == member.name {
                return member.kind;
            }
        }
        ItemKind::File
    }
}

/// 集合成员名（数据库会把 `-wal` / `-shm` / `-journal` 一并纳入）
fn group_member_names(root: &Path, group: &Group) -> Vec<String> {
    let mut names = Vec::new();
    for member in group.members {
        if member.kind == ItemKind::Dir {
            names.push(member.name.to_string());
            continue;
        }
        if member.name.to_ascii_lowercase().ends_with(".db") {
            names.extend(db_group_members(root, member.name));
        } else {
            names.push(member.name.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// 旧布局迁移（含版本门槛与修复遍）。
///
/// - 版本已达当前值且旧位置无残留：直接返回空报告（不触盘）。
/// - 版本已达当前值但仍有残留（历史失败或版本被误推进）：仍然执行修复遍。
/// - **只有全部集合无失败时才推进版本**，失败保持原版本，下次启动继续。
pub fn migrate_layout(app: &tauri::AppHandle) -> Result<LayoutReport, String> {
    let root = paths::storage_root(app)?;
    let current = paths::layout_version(app);
    let pending = leftovers(&root)?;
    if current >= paths::LAYOUT_VERSION && pending.is_empty() {
        return Ok(LayoutReport {
            outcomes: Vec::new(),
            version_advanced: false,
        });
    }

    let mut report = migrate_layout_at(&root)?;
    if report.has_failures() {
        eprintln!("[storage] {}", report.summary());
        return Ok(report);
    }

    if current < paths::LAYOUT_VERSION {
        paths::write_setting(
            app,
            paths::KEY_LAYOUT_VERSION,
            serde_json::json!(paths::LAYOUT_VERSION),
        )?;
        report.version_advanced = true;
    }
    eprintln!("[storage] {}", report.summary());
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 建立本测试专用临时目录
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchybox-layout-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 写入一个测试文件
    fn write(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// 生成一份真实 SQLite 数据库，便于验证「集合整体搬移」在真实文件上的行为
    fn make_db(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO t (v) VALUES ('x')", []).unwrap();
    }

    /// 旧位置项搬入分区并清理旧位置
    #[test]
    fn moves_legacy_items_into_partitions_and_clears_source() {
        let root = temp_root("moves");
        write(&root.join("credentials-master.key"), &[7u8; 32]);
        write(&root.join("credentials").join("database.enc"), b"cipher");
        write(&root.join("ssh-known-hosts"), b"host keys");
        write(&root.join("vault.dat"), b"vault");
        write(&root.join("vault-master.key"), &[9u8; 32]);
        make_db(&root.join("ssh.db"));
        write(&root.join("ssh.db-wal"), b"wal-page");

        let report = migrate_layout_at(&root).unwrap();
        assert!(
            !report.has_failures(),
            "不应有失败项: {:?}",
            report.failures()
        );
        assert!(root.join("data/credentials-master.key").exists());
        assert!(root.join("data/credentials/database.enc").exists());
        assert!(root.join("data/ssh.db").exists());
        assert!(
            root.join("data/ssh.db-wal").exists(),
            "数据库伴随文件必须同集合搬移"
        );
        assert!(root.join("data/ssh-known-hosts").exists());
        assert!(root.join("vault/vault.dat").exists());
        assert!(root.join("vault/vault-master.key").exists());
        // 旧位置已清空
        assert!(!root.join("credentials-master.key").exists());
        assert!(!root.join("ssh.db").exists());
        assert!(!root.join("vault.dat").exists());
        assert!(leftovers(&root).unwrap().is_empty());
    }

    /// 新位置已有相同内容：清理旧位置，不重复搬，也不算失败
    #[test]
    fn identical_target_clears_legacy_without_conflict() {
        let root = temp_root("identical");
        write(&root.join("vault.dat"), b"same-bytes");
        write(&root.join("vault/vault.dat"), b"same-bytes");

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(!root.join("vault.dat").exists(), "内容一致时旧位置应被清理");
        assert!(root.join("vault/vault.dat").exists());
        let state = report
            .outcomes
            .iter()
            .find(|o| o.name == "vault.dat")
            .map(|o| o.state)
            .unwrap();
        assert_eq!(state, ItemState::AlreadyCurrent);
    }

    /// 新位置内容不一致（半截）：留档并改用旧位置数据，绝不静默覆盖
    #[test]
    fn divergent_target_is_archived_and_legacy_wins() {
        let root = temp_root("divergent");
        write(&root.join("vault.dat"), b"complete-legacy-data");
        write(&root.join("vault/vault.dat"), b"half");

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures(), "{:?}", report.failures());
        assert_eq!(
            std::fs::read(root.join("vault/vault.dat")).unwrap(),
            b"complete-legacy-data"
        );
        let archived: Vec<_> = std::fs::read_dir(root.join("vault"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".incomplete-"))
            .collect();
        assert_eq!(archived.len(), 1, "半截目标必须留档: {archived:?}");
        assert!(!root.join("vault.dat").exists());
    }

    /// 集合内任一成员失败：整组保持原位（已搬移的成员回滚），版本不推进
    #[test]
    fn group_failure_rolls_back_whole_set() {
        let root = temp_root("group-rollback");
        make_db(&root.join("app.db"));
        // app.db-wal 故意做成目录：按文件复制必然失败，用来验证集合整体回滚
        std::fs::create_dir_all(root.join("app.db-wal")).unwrap();

        let report = migrate_layout_at(&root).unwrap();
        assert!(report.has_failures(), "成员失败必须报失败: {report:?}");
        assert!(
            root.join("app.db").exists(),
            "失败集合必须整体回滚，数据库留在旧位置"
        );
        assert!(root.join("app.db-wal").is_dir());
        assert!(!root.join("data/app.db").exists(), "回滚后新位置不得残留");
        let strays: Vec<String> = std::fs::read_dir(root.join("data"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        assert!(strays.is_empty(), "暂存残留必须清理: {strays:?}");
    }

    /// 密文与主密钥同集合：密钥缺失不影响密文搬迁，但两者都在时一起走
    #[test]
    fn ciphertext_and_key_move_together() {
        let root = temp_root("pair");
        write(&root.join("db-secrets.enc"), b"cipher");
        write(&root.join("db-master.key"), &[3u8; 32]);

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(root.join("data/db-secrets.enc").exists());
        assert!(root.join("data/db-master.key").exists());
        assert!(!root.join("db-secrets.enc").exists());
        assert!(!root.join("db-master.key").exists());
    }

    /// 修复遍：版本已达标但旧位置仍有残留（历史失败）→ 仍然修
    #[test]
    fn repair_pass_detects_leftovers_even_when_version_current() {
        let root = temp_root("repair");
        std::fs::create_dir_all(root.join("data")).unwrap();
        write(&root.join("db-master.key"), &[5u8; 32]);

        assert_eq!(leftovers(&root).unwrap(), vec!["db-master.key".to_string()]);
        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(root.join("data/db-master.key").exists());
        assert!(leftovers(&root).unwrap().is_empty(), "修复后旧位置应无残留");
    }

    /// 符号链接必须被拒绝并指出路径（不跟随）
    #[cfg(windows)]
    #[test]
    fn symlink_member_is_refused_with_path() {
        let root = temp_root("symlink");
        let target = root.join("real-tree");
        write(&target.join("inner.txt"), b"data");
        let link = root.join("agents");
        if std::os::windows::fs::symlink_dir(&target, &link).is_err() {
            return; // 无创建符号链接权限（未开发者模式）时跳过
        }

        let report = migrate_layout_at(&root).unwrap();
        let failed = report.failures();
        assert_eq!(failed.len(), 1);
        assert!(
            failed[0].detail.contains("符号链接"),
            "{}",
            failed[0].detail
        );
        assert!(link.exists(), "失败的项保留原位");
    }

    /// 空根目录：无项可搬，也不算失败（允许空数据环境迁移位置）
    #[test]
    fn empty_root_is_noop_without_failures() {
        let root = temp_root("empty");
        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert_eq!(report.moved(), 0);
        assert!(leftovers(&root).unwrap().is_empty());
    }
}
