//! 数据传输命令 · import
use crate::framework::data_transfer::types::DatasetDescriptor;
use crate::framework::data_transfer::types::ImportReport;
use crate::framework::data_transfer::types::ImportSelection;
use crate::framework::data_transfer::types::PackageManifest;
use crate::framework::data_transfer::types::TransportPolicy;

use super::views::ConflictChoice;
use super::views::DuplicateHint;
use super::views::ImportCommitResult;
use super::views::ImportInspectResult;
use super::views::ImportPlanResult;
use super::views::PackageDatasetView;
use super::views::PackageSummaryView;
use super::TASK_KIND;
use super::TASK_LABEL_IMPORT;
use crate::framework::context::maintenance_guard;
use crate::framework::data_transfer::catalog;
use crate::framework::data_transfer::import;
use crate::framework::data_transfer::package;
use crate::framework::data_transfer::session;
use crate::framework::data_transfer::types::ConflictDecision;
use crate::framework::data_transfer::types::ImportMode;
use crate::framework::space::index as space_index;
use crate::framework::tasks;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::AppHandle;

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
            let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
            let before =
                crate::framework::data_transfer::merge::storage_revision(device_root, &space_id)?;
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
            let after =
                crate::framework::data_transfer::merge::storage_revision(device_root, &space_id)?;
            if before != after {
                return Err("生成预览期间本地数据发生变化，请重新预览".into());
            }
            plan.expected_revision = Some(after);
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
                    let current = crate::framework::data_transfer::merge::storage_revision(
                        device_root,
                        &plan.space_id,
                    )?;
                    if plan.expected_revision.as_deref() != Some(current.as_str()) {
                        return Err("预览之后本地数据有变化，请重新预览后再提交".into());
                    }
                    crate::framework::data_transfer::merge::commit(&commit_app, &plan)
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
