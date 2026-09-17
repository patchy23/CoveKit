//! 导入导出命令层（sync-202609-001 L2 §5.3 的 8 条冻结命令）
//!
//! 三条口径：
//! - 密码只在单次命令调用内存在：不写设置、不进日志、不缓存（提交时再次输入）。
//! - 明文不进 JS：解密、校验、生成、报告全在 Rust 侧；返回给界面的只有计数与名称。
//! - 每个失败路径都返回可读原因；成功返回可展示的报告（不出现「点了没反应」）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::framework::context::maintenance_guard;
use crate::framework::settings::settings_patch;
use crate::framework::space::{self, index as space_index};
use crate::framework::tasks;

use super::catalog;
use super::import;
use super::package;
use super::session;
use super::types::*;

/// 传输任务的任务类型（任务面板显示用）
const TASK_KIND: &str = "data-transfer";
/// 导出任务标签
const TASK_LABEL_EXPORT: &str = "导出数据包";
/// 导入任务标签
const TASK_LABEL_IMPORT: &str = "导入数据包";

/// 空间摘要（`data_spaces_list` 条目；只含非秘密信息，绝不含路径与凭证）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    /// 空间 id
    pub space_id: String,
    /// 展示名
    pub name: String,
    /// 创建（登记）时间
    pub created_at: String,
    /// 是否是当前活动空间
    pub active: bool,
    /// 是否由导入创建
    pub imported: bool,
    /// 来源空间名（不可信，仅展示）
    pub source_space_name: Option<String>,
    /// 导入时间
    pub imported_at: Option<String>,
    /// 各类别导入条数
    pub counts: BTreeMap<String, usize>,
}

/// 切换活动空间的结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSwitchResult {
    /// 切换到的空间 id
    pub space_id: String,
    /// 切换到的空间名
    pub name: String,
    /// 是否必须重启才生效（本批一律为真：不热切换）
    pub restart_required: bool,
    /// 写入后的设置版本号（界面回传用于下次保存）
    pub revision: u64,
}

/// 导出报告
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// 生成的文件路径
    pub path: String,
    /// 文件大小（字节）
    pub bytes: u64,
    /// 包标识（再次导出同内容时便于区分）
    pub package_id: String,
    /// 来源空间名（写进包内，供导入侧展示）
    pub source_space_name: String,
    /// 数据集名 → 进包条数
    pub counts: BTreeMap<String, usize>,
    /// 未进包的数据集（含只声明未携带的凭证块）
    pub excluded: Vec<String>,
    /// 是否携带了秘密（界面据此提示「包内含有凭证」）
    pub secret_included: bool,
}

/// 传输启动结果（导出与导入共用）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransferStart {
    /// 任务 id（界面在任务面板里定位这次传输）
    pub task_id: String,
    /// 导出报告（导入的 `data_import_commit` 用 `ImportCommitResult`）
    pub report: ExportReport,
}

/// 包内一个数据集的展示视图（不含记录体）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageDatasetView {
    /// 数据集名
    pub name: String,
    /// 本应用给它的展示名（来自适配器目录；不认识的数据集为空）
    pub label: String,
    /// 传输策略
    pub policy: TransportPolicy,
    /// 包内声明的 schema 版本
    pub schema_version: u32,
    /// 包内声明的条数
    pub record_count: usize,
    /// 是否携带了记录体（未携带 = 只声明）
    pub carried: bool,
    /// 本应用能否导入（未知 owner 或版本过高为否）
    pub supported: bool,
    /// 不支持的原因
    pub reason: Option<String>,
}

/// 包摘要（inspect 结果里的展示部分）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummaryView {
    /// 包标识
    pub package_id: String,
    /// 来源空间 id（不可信）
    pub source_space_id: String,
    /// 来源空间名（不可信）
    pub source_space_name: String,
    /// 导出时间（不可信）
    pub created_at: String,
    /// 导出时的应用版本（不可信）
    pub app_version: String,
    /// 导出时的平台（不可信）
    pub platform: String,
    /// 数据集视图
    pub datasets: Vec<PackageDatasetView>,
    /// 导出侧显式排除的项
    pub excluded: Vec<String>,
}

