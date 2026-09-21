//! 框架 · 合并/覆盖导入提交（L3）：原地物化到当前空间，**不重启**
//!
//! 流程（L3 方案 §6.1）：
//! 1. 冻结数据库访问后快照将被修改的存储到 `backups/import-<时间戳>-<随机>`（保留最近 3 份，用户级后悔药）；
//! 2. 复核预览修订号，提交窗口内拒绝其他数据库访问；
//! 3. 分 owner 原子写入：sqlite 型 owner 单事务（崩溃即整体回滚），文件型 owner 原子替换；
//! 4. 更新导入映射（`import-map.json`）并广播 `space-data-changed`（前端定点重拉）。
//!
//! 原子性边界：单 owner 内部要么全成要么全不成；跨 owner（如 ssh 库已提交而偏好写失败）
//! 返回错误时自动还原快照；进程中断仍由快照恢复与重导幂等兜底。凭证永不在这条路径上。

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use tauri::AppHandle;

use super::adapter::{self, MergeTarget};
use super::import::ImportPlan;
use super::types::{IdMap, ImportMode, ImportReport, ItemDecision};
use super::{backup, lineage};
use crate::framework::context::{self, WriteFreezeGuard};
use crate::framework::space::index as space_index;

/// 将被修改的存储（空间根相对路径）：快照与修订号共用同一份清单
fn tracked_stores() -> Result<Vec<String>, String> {
    super::storage_files::tracked()
}

