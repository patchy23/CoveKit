//! 框架 · 存储位置管理（设置页「存储位置」卡片的命令层）
//!
//! 职责：
//! - `storage_info`：展示当前根目录、四分区路径与占用、待执行迁移计划、恢复状态。
//! - `storage_schedule_migration`：**只登记**迁移计划（复制与校验在下次启动的维护阶段执行）。
//! - `storage_cancel_migration`：取消未执行的计划（不修改任何业务文件）。
//! - `storage_recovery_status` / `storage_recovery_action`：配置盘不可用或迁移失败时的可见恢复入口。
//!
//! 语义与安全约定（任务书 T01）：
//! - 生效根在运行期**不切换**：登记计划或选择恢复动作后一律需要重启，重启时在维护阶段
//!   （业务资源初始化之前）执行复制与校验，校验通过才提交 `storageRoot`。
//! - 迁移**只复制不删除**：原目录原样保留，由用户确认后手工清理。
//! - 任一步失败即中止并回报原因，**绝不写 `storageRoot`**，计划里保留源、目标、阶段与错误。
//! - 同一时间只接受一个待执行计划（第二次请求被拒绝），避免两个计划互相覆盖。
//! - 配置盘不可用**不静默退回默认目录**：生效根保持配置值并进入可见恢复状态。
//! - 剩余空间不做预估（无跨平台可靠 API），改为复制写入失败即中止并回报已复制量。

pub(crate) mod plan;
pub(crate) mod recovery;

pub mod layout;
mod migrate;
mod scan;
mod transfer;
mod verify;

#[cfg(test)]
mod test_support {
    //!  存储相关测试替身（仅测试编译）：临时目录与文件型配置实现。
    //!
    //! 用途：让「登记计划 → 启动维护阶段执行 → 提交新根」的完整链路在临时目录里真跑一遍，
    //! 不触碰真实数据目录（安全约定：故障注入只在临时夹具）。

    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use serde_json::Value;

    use super::plan::ConfigStore;

    /// 建立本测试专用临时目录（先清空，保证夹具干净）
    pub fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchybox-storage-{tag}-{}-{}",
            std::process::id(),
            super::plan::now_ms()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 测试用配置实现：单文件 JSON 的 `app` 对象（键删除后不留 null）
    ///
    /// `fail_writes` 用于模拟「配置保存失败」：数据已复制但新根写不进去。
    pub struct FileConfig {
        /// 配置文件路径
        path: PathBuf,
        /// 只让指定键的写入失败（None = 不注入故障）；
        /// 用「单键失败」而不是「全部写入失败」，才能精确模拟「新根配置写不进去」这类可恢复故障
        fail_key: Mutex<Option<String>>,
        /// 保证同一文件上的读改写串行
        lock: Mutex<()>,
    }

    impl FileConfig {
        /// 新建（文件不存在视为空配置）
        pub fn new(path: &Path) -> Self {
            Self {
                path: path.to_path_buf(),
                fail_key: Mutex::new(None),
                lock: Mutex::new(()),
            }
        }

        /// 只让某个配置键的写入失败（None = 取消）。用于精确模拟「新根配置写不进去」。
        pub fn fail_key(&self, key: Option<&str>) {
            let mut guard = match self.fail_key.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = key.map(|k| k.to_string());
        }

        /// 该键当前是否应拒绝写入
        fn rejects(&self, key: &str) -> bool {
            match self.fail_key.lock() {
                Ok(guard) => guard.as_deref() == Some(key),
                Err(poisoned) => poisoned.into_inner().as_deref() == Some(key),
            }
        }

        /// 读取整份 JSON（缺失或损坏时返回空对象）
        fn load(&self) -> serde_json::Map<String, Value> {
            let raw = std::fs::read_to_string(&self.path).unwrap_or_default();
            let parsed: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            parsed
                .get("app")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default()
        }
    }

    impl ConfigStore for FileConfig {
        fn read(&self, key: &str) -> Option<Value> {
            self.load().get(key).cloned()
        }