/// 二次导入提示（同一 packageId 已导入过）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateHint {
    /// 已导入到的空间 id
    pub space_id: String,
    /// 已导入到的空间名
    pub space_name: String,
    /// 导入时间
    pub imported_at: String,
}

/// inspect 结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportInspectResult {
    /// 预览上下文 id（规划与提交都要带上）
    pub inspect_id: String,
    /// 包摘要
    pub summary: PackageSummaryView,
    /// 默认选择（界面据此预勾选）
    pub defaults: ImportSelection,
    /// 逐条判定预览（新增 / 待补全 / 被排除）
    pub preview: Vec<ImportPlanItem>,
    /// 待补全说明
    pub pending: Vec<String>,
    /// 本次会排除的数据集
    pub excluded: Vec<String>,
    /// 二次导入提示
    pub duplicate: Option<DuplicateHint>,
}

/// 单条冲突的用户决策（合并导入）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictChoice {
    /// 数据集名
    pub dataset: String,
    /// 包内（来源）记录 id
    pub source_id: String,
    /// 处置（保留本地 / 采用导入 / 保留两份）
    pub decision: ConflictDecision,
}

/// plan 结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanResult {
    /// 计划 id（提交时回传）
    pub plan_id: String,
    /// 导入模式（newSpace / merge / overwrite；合并与覆盖导入进当前空间）
    pub mode: String,
    /// 目标空间存储修订号（合并/覆盖：提交时回传复核，D7）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
    /// 新空间 id（已定，提交后即成为空间目录名）
    pub space_id: String,
    /// 新空间展示名
    pub space_name: String,
    /// 逐条判定预览
    pub preview: Vec<ImportPlanItem>,
    /// 待补全说明
    pub pending: Vec<String>,
    /// 会被排除的数据集
    pub excluded: Vec<String>,
    /// 无密码时也能展示的条数对账（数据集名 → 条数）
    pub counts: BTreeMap<String, usize>,
}

/// 提交结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitResult {
    /// 任务 id
    pub task_id: String,
    /// 导入报告（新空间、条数、待补全、排除项）
    pub report: ImportReport,
}

/// 取消结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    /// 是否确有在跑的传输被请求取消
    pub cancelled: bool,
}

/// 列出本机空间：索引条目 + 兼容承载位默认空间（旧安装没有索引也要看得到默认空间）
#[tauri::command]
pub fn data_spaces_list(app: AppHandle) -> Result<Vec<SpaceSummary>, String> {
    let index = space_index::read_index(&app)?;
    let active = space_index::read_active_id(&app)
        .or_else(|| space::current_id().ok())
        .unwrap_or_default();
    let mut list = Vec::with_capacity(index.len());
    for (space_id, record) in &index {
        let source = record.imported_from.as_ref();
        // 默认空间在索引里有条目且未命名时用固定文案（与 space::display_name 同一口径）
        let name = if record.is_default && record.name.trim().is_empty() {
            space_index::DEFAULT_SPACE_NAME.to_string()
        } else {
            record.display_name(space_id)
        };
        list.push(SpaceSummary {
            space_id: space_id.clone(),
            name,
            created_at: record.created_at.clone(),
            active: &active == space_id,
            imported: source.is_some(),
            source_space_name: source.map(|item| item.source_space_name.clone()),
            imported_at: source.map(|item| item.imported_at.clone()),
            counts: source.map(|item| item.counts.clone()).unwrap_or_default(),
        });
    }
    // 活动空间排最前，其余按时间倒序（新建/导入的空间更容易被找到）
    list.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
    Ok(list)
}

/// 切换活动空间：只写设备级指针，重启后由启动解析生效（不热切换）
#[tauri::command]
pub async fn data_space_switch(
    app: AppHandle,
    space_id: String,
    revision: Option<u64>,
) -> Result<SpaceSwitchResult, String> {
    if !space::is_valid_space_id(&space_id) {
        return Err(format!("空间 id 非法：{space_id}"));
    }
    let index = space_index::read_index(&app)?;
    if !index.contains_key(&space_id) {
        return Err(format!("空间不存在或尚未登记：{space_id}"));
    }
    let name = space_index::display_name(&app, &space_id);
    // 维护互斥：空间切换与导入提交、根迁移互斥（前端禁用按钮不算锁）
    let _guard = maintenance_guard().await;
    let mut patch = serde_json::Map::new();
    patch.insert(
        space::KEY_ACTIVE_SPACE_ID.to_string(),
        serde_json::Value::String(space_id.clone()),
    );
    let next = settings_patch(app, revision, patch)?;
    Ok(SpaceSwitchResult {
        space_id,
        name,
        restart_required: true,
        revision: next,
    })
}