/// 提交合并/覆盖导入：计划（含决策与目标 id）→ 当前空间原地写入
pub(crate) fn commit(app: &AppHandle, plan: &ImportPlan) -> Result<ImportReport, String> {
    if plan.mode == ImportMode::NewSpace {
        return Err("隔离导入请走暂存物化路径（materialize），不走合并提交".into());
    }
    let device_root = context::root().ok_or("数据上下文未初始化")?;
    let space_root = space_index::space_root(device_root, &plan.space_id);
    if !space_root.exists() {
        return Err(format!("目标空间不存在：{}", plan.space_id));
    }

    // 1. 先等在途数据库访问完成，复核计划，再拍摄可恢复快照
    let freeze = WriteFreezeGuard::begin()?;
    let current = storage_revision(device_root, &plan.space_id)?;
    if plan.expected_revision.as_deref() != Some(current.as_str()) {
        return Err("预览之后本地数据有变化，请重新预览后再提交".into());
    }
    let stores = tracked_stores()?;
    let paths: Vec<&str> = stores.iter().map(String::as_str).collect();
    let snapshot = backup::snapshot_stores(device_root, &plan.space_id, &paths)?;
    // 决策表与 id 映射：计划阶段已落位，提交不再判定
    let decisions: BTreeMap<(String, String), ItemDecision> = plan
        .items
        .iter()
        .map(|item| ((item.dataset.clone(), item.id.clone()), item.decision))
        .collect();
    let id_map: IdMap = plan
        .items
        .iter()
        .filter_map(|item| {
            item.target_id
                .clone()
                .map(|target| ((item.dataset.clone(), item.id.clone()), target))
        })
        .collect();

    // 3. 分 owner 原子写入：跳过没有任何写入决策的数据集块（如全部标「未导入」的凭证块）
    let mut by_owner: BTreeMap<String, Vec<(String, Vec<serde_json::Value>)>> = BTreeMap::new();
    for block in &plan.blocks {
        let has_writes = plan.items.iter().any(|item| {
            item.dataset == block.dataset
                && matches!(
                    item.decision,
                    ItemDecision::Insert
                        | ItemDecision::PendingReference
                        | ItemDecision::Replace
                        | ItemDecision::KeepBoth
                )
        });
        // 覆盖模式下空块也要带着：清空该数据集本身就是写入语义
        if has_writes || plan.mode == ImportMode::Overwrite {
            by_owner
                .entry(block.owner.clone())
                .or_default()
                .push((block.dataset.clone(), block.records.clone()));
        }
    }
    let target = MergeTarget {
        space_root: &space_root,
        app,
        mode: plan.mode,
        id_map: &id_map,
        decisions: &decisions,
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut written_datasets: Vec<String> = Vec::new();
    for (owner, blocks) in &by_owner {
        let owner_adapter = adapter::all()
            .into_iter()
            .find(|item| item.owner() == owner)
            .ok_or_else(|| format!("没有 owner 为 {owner} 的数据适配器"))?;
        let written = match owner_adapter
            .apply_merge(blocks, &target)
            .and_then(|written| {
                for (dataset, records) in blocks {
                    let expected = if plan.mode == ImportMode::Overwrite {
                        records.len()
                    } else {
                        plan.items
                            .iter()
                            .filter(|item| {
                                item.dataset == *dataset
                                    && matches!(
                                        item.decision,
                                        ItemDecision::Insert
                                            | ItemDecision::PendingReference
                                            | ItemDecision::Replace
                                            | ItemDecision::KeepBoth
                                    )
                            })
                            .count()
                    };
                    if written.get(dataset).copied() != Some(expected) {
                        return Err(format!("数据集 {dataset} 的实际写入条数与计划不一致"));
                    }
                }
                if written
                    .keys()
                    .any(|dataset| !blocks.iter().any(|(name, _)| name == dataset))
                {
                    return Err("适配器返回了计划以外的数据集".into());
                }
                Ok(written)
            }) {
            Ok(written) => written,
            Err(error) => {
                let recovery = backup::restore_snapshot(device_root, &snapshot);
                drop(freeze);
                super::emit_space_data_changed(app, &["*".into()]);
                return Err(match recovery {
                    Ok(()) => format!("导入失败，已还原导入前数据：{error}"),
                    Err(recovery) => {
                        format!("导入失败：{error}；自动还原失败：{recovery}。请使用导入前快照恢复")
                    }
                });
            }
        };
        for (dataset, count) in written {
            counts.insert(dataset.clone(), count);
        }
        // 事件面按「触及的数据集」计（含覆盖模式的清空；与该块是否真有写入无关）
        for (dataset, _) in blocks {
            written_datasets.push(dataset.clone());
        }
    }

    // 4. 导入映射更新（写入成功的记录都登记；已识别/保留本地的同 id 命中也登记，便于下次识别）
    let map_path = lineage::map_path(device_root, &plan.space_id);
    let mut map = lineage::load(&map_path);
    let now = chrono::Utc::now().to_rfc3339();
    for item in &plan.items {
        let Some(target_id) = &item.target_id else {
            continue;
        };
        if matches!(
            item.decision,
            ItemDecision::Insert
                | ItemDecision::PendingReference
                | ItemDecision::Replace
                | ItemDecision::KeepBoth
                | ItemDecision::Identical
        ) {
            map.record(
                &plan.source_space_id,
                &item.dataset,
                &item.id,
                target_id,
                &plan.package_id,
                &now,
            );
        }
    }
    if let Err(error) = lineage::save(&map_path, &map) {
        let recovery = backup::restore_snapshot(device_root, &snapshot);
        drop(freeze);
        super::emit_space_data_changed(app, &["*".into()]);
        return Err(match recovery {
            Ok(()) => format!("保存导入映射失败，已还原数据：{error}"),
            Err(recovery) => {
                format!("保存导入映射失败：{error}；还原失败：{recovery}，请使用快照恢复")
            }
        });
    }

    if let Err(error) = backup::prune_snapshots(device_root) {
        // 清理失败只意味着多留了几份备份，不阻断提交；但要留痕，不静默
        log::warn!(
            "清理旧快照失败（不影响本次提交）: {error_type}",
            error_type = std::any::type_name_of_val(&error)
        );
    }

    // 5. 广播受影响数据集（前端订阅后定点重拉，不重启生效）
    drop(freeze);
    super::emit_space_data_changed(app, &written_datasets);

    Ok(ImportReport {
        space_id: plan.space_id.clone(),
        space_name: plan.space_name.clone(),
        package_id: plan.package_id.clone(),
        source_space_id: plan.source_space_id.clone(),
        source_space_name: plan.source_space_name.clone(),
        imported_at: now,
        counts,
        declared_counts: plan.declared_counts.clone(),
        pending: plan.pending_notes(),
        excluded: plan.excluded.clone(),
    })
}

/// 目标空间的存储修订号（expectedRevision）：将被修改的存储内容的哈希摘要
///
/// 提交前重算并与计划时给用户看到的值比对：不一致说明「预览之后本地又改过」，
/// 直接拒绝，让用户回到预览重新确认（D7）。
pub(crate) fn storage_revision(
    device_root: &std::path::Path,
    space_id: &str,
) -> Result<String, String> {
    let space_root = space_index::space_root(device_root, space_id);
    let mut hasher = Sha256::new();
    let mut files = std::collections::BTreeSet::new();
    for relative in tracked_stores()? {
        let expanded = super::storage_files::expand(&space_root, &relative)?;
        files.insert(relative);
        files.extend(expanded);
    }
    for relative in files {
        let path = super::storage_files::resolve(&space_root, &relative)?;
        if path.is_dir() {
            continue;
        }
        hasher.update(relative.as_bytes());
        if path.extension().is_some_and(|extension| extension == "db") {
            let wal = std::path::PathBuf::from(format!("{}-wal", path.display()));
            match std::fs::read(wal) {
                Ok(bytes) => hasher.update(&bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("读取数据库 WAL 失败: {error}")),
            }
        }
        match std::fs::read(&path) {
            Ok(bytes) => {
                hasher.update(&(bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hasher.update(b"<absent>");
            }
            Err(error) => {
                return Err(format!("读取 {} 失败: {error}", path.display()));
            }
        }
    }
    Ok(hex::encode(hasher.finalize()))
}
