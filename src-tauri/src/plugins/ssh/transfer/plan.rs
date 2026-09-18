//! SSH 导入规划：新增、覆盖与冲突决策。

use super::catalog::records_index;
use super::catalog::with_read_conn;
use super::records::business_key;
use super::records::decode;
use super::records::non_empty;
use super::records::normalize_compare;
use super::records::record_identity;
use super::CREDENTIAL_DATASET;
use super::DATASET_BOOKMARKS;
use super::DATASET_GROUPS;
use super::DATASET_PROFILES;
use super::DATASET_TUNNELS;
use crate::framework::data_transfer::types::ConflictDecision;
use crate::framework::data_transfer::types::ImportContext;
use crate::framework::data_transfer::types::ImportMode;
use crate::framework::data_transfer::types::ImportPlanItem;
use crate::framework::data_transfer::types::ItemDecision;
use crate::framework::data_transfer::types::MergeContext;
use crate::framework::data_transfer::types::MergePlanView;
use crate::plugins::ssh::models::ServerProfile;
use crate::plugins::ssh::models::SshBookmark;
use crate::plugins::ssh::models::SshGroup;
use crate::plugins::ssh::models::TunnelConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::AppHandle;

/// 导入判定：逐条给出「会写入 / 待补全」结论
///
/// 判定依据是**包内实际带了什么**（`ImportContext`），不是「本机现在有没有」：
/// 本机同 id 的凭证与包里引用的凭证是两回事，导入后要不要重绑由用户决定。
pub(super) fn plan_import(
    dataset: &str,
    records: &[Value],
    context: &ImportContext<'_>,
) -> Result<Vec<ImportPlanItem>, String> {
    // 合并/覆盖模式走与当前空间现状比对的判定链
    if let Some(view) = &context.merge {
        return plan_import_live(dataset, records, context, view);
    }
    let mut items = Vec::with_capacity(records.len());
    match dataset {
        DATASET_PROFILES => {
            for record in records {
                items.push(profile_plan_item(record, context)?);
            }
        }
        DATASET_GROUPS => {
            for record in records {
                let group = decode::<SshGroup>(record, "服务器分组")?;
                items.push(ImportPlanItem::added(
                    DATASET_GROUPS,
                    &group.id,
                    &group.name,
                ));
            }
        }
        DATASET_TUNNELS => {
            for record in records {
                let tunnel = decode::<TunnelConfig>(record, "隧道配置")?;
                items.push(ImportPlanItem::added(
                    DATASET_TUNNELS,
                    &tunnel.id,
                    &tunnel.name,
                ));
            }
        }
        DATASET_BOOKMARKS => {
            for record in records {
                let bookmark = decode::<SshBookmark>(record, "目录书签")?;
                items.push(ImportPlanItem::added(
                    DATASET_BOOKMARKS,
                    &bookmark.id,
                    &bookmark.name,
                ));
            }
        }
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(items)
}

/// 单条档案的导入结论：引用的凭证 / 分组没随包带来就是「待补全」
fn profile_plan_item(
    record: &Value,
    context: &ImportContext<'_>,
) -> Result<ImportPlanItem, String> {
    let profile = decode::<ServerProfile>(record, "服务器档案")?;
    Ok(match profile_pending_note(&profile, context) {
        None => ImportPlanItem::added(DATASET_PROFILES, &profile.id, &profile.name),
        Some(note) => {
            ImportPlanItem::pending_reference(DATASET_PROFILES, &profile.id, &profile.name, &note)
        }
    })
}

/// 档案的「待补录」提示：引用的凭证 / 分组没随包带来时给出（合并/覆盖模式下同样要带）
fn profile_pending_note(profile: &ServerProfile, context: &ImportContext<'_>) -> Option<String> {
    let mut missing = Vec::new();
    if let Some(id) = non_empty(profile.credential_ref.as_deref()) {
        if !context.carries_id(CREDENTIAL_DATASET, id) {
            missing.push(format!("凭证 {id}"));
        }
    }
    if let Some(id) = non_empty(profile.group_id.as_deref()) {
        if !context.carries_id(DATASET_GROUPS, id) {
            missing.push(format!("分组 {id}"));
        }
    }
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "{} 未随包带出，导入后需重新绑定",
            missing.join("、")
        ))
    }
}

