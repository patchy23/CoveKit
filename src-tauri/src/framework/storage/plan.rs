//! 框架 · 存储根迁移计划（可靠性 T01 的持久化部分）
//!
//! 语义约定（`docs/batches/rel-202609-001-可靠性与扩展治理/可靠性与扩展治理任务书.md` T01）：
//! - 用户选定新位置时**只登记计划**，不复制、不改生效根；继续操作仍写当前根。
//! - 复制与校验在**下次启动的维护阶段**执行（见 `migrate.rs`），成功后才提交新根。
//! - 计划存放于自举配置 `settings.json` 的 `app.pendingMigration`。该文件永远位于
//!   `app_data_dir`，与数据根指向哪个盘无关，因此计划在根不可用时仍可读取与修改。
//! - 计划里保留 `source`、`target`、阶段与最后一次失败原因：迁移失败后用户仍能看到
//!   原始信息，并据此重试或取消，而不是得到一条无上下文的自述错误。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

/// 待执行计划键（`settings.json` → `app.pendingMigration`）
pub const KEY_PENDING_MIGRATION: &str = "pendingMigration";
/// 最近一次成功迁移的留档键（`settings.json` → `app.lastMigration`）
pub const KEY_LAST_MIGRATION: &str = "lastMigration";

/// 迁移阶段（崩溃后靠阶段值识别半截状态，不靠猜）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MigrationPhase {
    /// 已登记，等待下次启动执行
    Scheduled,
    /// 正在复制分区内容
    Copying,
    /// 复制完成，正在校验
    Verifying,
}

impl MigrationPhase {
    /// 阶段是否已进入复制（崩溃恢复诊断用：复制过的目标目录不能当成功看待）
    pub fn has_started_copy(self) -> bool {
        !matches!(self, Self::Scheduled)
    }
}

/// 待执行的根迁移计划
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPlan {
    /// 计划标识（登记时间 + 随机段，展示与日志据此对齐）
    pub id: String,
    /// 源根目录（登记时的生效根）
    pub source: String,
    /// 目标根目录
    pub target: String,
    /// 登记时的布局版本（迁移后写入 `layoutVersion`）
    pub layout_version: i64,
    /// 当前阶段
    pub phase: MigrationPhase,
    /// 登记时间（Unix 毫秒）
    pub created_at: i64,
    /// 启动尝试次数（每次维护阶段执行 +1，用于展示「已尝试 N 次」）
    pub attempts: u32,
    /// 最后一次失败原因（成功提交时随计划一起清除）
    pub last_error: Option<String>,
}

impl PendingPlan {
    /// 新建计划（阶段为 scheduled，未尝试）
    pub fn new(source: &Path, target: &Path, layout_version: i64) -> Self {
        Self {
            id: new_plan_id(),
            source: source.display().to_string(),
            target: target.display().to_string(),
            layout_version,
            phase: MigrationPhase::Scheduled,
            created_at: now_ms(),
            attempts: 0,
            last_error: None,
        }
    }

    /// 源根（计划里记为字符串，解析失败按「源不可用」处理）
    pub fn source_path(&self) -> PathBuf {
        PathBuf::from(&self.source)
    }

    /// 目标根
    pub fn target_path(&self) -> PathBuf {
        PathBuf::from(&self.target)
    }
}

/// 最近一次成功迁移的留档（验收与诊断用；不参与启动决策）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastMigration {
    /// 对应计划标识
    pub id: String,
    /// 源根目录
    pub source: String,
    /// 已提交的新根目录
    pub target: String,
    /// 完成时间（Unix 毫秒）
    pub completed_at: i64,
    /// 已复制文件数
    pub copied_files: u64,
    /// 已复制字节数
    pub copied_bytes: u64,
}

/// 配置读写抽象。
///
/// 生产实现走 `settings.json`（`AppConfig`）；测试实现走临时文件（`FileConfig`），
/// 因此「登记计划 → 启动执行 → 提交新根」的完整链路可以在临时目录里真跑一遍。
pub trait ConfigStore {
    /// 读取 `app.<key>`
    fn read(&self, key: &str) -> Option<Value>;
    /// 写入 `app.<key>`
    fn write(&self, key: &str, value: Value) -> Result<(), String>;
    /// 删除 `app.<key>`
    fn remove(&self, key: &str) -> Result<(), String>;
}

/// 生产配置实现：`settings.json` 的 `app` 对象
pub struct AppConfig<'a>(pub &'a AppHandle);

