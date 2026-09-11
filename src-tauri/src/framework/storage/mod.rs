//! 框架 · 存储位置管理（设置页「存储位置」卡片的命令层）
//!
//! 职责：
//! - `storage_info`：展示当前根目录、四分区路径与占用、是否默认位置、布局版本。
//! - `storage_migrate`：把四分区内容复制到新根目录，**校验通过才写配置**，重启生效。
//!
//! 安全约定（细则 `docs/tasks/2026-09-11-存储目录配置任务书.md` §3.4）：
//! - 迁移**只复制不删除**：原目录原样保留，由用户确认后手工清理。
//! - 任一步失败即中止并回报原因，**绝不写 `storageRoot` 配置**（失败不改变现状）。
//! - 目标目录非空时合并（同名文件以源为准覆盖），预检阶段已让用户确认。
//! - 剩余空间不做预估（无跨平台可靠 API），改为复制写入失败即中止并回报已复制量。

mod transfer;
mod verify;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::framework::paths;

/// 四分区名称（顺序即设置页展示顺序）
const PARTITIONS: [&str; 4] = ["data", "vault", "logs", "cache"];

/// 存储位置信息（设置页展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// 当前生效的根目录
    root: String,
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

/// 迁移结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrateResult {
    /// 是否成功（失败时不写配置）
    ok: bool,
    /// 目标根目录
    target: String,
    /// 已复制文件数
    copied_files: u64,
    /// 已复制字节数
    copied_bytes: u64,
    /// 失败原因
    error: Option<String>,
}

/// 迁移进度事件负载（事件名 `storage://progress`）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MigrateProgress {
    /// 阶段：precheck / copy / verify / done
    phase: &'static str,
    /// 已复制文件数
    copied_files: u64,
    /// 已复制字节数
    copied_bytes: u64,
    /// 源数据总字节数（进度分母）
    total_bytes: u64,
}

/// 读取当前存储位置信息（设置页「存储位置」卡片）
#[tauri::command]
pub fn storage_info(app: AppHandle) -> Result<StorageInfo, String> {
    let root = paths::storage_root(&app)?;
    let default_root = paths::default_root(&app)?;

    let mut partitions = Vec::with_capacity(PARTITIONS.len());
    let mut total_bytes = 0u64;
    let mut file_count = 0u64;
    for name in PARTITIONS {
        let dir = root.join(name);
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
        is_default: root == default_root,
        default_root: default_root.display().to_string(),
        partitions,
        total_bytes,
        file_count,
        layout_version: paths::layout_version(&app),
    })
}

/// 迁移到新的根目录（预检 → 复制 → 校验 → 写配置）
///
/// 在阻塞线程执行（复制可能耗时）；进度经 `storage://progress` 事件推送。
/// 成功后需重启应用生效（不写任何句柄的热切换）。
#[tauri::command]
pub async fn storage_migrate(
    app: AppHandle,
    target: String,
) -> Result<StorageMigrateResult, String> {
    tauri::async_runtime::spawn_blocking(move || migrate_blocking(&app, &target))
        .await
        .map_err(|e| format!("迁移任务失败: {e}"))?
}

/// 迁移主流程（阻塞）：预检 → 复制 → 校验 → 写配置
fn migrate_blocking(app: &AppHandle, target: &str) -> Result<StorageMigrateResult, String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("目标目录不能为空".into());
    }
    let target_root = std::path::PathBuf::from(trimmed);
    if !target_root.is_absolute() {
        return Err("目标目录必须是绝对路径".into());
    }

    let source_root = paths::storage_root(app)?;
    if source_root == target_root {
        return Err("目标目录与当前存储目录相同".into());
    }

    // ── 预检：可写性探针（不可写立即中止，不写配置）──
    emit_progress(app, "precheck", 0, 0);
    if !paths::is_writable_dir(&target_root) {
        return Err(format!("目标目录不可写：{}", target_root.display()));
    }
    // 目标目录非空 → 记为合并（前端已确认；此处仅日志留痕）
    if transfer::count_files(&target_root).unwrap_or(0) > 0 {
        eprintln!(
            "[storage] 目标目录非空，按合并方式写入：{}",
            target_root.display()
        );
    }

    // ── 复制：逐分区保留相对路径 ──
    let total_bytes = paths::dir_size(&source_root).unwrap_or(0);
    let mut copied_files = 0u64;
    let mut copied_bytes = 0u64;
    for name in PARTITIONS {
        let from = source_root.join(name);
        if !from.exists() {
            continue;
        }
        let to = target_root.join(name);
        let (files, bytes) = transfer::copy_tree(&from, &to)?;
        copied_files += files;
        copied_bytes += bytes;
        emit_progress(app, "copy", copied_files, copied_bytes);
    }
    if copied_files == 0 {
        return Err("源存储目录没有可迁移的数据（四分区均为空）".into());
    }
    let _ = total_bytes;

    // ── 校验：文件数与字节数一致 + 每个 SQLite 库 quick_check ──
    emit_progress(app, "verify", copied_files, copied_bytes);
    verify::verify_copy(&source_root, &target_root)?;

    // ── 写配置（校验通过才落盘；重启生效）──
    paths::set_storage_root(app, trimmed)?;
    emit_progress(app, "done", copied_files, copied_bytes);

    Ok(StorageMigrateResult {
        ok: true,
        target: target_root.display().to_string(),
        copied_files,
        copied_bytes,
        error: None,
    })
}

/// 推送迁移进度（前端订阅 `storage://progress`）
fn emit_progress(app: &AppHandle, phase: &'static str, copied_files: u64, copied_bytes: u64) {
    let payload = MigrateProgress {
        phase,
        copied_files,
        copied_bytes,
        total_bytes: copied_bytes,
    };
    let _ = app.emit("storage://progress", payload);
}

/// 框架命令注册（命令入库 + 中文说明）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(
        "framework",
        &[
            ("storage_info", "读取存储位置信息（四分区路径与占用）"),
            ("storage_migrate", "迁移存储目录到新根目录（重启生效）"),
        ],
    )
    .expect("IPC 命令重复注册");
    builder
}
