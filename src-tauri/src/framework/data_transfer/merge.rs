//! 框架 · 合并/覆盖导入提交（L3）：原地物化到当前空间，**不重启**
//!
//! 流程（L3 方案 §6.1）：
//! 1. 快照将被修改的存储到 `backups/import-<时间戳>-<随机>`（保留最近 3 份，用户级后悔药）；
//! 2. 挂写冻结守卫（提交窗口内用户写入口被拒；窗口亚秒级）；
//! 3. 分 owner 原子写入：sqlite 型 owner 单事务（崩溃即整体回滚），文件型 owner 原子替换；
//! 4. 更新导入映射（`import-map.json`）并广播 `space-data-changed`（前端定点重拉）。
//!
//! 原子性边界：单 owner 内部要么全成要么全不成；跨 owner（如 ssh 库已提交而偏好写失败）
//! 不做分布式事务，由快照 + 重导幂等自愈兜底（方案 §6.4）。凭证永不在这条路径上。

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::Hasher;

use tauri::AppHandle;

use super::adapter::{self, MergeTarget};
use super::import::ImportPlan;
use super::types::{IdMap, ImportMode, ImportReport, ItemDecision};
use super::{backup, lineage};
use crate::framework::context::{self, WriteFreezeGuard};
use crate::framework::space::index as space_index;

/// 将被修改的存储（空间根相对路径）：快照与修订号共用同一份清单
const TRACKED_STORES: [&str; 3] = ["data/ssh.db", "preferences.json", lineage::IMPORT_MAP_FILE];

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

    // 1. 提交前快照（将被修改的存储），并清理超出保留份数的旧快照
    let _snapshot = backup::snapshot_stores(device_root, &plan.space_id, &TRACKED_STORES)?;
    if let Err(error) = backup::prune_snapshots(device_root) {
        // 清理失败只意味着多留了几份备份，不阻断提交；但要留痕，不静默
        eprintln!("[data_transfer] 清理旧快照失败（不影响本次提交）: {error}");
    }
    // 2. 写冻结（离开作用域自动解除；提交失败也一样解除）
    let _freeze = WriteFreezeGuard::begin();

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
        let written = owner_adapter.apply_merge(blocks, &target)?;
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
    lineage::save(&map_path, &map)?;

    // 5. 广播受影响数据集（前端订阅后定点重拉，不重启生效）
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
    let mut hasher = DefaultHasher::new();
    for relative in TRACKED_STORES {
        let path = space_root.join(relative);
        hasher.write(relative.as_bytes());
        match std::fs::read(&path) {
            Ok(bytes) => {
                hasher.write(&(bytes.len() as u64).to_le_bytes());
                hasher.write(&bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hasher.write(b"<absent>");
            }
            Err(error) => {
                return Err(format!("读取 {} 失败: {error}", path.display()));
            }
        }
    }
    Ok(format!("{:016x}", hasher.finish()))
}