/* ── 合并/覆盖导入判定（L3）── */

/// 合并/覆盖模式的导入判定：与当前空间现状逐条比对出处置结论
///
/// 命中顺序（L3 方案 §3.5）：映射命中 → 主键命中 → 业务键命中 → 全新插入。
/// 凭证引用不参与同一性比较（凭证永不随包，导入后一律置空待补录）。
fn plan_import_live(
    dataset: &str,
    records: &[Value],
    context: &ImportContext<'_>,
    view: &MergePlanView<'_>,
) -> Result<Vec<ImportPlanItem>, String> {
    // 当前空间同数据集的全部逻辑记录（库文件不存在按空处理，与导出侧口径一致）
    let live = with_read_conn(view.app, |conn| records_index(conn, dataset))?;
    // 覆盖模式：包内容整体替换目标数据集，不做逐条冲突判定（提交时先清空）
    if view.core.mode == ImportMode::Overwrite {
        return records
            .iter()
            .map(|record| overwrite_item(dataset, record, context))
            .collect();
    }
    // 业务键索引：档案/分组/隧道按名称；书签按「档案名::路径」（档案名经两侧档案数据集解析）
    let by_key = business_key_index(dataset, &live, view.app)?;
    plan_merge_items(dataset, records, context, &view.core, &live, &by_key)
}

/// 合并判定的纯核：全部输入就绪后的逐条判定（可单测）
pub(super) fn plan_merge_items(
    dataset: &str,
    records: &[Value],
    context: &ImportContext<'_>,
    core: &MergeContext<'_>,
    live: &BTreeMap<String, Value>,
    by_key: &BTreeMap<String, String>,
) -> Result<Vec<ImportPlanItem>, String> {
    let mut items = Vec::with_capacity(records.len());
    for record in records {
        items.push(merge_item(dataset, record, context, core, live, by_key)?);
    }
    Ok(items)
}