/// 当前空间可导出集合摘要与依赖关系
#[tauri::command]
pub fn data_export_catalog(app: AppHandle) -> Result<ExportCatalog, String> {
    catalog::build_catalog(&app)
}

/// 取消正在进行的导出 / 导入
#[tauri::command]
pub fn data_transfer_cancel() -> CancelResult {
    CancelResult {
        cancelled: session::cancel_current(),
    }
}

/// 生成数据包（写入用户选定路径）；密码只在本次调用内存在
#[tauri::command]
pub async fn data_export_start(
    app: AppHandle,
    selection: ExportSelection,
    password: String,
    path: String,
) -> Result<TransferStart, String> {
    package::validate_password(&password)?;
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("请选择数据包保存位置".into());
    }
    let handle = tasks::begin(
        Some(&app),
        "framework",
        TASK_KIND,
        true,
        Some(TASK_LABEL_EXPORT),
    );
    if handle.is_rejected() {
        return Err("同时进行的任务过多，请稍后再试".into());
    }
    let cancel = session::begin_transfer();
    let build_app = app.clone();
    let work = tauri::async_runtime::spawn_blocking(move || -> Result<ExportReport, String> {
        let manifest = catalog::build_manifest(&build_app, &selection)?;
        cancel.check()?;
        let counts = dataset_counts(&manifest);
        let excluded = manifest.excluded.clone();
        let secret_included = manifest
            .datasets
            .iter()
            .any(|block| block.policy == TransportPolicy::Secret && block.records.is_some());
        let report = ExportReport {
            path: target.display().to_string(),
            bytes: 0,
            package_id: manifest.package_id.clone(),
            source_space_name: manifest.source_space_name.clone(),
            counts,
            excluded,
            secret_included,
        };
        let content = package::seal_package(&password, &manifest)?;
        cancel.check()?;
        package::write_package(&target, &content)?;
        Ok(ExportReport {
            bytes: content.len() as u64,
            ..report
        })
    })
    .await
    .map_err(|e| format!("导出任务失败: {e}"))?;
    session::end_transfer();
    match work {
        Ok(report) => {
            handle.succeed(Some(&app));
            Ok(TransferStart {
                task_id: handle.id().to_string(),
                report,
            })
        }
        Err(error) => {
            handle.fail(Some(&app), "data-transfer-failed", &error);
            Err(error)
        }
    }
}

/// 校验数据包：解密 + 清单校验 + 产出预览（不写任何业务数据）
#[tauri::command]
pub async fn data_import_inspect(
    app: AppHandle,
    path: String,
    password: String,
) -> Result<ImportInspectResult, String> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("请选择数据包文件".into());
    }
    let cancel = session::begin_transfer();
    let inspect_app = app.clone();
    let work =
        tauri::async_runtime::spawn_blocking(move || -> Result<ImportInspectResult, String> {
            let raw = package::read_package(&target)?;
            cancel.check()?;
            let digest = package::file_digest(&raw);
            let manifest = package::open_package(&password, &raw)?;
            let descriptors = catalog::collect_descriptors(&inspect_app)?;
            let summary = package_summary(&manifest, &descriptors);
            let defaults = default_import_selection(&manifest, &descriptors);
            let probe = import::build_plan(
                &manifest,
                &descriptors,
                &defaults,
                &uuid::Uuid::new_v4().to_string(),
                "预览",
                &import::new_plan_id(),
                None,
            )?;
            let duplicate = find_duplicate(&inspect_app, &manifest.package_id)?;
            let inspect_id = session::put_inspect(target.clone(), digest, manifest)?;
            // 先取走借用 `probe` 的派生数据，再移动 items（避免部分移动后被借用）
            let pending = probe.pending_notes();
            let excluded = probe.excluded;
            Ok(ImportInspectResult {
                inspect_id,
                summary,
                defaults,
                preview: probe.items,
                pending,
                excluded,
                duplicate,
            })
        })
        .await
        .map_err(|e| format!("校验任务失败: {e}"))?;
    session::end_transfer();
    work
}