        fn write(&self, key: &str, value: Value) -> Result<(), String> {
            if self.rejects(key) {
                return Err("模拟写入失败".into());
            }
            let _guard = self.lock.lock().map_err(|e| format!("锁失败: {e}"))?;
            let mut app = self.load();
            app.insert(key.to_string(), value);
            let doc = serde_json::json!({ "app": Value::Object(app) });
            std::fs::write(
                &self.path,
                serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }

        fn remove(&self, key: &str) -> Result<(), String> {
            if self.rejects(key) {
                return Err("模拟删除失败".into());
            }
            let _guard = self.lock.lock().map_err(|e| format!("锁失败: {e}"))?;
            let mut app = self.load();
            app.remove(key);
            let doc = serde_json::json!({ "app": Value::Object(app) });
            std::fs::write(
                &self.path,
                serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }
    }
}

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::framework::{context, paths, space};

/// 四分区名称（顺序即设置页展示顺序）
const PARTITIONS: [&str; 4] = ["data", "vault", "logs", "cache"];

/// 存储位置信息（设置页展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// 当前生效的根目录（设备级根）
    root: String,
    /// 落盘形态：`legacyFlat`（旧扁平，默认空间保持此形态）或 `partitioned`（按空间分区）
    layout: String,
    /// 是否使用默认根目录（app_data_dir）
    is_default: bool,
    /// 默认根目录（「恢复默认」按钮的目标值）
    default_root: String,
    /// 四分区明细
    partitions: Vec<PartitionInfo>,
    /// 四分区合计占用字节数
    total_bytes: u64,
    /// 四分区合计文件数
    file_count: u64,
    /// 已完成的布局版本（paths::LAYOUT_VERSION 表示已是四分区布局）
    layout_version: i64,
    /// 非秘密空间标识（数据上下文；默认空间在导入导出 L0/L1 交付前恒为 default）
    space_id: String,
    /// 空间代际（导入激活/空间切换后递增；用于判定计划是否过期）
    generation_id: u64,
    /// 空间回落登记（活动空间标识非法时回落默认空间的原因；None = 正常）
    space_fallback: Option<space::SpaceFallback>,
    /// 待执行的迁移计划（重启后由维护阶段执行）
    pending_migration: Option<plan::PendingPlan>,
    /// 最近一次成功迁移的留档（诊断用）
    last_migration: Option<plan::LastMigration>,
    /// 恢复状态（配置盘不可用或迁移失败；前端据此显示恢复页）
    recovery: Option<recovery::StorageRecovery>,
}

/// 单个分区的路径与占用
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionInfo {
    /// 分区名（data / vault / logs / cache）
    name: String,
    /// 绝对路径
    path: String,
    /// 占用字节数
    bytes: u64,
    /// 文件数
    file_count: u64,
}

/// 安排迁移的结果（不复制任何文件，重启后执行）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageScheduleResult {
    /// 是否登记成功
    ok: bool,
    /// 计划标识（用于展示与日志对齐）
    plan_id: String,
    /// 目标根目录
    target: String,
    /// 源根目录（计划登记时的生效根）
    source: String,
    /// 计划生效时机说明
    message: String,
}

/// 恢复动作结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryActionResult {
    /// 动作是否被接受
    ok: bool,
    /// 是否需要重启才能生效（恢复动作一律不热切换）
    restart_required: bool,
    /// 面向用户的说明
    message: String,
}

/// 迁移进度事件负载（事件名 `storage://progress`）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrateProgress {
    /// 阶段：copy / verify / done
    phase: &'static str,
    /// 已复制文件数
    copied_files: u64,
    /// 已复制字节数
    copied_bytes: u64,
    /// 源数据总字节数（进度分母）
    total_bytes: u64,
}

/// 推送迁移进度（前端订阅 `storage://progress`）
pub(crate) fn emit_progress(
    app: &AppHandle,
    phase: &'static str,
    copied_files: u64,
    copied_bytes: u64,
    total_bytes: u64,
) {
    let payload = MigrateProgress {
        phase,
        copied_files,
        copied_bytes,
        total_bytes,
    };
    let _ = app.emit("storage://progress", payload);
}

/// 维护阶段入口：执行待执行的根迁移计划（由 `lib.rs` 在任何业务初始化之前调用）
pub(crate) fn run_pending(app: &AppHandle, configured_root: &str) -> migrate::MigrationOutcome {
    migrate::run_pending(app, configured_root)
}

