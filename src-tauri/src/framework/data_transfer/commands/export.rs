//! 数据传输命令 · export
use crate::framework::data_transfer::types::ExportCatalog;
use crate::framework::data_transfer::types::ExportSelection;
use crate::framework::data_transfer::types::PackageManifest;
use crate::framework::data_transfer::types::TransportPolicy;

use crate::framework::data_transfer::catalog;
use crate::framework::data_transfer::package;
use crate::framework::data_transfer::session;

use super::views::ExportReport;
use super::views::TransferStart;
use super::TASK_KIND;
use super::TASK_LABEL_EXPORT;
use crate::framework::tasks;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::AppHandle;

/// 当前空间可导出集合摘要与依赖关系
#[tauri::command]
pub fn data_export_catalog(app: AppHandle) -> Result<ExportCatalog, String> {
    catalog::build_catalog(&app)
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

/// 清单里各数据集的条数（不含记录体本身）
fn dataset_counts(manifest: &PackageManifest) -> BTreeMap<String, usize> {
    manifest
        .datasets
        .iter()
        .map(|block| (block.name.clone(), block.record_count))
        .collect()
}