/// 规划导入：确定目标空间与要写入的记录（不写任何业务数据）
///
/// 模式（L3）：`newSpace`（默认）进新空间；`merge` / `overwrite` 进**当前空间**——
/// 后者按映射表 + 当前空间现状做逐条判定，并记下存储修订号供提交时复核（D7）。
#[tauri::command]
pub fn data_import_plan(
    app: AppHandle,
    inspect_id: String,
    selection: ImportSelection,
    new_space_name: String,
    allow_duplicate: Option<bool>,
    mode: Option<String>,
    conflicts: Option<Vec<ConflictChoice>>,
) -> Result<ImportPlanResult, String> {
    let mode = parse_import_mode(mode.as_deref())?;
    let (file_path, file_digest, manifest) = session::inspect(&inspect_id)?;
    let descriptors = catalog::collect_descriptors(&app)?;

    let plan = match mode {
        ImportMode::NewSpace => {
            let name = new_space_name.trim();
            if name.is_empty() {
                return Err("请为新空间命名".into());
            }
            if !allow_duplicate.unwrap_or(false) {
                if let Some(hint) = find_duplicate(&app, &manifest.package_id)? {
                    return Err(format!(
                        "该数据包已于 {} 导入到空间「{}」；确认要再导入一份时请显式选择",
                        hint.imported_at, hint.space_name
                    ));
                }
            }
            let space_id = uuid::Uuid::new_v4().to_string();
            import::build_plan(
                &manifest,
                &descriptors,
                &selection,
                &space_id,
                name,
                &import::new_plan_id(),
                None,
            )?
        }
        ImportMode::Merge | ImportMode::Overwrite => {
            // 合并/覆盖：目标是当前空间；重复导入由映射表幂等承接，不做「同包拦截」
            let ctx = crate::framework::context::current().ok_or("数据上下文未初始化")?;
            let space_id = ctx.space_id().to_string();
            let space_name = space_index::display_name(&app, &space_id);
            let decisions: BTreeMap<(String, String), ConflictDecision> = conflicts
                .unwrap_or_default()
                .into_iter()
                .map(|choice| ((choice.dataset, choice.source_id), choice.decision))
                .collect();
            let mut plan = import::build_plan(
                &manifest,
                &descriptors,
                &selection,
                &space_id,
                &space_name,
                &import::new_plan_id(),
                Some(import::MergeInput {
                    mode,
                    decisions: &decisions,
                    app: &app,
                }),
            )?;
            let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
            plan.expected_revision = Some(super::merge::storage_revision(device_root, &space_id)?);
            plan
        }
    };
    let result = ImportPlanResult {
        plan_id: plan.plan_id.clone(),
        mode: plan_mode_name(plan.mode).to_string(),
        expected_revision: plan.expected_revision.clone(),
        space_id: plan.space_id.clone(),
        space_name: plan.space_name.clone(),
        preview: plan.items.clone(),
        pending: plan.pending_notes(),
        excluded: plan.excluded.clone(),
        counts: plan.declared_counts.clone(),
    };
    session::put_plan(plan, file_path, file_digest)?;
    Ok(result)
}

/// 解析导入模式参数（缺省 = 隔离导入；不认识的值直接拒绝，不猜）
fn parse_import_mode(raw: Option<&str>) -> Result<ImportMode, String> {
    match raw {
        None | Some("newSpace") => Ok(ImportMode::NewSpace),
        Some("merge") => Ok(ImportMode::Merge),
        Some("overwrite") => Ok(ImportMode::Overwrite),
        Some(other) => Err(format!("不认识的导入模式 {other}")),
    }
}

/// 导入模式的前端契约名
fn plan_mode_name(mode: ImportMode) -> &'static str {
    match mode {
        ImportMode::NewSpace => "newSpace",
        ImportMode::Merge => "merge",
        ImportMode::Overwrite => "overwrite",
    }
}