/// 读取当前存储位置信息（设置页「存储位置」卡片）
#[tauri::command]
pub fn storage_info(app: AppHandle) -> Result<StorageInfo, String> {
    let root = paths::storage_root(&app)?;
    let default_root = paths::default_root(&app)?;
    let cfg = plan::AppConfig(&app);
    // 分区明细与落盘取值同源：都来自本次生效的位置描述符
    let location = paths::current_location(&app)?;

    let mut partitions = Vec::with_capacity(PARTITIONS.len());
    let mut total_bytes = 0u64;
    let mut file_count = 0u64;
    for name in PARTITIONS {
        let dir =
            paths::partition_path(&location, name).unwrap_or_else(|| location.root.join(name));
        let bytes = paths::dir_size(&dir).unwrap_or(0);
        let files = transfer::count_files(&dir).unwrap_or(0);
        total_bytes += bytes;
        file_count += files;
        partitions.push(PartitionInfo {
            name: name.to_string(),
            path: dir.display().to_string(),
            bytes,
            file_count: files,
        });
    }

    Ok(StorageInfo {
        root: root.display().to_string(),
        layout: location.layout.as_str().to_string(),
        is_default: root == default_root,
        default_root: default_root.display().to_string(),
        partitions,
        total_bytes,
        file_count,
        layout_version: paths::layout_version(&app),
        space_id: context::current()
            .map(|ctx| ctx.space_id().to_string())
            .unwrap_or_else(|| context::DEFAULT_SPACE_ID.to_string()),
        generation_id: context::current()
            .map(context::DataContext::generation_id)
            .unwrap_or(context::DEFAULT_GENERATION_ID),
        space_fallback: space::fallback(),
        pending_migration: plan::load_pending(&cfg),
        last_migration: plan::load_last(&cfg),
        recovery: recovery::current(),
    })
}

/// 安排一次根迁移（**不复制文件**：只做静态校验并登记计划，重启后由维护阶段执行）
///
/// 同一时间只接受一个计划：已有未执行计划时返回错误，避免互相覆盖。
#[tauri::command]
pub async fn storage_schedule_migration(
    app: AppHandle,
    target: String,
) -> Result<StorageScheduleResult, String> {
    // 维护互斥：根迁移与导入提交/空间激活/更新安装共用同一把锁（前端禁用按钮不算锁）
    let _maintenance = context::maintenance_guard().await;
    let cfg = plan::AppConfig(&app);

    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("目标目录不能为空".into());
    }
    let target_root = std::path::PathBuf::from(trimmed);
    let source_root = paths::storage_root(&app)?;
    // 静态校验：绝对路径、不与源相同（含大小写/分隔符差异）、不互相嵌套
    let target_root = plan::validate_target(&source_root, &target_root)?;

    if let Some(existing) = plan::load_pending(&cfg) {
        return Err(format!(
            "已有待执行的迁移计划（计划 {}，目标 {}）。请先取消该计划，或重启应用完成迁移后再安排新的。",
            existing.id, existing.target
        ));
    }
    // 目标可写性预检：不可写立即拒绝（不登记必然失败的计划）
    if !paths::is_writable_dir(&target_root) {
        return Err(format!("目标目录不可写：{}", target_root.display()));
    }

    // 先生成计划标识：非空判定要放行「本计划自己的暂存目录」（上次中断留下的）
    let pending = plan::PendingPlan::new(&source_root, &target_root, paths::LAYOUT_VERSION);

    // 非空目标默认拒绝（T02-3）：不自动覆盖、不做隐式合并；
    // 恢复默认位置若已有旧数据同样在这里被拦下，由用户改选空目录。
    // 唯一例外：目标只有本计划自己的暂存目录（上次中断），重启后由维护阶段按续跑路径处理。
    if !verify::only_own_staging(&target_root, &plan::staging_dir_name(&pending.id))? {
        let existing = verify::existing_partitions(&target_root);
        return Err(format!(
            "目标目录不是空目录（已存在分区：{}），迁移不会覆盖或合并已有数据。请选择空目录，或先清理目标后重试。",
            existing.join(", ")
        ));
    }

    plan::save_pending(&cfg, &pending)?;
    let message = format!(
        "已登记迁移计划：重启应用后在新位置建立数据（复制并校验，完成后自动切换）。当前仍使用原目录 {}。",
        source_root.display()
    );
    Ok(StorageScheduleResult {
        ok: true,
        plan_id: pending.id,
        target: target_root.display().to_string(),
        source: source_root.display().to_string(),
        message,
    })
}