/// 当前空间记录的业务键索引（键 → 本地记录 id；同名多条保留先见者，键只用于冲突提示）
fn business_key_index(
    dataset: &str,
    live: &BTreeMap<String, Value>,
    app: &AppHandle,
) -> Result<BTreeMap<String, String>, String> {
    let mut index = BTreeMap::new();
    match dataset {
        DATASET_PROFILES | DATASET_GROUPS | DATASET_TUNNELS => {
            for (id, record) in live {
                let name = record
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !name.is_empty() {
                    index.entry(name.to_string()).or_insert_with(|| id.clone());
                }
            }
        }
        DATASET_BOOKMARKS => {
            // 书签业务键 = 档案名 + 路径：本地档案名从档案索引解析
            let live_profiles = with_read_conn(app, |conn| records_index(conn, DATASET_PROFILES))?;
            for (id, record) in live {
                let profile_id = record
                    .get("profileId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let path = record
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let profile_name = live_profiles
                    .get(profile_id)
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str);
                if let (Some(name), false) = (profile_name, path.is_empty()) {
                    index
                        .entry(format!("{name}::{path}"))
                        .or_insert_with(|| id.clone());
                }
            }
        }
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(index)
}

/// 单条记录的合并判定
fn merge_item(
    dataset: &str,
    record: &Value,
    context: &ImportContext<'_>,
    core: &MergeContext<'_>,
    live: &BTreeMap<String, Value>,
    by_key: &BTreeMap<String, String>,
) -> Result<ImportPlanItem, String> {
    let (id, label) = record_identity(dataset, record)?;
    let pending_note = match dataset {
        DATASET_PROFILES => {
            profile_pending_note(&decode::<ServerProfile>(record, "服务器档案")?, context)
        }
        _ => None,
    };

    // 1. 映射命中（同源识别）：目标在本地 → 内容比对；目标已不在（用户删过）→ 默认不复活
    let mapped = core.lineage.lookup(&context.source_space_id, dataset, &id);
    if !mapped.is_empty() {
        let hit = mapped
            .iter()
            .find(|entry| live.contains_key(&entry.target_id))
            .map(|entry| entry.target_id.clone());
        return Ok(match hit {
            Some(target_id) => {
                let live_record = live
                    .get(&target_id)
                    .ok_or_else(|| format!("{dataset} 本地记录 {target_id} 读取失败"))?;
                if normalize_compare(live_record) == normalize_compare(record) {
                    identified_item(dataset, &id, &label, &target_id, pending_note)
                } else {
                    conflict_item(
                        dataset,
                        &id,
                        &label,
                        core,
                        Some(&target_id),
                        "这条记录此前导入过、之后内容有了变化",
                        pending_note,
                    )
                }
            }
            None => {
                let mut item = plan_item(dataset, &id, &label, ItemDecision::RestorePrompt);
                item.note = Some(join_note(
                    "这条记录此前导入过、本地已删除；默认不复活，需要的话请显式选择恢复",
                    pending_note,
                ));
                item
            }
        });
    }

    // 2. 主键命中
    if let Some(live_record) = live.get(&id) {
        return Ok(
            if normalize_compare(live_record) == normalize_compare(record) {
                identified_item(dataset, &id, &label, &id, pending_note)
            } else {
                conflict_item(
                    dataset,
                    &id,
                    &label,
                    core,
                    Some(&id),
                    "与本地同 id 记录内容不同",
                    pending_note,
                )
            },
        );
    }

    // 3. 业务键命中（同名 / 同档案同路径）
    if let Some(key) = business_key(dataset, record, core)? {
        if let Some(target_id) = by_key.get(&key) {
            return Ok(conflict_item(
                dataset,
                &id,
                &label,
                core,
                Some(target_id),
                "本地已存在同名记录",
                pending_note,
            ));
        }
    }

    // 4. 全新插入
    let mut item = plan_item(dataset, &id, &label, ItemDecision::Insert);
    item.note = pending_note;
    Ok(item)
}

/// 覆盖模式条目：整体替换目标数据集，全部按「新增写入」判定（提交时先清空）
pub(super) fn overwrite_item(
    dataset: &str,
    record: &Value,
    context: &ImportContext<'_>,
) -> Result<ImportPlanItem, String> {
    let (id, label) = record_identity(dataset, record)?;
    let mut item = plan_item(dataset, &id, &label, ItemDecision::Insert);
    if dataset == DATASET_PROFILES {
        item.note = profile_pending_note(&decode::<ServerProfile>(record, "服务器档案")?, context);
    }
    Ok(item)
}

/// 已识别条目（内容一致，跳过写入）
fn identified_item(
    dataset: &str,
    id: &str,
    label: &str,
    target_id: &str,
    pending_note: Option<String>,
) -> ImportPlanItem {
    let mut item = plan_item(dataset, id, label, ItemDecision::Identical);
    item.target_id = Some(target_id.to_string());
    item.note = pending_note;
    item
}

/// 冲突条目：按用户决策落位，未选择时默认「保留本地」
fn conflict_item(
    dataset: &str,
    id: &str,
    label: &str,
    core: &MergeContext<'_>,
    target_id: Option<&str>,
    reason: &str,
    pending_note: Option<String>,
) -> ImportPlanItem {
    let chosen = core
        .decisions
        .get(&(dataset.to_string(), id.to_string()))
        .copied()
        .unwrap_or(ConflictDecision::KeepLocal);
    let mut item = match chosen {
        ConflictDecision::KeepLocal => plan_item(dataset, id, label, ItemDecision::Skip),
        ConflictDecision::UseImported => plan_item(dataset, id, label, ItemDecision::Replace),
        ConflictDecision::KeepBoth => plan_item(dataset, id, label, ItemDecision::KeepBoth),
    };
    item.conflict = true;
    // 保留两份要写新记录（目标 id 由框架在计划汇总时新分配），其余处置落在既有记录上
    item.target_id = match chosen {
        ConflictDecision::KeepBoth => None,
        _ => target_id.map(str::to_string),
    };
    item.note = Some(join_note(reason, pending_note));
    item
}

/// 计划条目骨架
fn plan_item(dataset: &str, id: &str, label: &str, decision: ItemDecision) -> ImportPlanItem {
    ImportPlanItem {
        dataset: dataset.to_string(),
        id: id.to_string(),
        label: label.to_string(),
        decision,
        conflict: false,
        target_id: None,
        impact: None,
        note: None,
    }
}

/// 冲突原因与「待补录」提示拼成一条说明
fn join_note(reason: &str, pending_note: Option<String>) -> String {
    match pending_note {
        Some(extra) => format!("{reason}；{extra}"),
        None => reason.to_string(),
    }
}