/// 提交导入：复核文件与密码 → 写暂存目录 → 搬移成新空间 → 写索引
#[tauri::command]
pub async fn data_import_commit(
    app: AppHandle,
    plan_id: String,
    password: String,
) -> Result<ImportCommitResult, String> {
    package::validate_password(&password)?;
    let (plan, file_path, file_digest) = session::plan(&plan_id)?;
    let handle = tasks::begin(
        Some(&app),
        "framework",
        TASK_KIND,
        true,
        Some(TASK_LABEL_IMPORT),
    );
    if handle.is_rejected() {
        return Err("同时进行的任务过多，请稍后再试".into());
    }
    let cancel = session::begin_transfer();
    let commit_app = app.clone();
    let work = {
        // 维护互斥：导入提交与根迁移、空间切换互斥
        let _guard = maintenance_guard().await;
        tauri::async_runtime::spawn_blocking(move || -> Result<ImportReport, String> {
            cancel.check()?;
            // 复核：文件没被换过、密码仍能解开同一份包（不缓存密码，这里重新解一次）
            let raw = package::read_package(&file_path)?;
            if package::file_digest(&raw) != file_digest {
                return Err("数据包内容已改变，请重新预览后再提交".into());
            }
            let manifest = package::open_package(&password, &raw)?;
            if manifest.package_id != plan.package_id {
                return Err("数据包与预览的不是同一份，请重新预览后再提交".into());
            }
            match plan.mode {
                ImportMode::NewSpace => {
                    let device_root = crate::framework::paths::storage_root(&commit_app)?;
                    import::commit(&commit_app, &device_root, &plan)
                }
                ImportMode::Merge | ImportMode::Overwrite => {
                    // 修订号复核（D7）：预览之后本地数据有变化 → 拒绝，回到预览重新确认
                    let device_root =
                        crate::framework::context::root().ok_or("数据上下文未初始化")?;
                    let current = super::merge::storage_revision(device_root, &plan.space_id)?;
                    if plan.expected_revision.as_deref() != Some(current.as_str()) {
                        return Err("预览之后本地数据有变化，请重新预览后再提交".into());
                    }
                    super::merge::commit(&commit_app, &plan)
                }
            }
        })
        .await
        .map_err(|e| format!("导入任务失败: {e}"))?
    };
    session::end_transfer();
    match work {
        Ok(report) => {
            session::drop_plan(&plan_id);
            handle.succeed(Some(&app));
            Ok(ImportCommitResult {
                task_id: handle.id().to_string(),
                report,
            })
        }
        Err(error) => {
            handle.fail(Some(&app), "data-transfer-failed", &error);
            Err(error)
        }
    }
}

/// 快照条目（设置页「还原到导入前」列表用；只含非秘密信息）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    /// 快照目录名（还原时回传）
    pub dir: String,
    /// 快照所属空间 uid
    pub space_id: String,
    /// 拍摄时间（RFC3339）
    pub created_at: String,
    /// 覆盖的存储文件（空间根相对路径）
    pub files: Vec<String>,
}

/// 列出当前空间的导入前快照（新的在前）
#[tauri::command]
pub fn data_backup_list(_app: AppHandle) -> Result<Vec<BackupSummary>, String> {
    let ctx = crate::framework::context::current().ok_or("数据上下文未初始化")?;
    let space_id = ctx.space_id().to_string();
    let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
    let snapshots = super::backup::list_snapshots(device_root)?;
    Ok(snapshots
        .into_iter()
        .filter(|(_, manifest)| manifest.space_id == space_id)
        .map(|(dir, manifest)| BackupSummary {
            dir: dir
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            space_id: manifest.space_id,
            created_at: manifest.created_at,
            files: manifest.files,
        })
        .collect())
}