impl ConfigStore for AppConfig<'_> {
    fn read(&self, key: &str) -> Option<Value> {
        crate::framework::paths::read_setting(self.0, key)
    }

    fn write(&self, key: &str, value: Value) -> Result<(), String> {
        crate::framework::paths::write_setting(self.0, key, value)
    }

    fn remove(&self, key: &str) -> Result<(), String> {
        crate::framework::paths::remove_setting(self.0, key)
    }
}

/// 读取待执行计划（缺字段或阶段值非法时返回 None 并保留原值，不静默清计划）
pub fn load_pending(cfg: &dyn ConfigStore) -> Option<PendingPlan> {
    let value = cfg.read(KEY_PENDING_MIGRATION)?;
    serde_json::from_value::<PendingPlan>(value).ok()
}

/// 写入待执行计划
pub fn save_pending(cfg: &dyn ConfigStore, plan: &PendingPlan) -> Result<(), String> {
    let value = serde_json::to_value(plan).map_err(|e| format!("迁移计划序列化失败: {e}"))?;
    cfg.write(KEY_PENDING_MIGRATION, value)
}

/// 更新计划阶段并落盘（崩溃后据此识别半截状态，不靠猜）
pub fn set_phase(
    cfg: &dyn ConfigStore,
    plan: &mut PendingPlan,
    phase: MigrationPhase,
) -> Result<(), String> {
    plan.phase = phase;
    plan.last_error = None;
    save_pending(cfg, plan)
}

/// 清除待执行计划（提交或取消时调用）
pub fn clear_pending(cfg: &dyn ConfigStore) -> Result<(), String> {
    cfg.remove(KEY_PENDING_MIGRATION)
}

/// 读取最近一次成功迁移留档
pub fn load_last(cfg: &dyn ConfigStore) -> Option<LastMigration> {
    serde_json::from_value::<LastMigration>(cfg.read(KEY_LAST_MIGRATION)?).ok()
}

/// 写入最近一次成功迁移留档
pub fn save_last(cfg: &dyn ConfigStore, last: &LastMigration) -> Result<(), String> {
    let value = serde_json::to_value(last).map_err(|e| format!("迁移留档序列化失败: {e}"))?;
    cfg.write(KEY_LAST_MIGRATION, value)
}

/// 目标目录静态校验（纯函数）：
/// 必须是绝对路径、不能与源相同（按规范化结果比较）、源与目标不能互相嵌套。
///
/// 两级规范化（T02-1）：
/// 1. 词法规范化：小写、统一分隔符、消解 `.`/`..`，不要求路径存在。
/// 2. 最近存在祖先规范化：目标未创建时，先 `canonicalize` 其最近存在的祖先再拼回剩余段，
///    从而识破「同一目录的不同写法」（8.3 短名、junction、符号链接、大小写别名）。
pub fn validate_target(source: &Path, target: &Path) -> Result<PathBuf, String> {
    if !target.is_absolute() {
        return Err("目标目录必须是绝对路径".into());
    }
    let source_norm = normalize(source);
    let target_norm = normalize(target);
    if source_norm == target_norm {
        return Err("目标目录与当前存储目录相同".into());
    }
    if contains_path(&target_norm, &source_norm) || contains_path(&source_norm, &target_norm) {
        return Err("目标目录不能是当前存储目录的子目录或父目录".into());
    }
    // 真实路径比对：能解析到同一真实目录（链接/别名写法）同样拒绝
    if let (Some(src_real), Some(tgt_real)) = (real_path(source), real_path(target)) {
        let src_real_norm = normalize(&src_real);
        let tgt_real_norm = normalize(&tgt_real);
        if src_real_norm == tgt_real_norm
            || contains_path(&tgt_real_norm, &src_real_norm)
            || contains_path(&src_real_norm, &tgt_real_norm)
        {
            return Err("目标目录与当前存储目录指向同一位置（链接或别名写法）".into());
        }
    }
    Ok(target.to_path_buf())
}

/// 解析路径的真实形式：未存在的路径先解析最近存在的祖先，再拼回剩余段
fn real_path(path: &Path) -> Option<PathBuf> {
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        if let Ok(real) = std::fs::canonicalize(&current) {
            let mut out = real;
            for part in suffix.iter().rev() {
                out.push(part);
            }
            return Some(out);
        }
        let name = current.file_name()?.to_os_string();
        suffix.push(name);
        if !current.pop() {
            return None;
        }
    }
}

/// 按路径段判断祖先关系（`c:/data/box` 不是 `c:/data/box2` 的祖先，纯字符串前缀会误判）
fn contains_path(parent: &str, child: &str) -> bool {
    child.len() > parent.len()
        && child.starts_with(parent)
        && child.as_bytes().get(parent.len()) == Some(&b'/')
}