/// 取消未执行的迁移计划（不修改任何业务文件）
#[tauri::command]
pub fn storage_cancel_migration(app: AppHandle) -> Result<bool, String> {
    let cfg = plan::AppConfig(&app);
    if plan::load_pending(&cfg).is_none() {
        return Ok(false);
    }
    plan::clear_pending(&cfg)?;
    Ok(true)
}

/// 读取当前恢复状态（None = 正常；前端启动时轮询一次）
#[tauri::command]
pub fn storage_recovery_status() -> Option<recovery::StorageRecovery> {
    recovery::current()
}

/// 执行一个恢复动作：retry（重试探测）/ use-default（改用默认数据环境）/ choose（选择新数据环境）
///
/// 三个动作都**不热切换**：需要重启才生效（恢复动作改变的是下次启动的解析结果）。
#[tauri::command]
pub async fn storage_recovery_action(
    app: AppHandle,
    action: String,
    target: Option<String>,
) -> Result<RecoveryActionResult, String> {
    let _maintenance = context::maintenance_guard().await;
    let cfg = plan::AppConfig(&app);
    let state = recovery::current();

    match action.as_str() {
        "retry" => {
            let Some(state) = state else {
                return Ok(RecoveryActionResult {
                    ok: true,
                    restart_required: false,
                    message: "当前没有待处理的存储故障".into(),
                });
            };
            match state.reason {
                recovery::RecoveryReason::ConfiguredRootUnavailable => {
                    // 显式重试：由用户触发，不做任何自动换根
                    let root = std::path::PathBuf::from(&state.configured_root);
                    if !paths::is_writable_dir(&root) {
                        return Err(format!("存储目录仍不可用：{}", root.display()));
                    }
                    recovery::clear();
                    Ok(RecoveryActionResult {
                        ok: true,
                        restart_required: true,
                        message: "存储目录已恢复可用，请重启应用以完成初始化。".into(),
                    })
                }
                recovery::RecoveryReason::MigrationFailed => {
                    // 计划仍在：重启后维护阶段会再试一次（失败原因已保留在计划里）
                    recovery::clear();
                    Ok(RecoveryActionResult {
                        ok: true,
                        restart_required: true,
                        message: "迁移计划已保留，重启应用后将重新执行迁移。".into(),
                    })
                }
            }
        }
        "use-default" => {
            if let Some(state) = &state {
                if !state.can_use_default {
                    return Err("当前故障不允许改用默认数据环境，请先处理迁移问题".into());
                }
            }
            let previous = state
                .as_ref()
                .map(|s| s.configured_root.clone())
                .unwrap_or_else(|| {
                    paths::storage_root(&app)
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                });
            // 清空配置根 = 下次启动使用默认目录；原目录文件一个都不动
            paths::set_storage_root(&app, "")?;
            recovery::clear();
            Ok(RecoveryActionResult {
                ok: true,
                restart_required: true,
                message: format!(
                    "已改为使用默认数据环境，请重启应用。原目录 {previous} 中的数据未被删除或修改。"
                ),
            })
        }
        "choose" => {
            let Some(trimmed) = target.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
                return Err("选择新数据环境时必须提供目标目录".into());
            };
            if plan::load_pending(&cfg).is_some() {
                return Err("已有待执行的迁移计划，请先取消或重启完成迁移".into());
            }
            let source_root = paths::storage_root(&app)?;
            let target_root =
                plan::validate_target(&source_root, &std::path::PathBuf::from(trimmed))?;
            if !paths::is_writable_dir(&target_root) {
                return Err(format!("目标目录不可写：{}", target_root.display()));
            }
            let pending = plan::PendingPlan::new(&source_root, &target_root, paths::LAYOUT_VERSION);
            plan::save_pending(&cfg, &pending)?;
            recovery::clear();
            Ok(RecoveryActionResult {
                ok: true,
                restart_required: true,
                message: format!(
                    "已登记迁移计划：重启应用后把 {} 的数据复制到 {} 并切换。",
                    source_root.display(),
                    target_root.display()
                ),
            })
        }
        other => Err(format!("未知的恢复动作: {other}")),
    }
}