/// 还原到导入前：把快照里的存储写回当前空间（覆盖现状），完成后广播刷新
///
/// 与导入提交同款互斥与写冻结；还原对象必须是当前空间的快照（别的空间的快照拒绝）。
#[tauri::command]
pub async fn data_backup_restore(app: AppHandle, dir: String) -> Result<(), String> {
    let ctx = crate::framework::context::current().ok_or("数据上下文未初始化")?;
    let space_id = ctx.space_id().to_string();
    let _guard = maintenance_guard().await;
    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
        let snapshot_dir = super::backup::backups_dir(device_root).join(&dir);
        // 防目录穿越：目录名不允许含路径分隔符
        if dir.contains(['/', '\\']) || dir.is_empty() {
            return Err("快照目录名非法".into());
        }
        let snapshots = super::backup::list_snapshots(device_root)?;
        let Some((_, manifest)) = snapshots.iter().find(|(path, _)| {
            path.file_name().map(|n| n.to_string_lossy().to_string()) == Some(dir.clone())
        }) else {
            return Err("快照不存在（可能已被清理）".into());
        };
        if manifest.space_id != space_id {
            return Err("该快照属于别的空间，拒绝还原到当前空间".into());
        }
        let _freeze = crate::framework::context::WriteFreezeGuard::begin();
        super::backup::restore_snapshot(device_root, &snapshot_dir)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("还原任务失败: {e}"))??;
    // 还原成功后广播全量刷新（涉及的数据集无法从快照精确反推，发全量）
    super::emit_space_data_changed(&app_clone, &["*".to_string()]);
    Ok(())
}

/// 包摘要视图：把清单翻译成界面能显示的结构（不含记录体）
fn package_summary(
    manifest: &PackageManifest,
    descriptors: &[DatasetDescriptor],
) -> PackageSummaryView {
    let datasets = manifest
        .datasets
        .iter()
        .map(|block| {
            let descriptor = descriptors.iter().find(|item| item.name == block.name);
            let supported = match descriptor {
                Some(descriptor) => {
                    descriptor.schema_version >= block.schema_version
                        && block.policy != TransportPolicy::DeviceLocal
                }
                None => false,
            };
            let reason = if descriptor.is_none() {
                Some("本应用不认识这个类别（可能来自更新的版本）".to_string())
            } else if !supported {
                Some("数据包的版本高于当前应用，需升级后再导入".to_string())
            } else {
                None
            };
            PackageDatasetView {
                name: block.name.clone(),
                label: descriptor
                    .map(|item| item.label.clone())
                    .unwrap_or_else(|| block.name.clone()),
                policy: block.policy,
                schema_version: block.schema_version,
                record_count: block.record_count,
                carried: block.records.is_some(),
                supported,
                reason,
            }
        })
        .collect();
    PackageSummaryView {
        package_id: manifest.package_id.clone(),
        source_space_id: manifest.source_space_id.clone(),
        source_space_name: manifest.source_space_name.clone(),
        created_at: manifest.created_at.clone(),
        app_version: manifest.app_version.clone(),
        platform: manifest.platform.clone(),
        datasets,
        excluded: manifest.excluded.clone(),
    }
}

/// 默认导入选择：能导入的都选上（未携带记录的块由 `build_plan` 自动排除）
fn default_import_selection(
    manifest: &PackageManifest,
    descriptors: &[DatasetDescriptor],
) -> ImportSelection {
    let datasets = manifest
        .datasets
        .iter()
        .filter(|block| {
            descriptors.iter().any(|descriptor| {
                descriptor.name == block.name
                    && descriptor.schema_version >= block.schema_version
                    && block.policy != TransportPolicy::DeviceLocal
            })
        })
        .map(|block| block.name.clone())
        .collect();
    ImportSelection { datasets }
}

/// 二次导入命中：索引里已有同一 `packageId`
fn find_duplicate(app: &AppHandle, package_id: &str) -> Result<Option<DuplicateHint>, String> {
    let index = space_index::read_index(app)?;
    for (space_id, record) in index {
        if let Some(source) = record.imported_from.as_ref() {
            if source.package_id == package_id {
                return Ok(Some(DuplicateHint {
                    space_id: space_id.clone(),
                    space_name: record.display_name(&space_id),
                    imported_at: source.imported_at.clone(),
                }));
            }
        }
    }
    Ok(None)
}

/// 清单里各数据集的条数（不含记录体本身）
fn dataset_counts(manifest: &PackageManifest) -> BTreeMap<String, usize> {
    manifest
        .datasets
        .iter()
        .map(|block| (block.name.clone(), block.record_count))
        .collect()
}