/// 词法规范化：供同路径判定使用（Windows 下大小写不敏感，统一转小写与 `/` 分隔）
pub fn normalize(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::Prefix(prefix) => {
                parts.push(normalize_text(&prefix.as_os_str().to_string_lossy()))
            }
            Component::RootDir => parts.push(String::new()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(name) => parts.push(normalize_text(&name.to_string_lossy())),
        }
    }
    parts.join("/")
}

/// 单段文本规范化：Windows 去大小写差异；去掉 Windows 长路径前缀与 UNC 前导斜杠
fn normalize_text(text: &str) -> String {
    let slashed = text.replace('\\', "/");
    let no_lead = slashed.trim_start_matches('/').trim_start_matches("?/");
    let trimmed = no_lead.trim_end_matches('/');
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// 本次任务的暂存目录名（带随机计划 id：只清理自己创建的目录，不碰用户既有文件）
pub fn staging_dir_name(plan_id: &str) -> String {
    format!(".patchybox-staging-{plan_id}")
}

/// 当前时间（Unix 毫秒）；取不到时钟时返回 0（仅用于展示与排序，不参与判定）
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 生成计划标识（时间戳 + uuid，可作为暂存目录名的安全片段）
fn new_plan_id() -> String {
    format!("{}-{}", now_ms(), uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{temp_dir, FileConfig};
    use super::*;

    #[test]
    fn plan_roundtrip_keeps_phase_and_error() {
        let dir = temp_dir("roundtrip");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let plan = PendingPlan::new(Path::new("C:/old-root"), Path::new("D:/new-root"), 2);
        save_pending(&cfg, &plan).unwrap();

        let loaded = load_pending(&cfg).expect("计划应能读回");
        assert_eq!(loaded, plan);
        assert_eq!(loaded.phase, MigrationPhase::Scheduled);
        assert_eq!(loaded.attempts, 0);

        let mut failed = loaded.clone();
        failed.phase = MigrationPhase::Verifying;
        failed.attempts = 1;
        failed.last_error = Some("校验失败：文件数不一致".into());
        save_pending(&cfg, &failed).unwrap();
        let reloaded = load_pending(&cfg).expect("失败计划应保留");
        assert_eq!(reloaded.phase, MigrationPhase::Verifying);
        assert_eq!(reloaded.attempts, 1);
        assert!(reloaded.last_error.is_some());

        clear_pending(&cfg).unwrap();
        assert!(load_pending(&cfg).is_none());
        // 清除后再读配置不报错（键被真正移除）
        assert!(cfg.read(KEY_PENDING_MIGRATION).is_none());
    }

    #[test]
    fn last_migration_roundtrip() {
        let dir = temp_dir("last");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        assert!(load_last(&cfg).is_none());
        let last = LastMigration {
            id: "p1".into(),
            source: "C:/old".into(),
            target: "D:/new".into(),
            completed_at: 42,
            copied_files: 7,
            copied_bytes: 1024,
        };
        save_last(&cfg, &last).unwrap();
        assert_eq!(load_last(&cfg).unwrap(), last);
    }

    #[test]
    fn validate_target_rejects_same_path_with_case_and_separator_differences() {
        let source = Path::new("C:/Data/Box");
        // 大小写与分隔符不同但指向同一目录
        assert!(validate_target(source, Path::new("c:/data/box")).is_err());
        assert!(validate_target(source, Path::new("C:\\Data\\Box\\")).is_err());
        // 自身子目录 / 父目录都会被递归复制或覆盖，必须拒绝
        assert!(validate_target(source, Path::new("C:/Data/Box/data")).is_err());
        assert!(validate_target(source, Path::new("C:/Data")).is_err());
        // 相对路径拒绝
        assert!(validate_target(source, Path::new("relative/dir")).is_err());
        // 兄弟目录接受
        let ok = validate_target(source, Path::new("C:/Data/Box2")).unwrap();
        assert_eq!(ok, PathBuf::from("C:/Data/Box2"));
    }

    #[test]
    fn validate_target_rejects_dot_segments_pointing_to_source() {
        let source = Path::new("C:/Data/Box");
        assert!(validate_target(source, Path::new("C:/Data/Other/../Box")).is_err());
        assert!(validate_target(source, Path::new("C:/Data/Box/./")).is_err());
    }

    #[test]
    fn plan_id_is_unique_per_call() {
        let a = PendingPlan::new(Path::new("C:/a"), Path::new("D:/b"), 2);
        let b = PendingPlan::new(Path::new("C:/a"), Path::new("D:/b"), 2);
        assert_ne!(a.id, b.id);
    }
}
