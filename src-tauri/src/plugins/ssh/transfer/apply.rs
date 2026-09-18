//! SSH 导入写入：记录重映射、暂存库和事务内合并。

use super::dataset_order;
use super::owns;
use super::records::decode;
use super::records::record_identity;
use super::DATASET_BOOKMARKS;
use super::DATASET_GROUPS;
use super::DATASET_PROFILES;
use super::DATASET_TUNNELS;
use crate::framework::data_transfer::adapter::StagingTarget;
use crate::framework::data_transfer::types::IdMap;
use crate::framework::data_transfer::types::ImportMode;
use crate::framework::data_transfer::types::ItemDecision;
use crate::plugins::ssh::models::ServerProfile;
use crate::plugins::ssh::models::SshBookmark;
use crate::plugins::ssh::models::SshGroup;
use crate::plugins::ssh::models::TunnelConfig;
use crate::plugins::ssh::store;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use serde_json::Value;
use std::collections::BTreeMap;

/// 写入新空间暂存的插件库：建库（沿用插件同一份迁移）→ 逐条写入 → 读回自查
///
/// 只在 `target.root` 之内写文件；读回这一步是契约要求的自查（写坏了的包不能进空间）。
pub(super) fn apply_to_staging(
    dataset: &str,
    records: &[Value],
    target: &StagingTarget,
) -> Result<usize, String> {
    if !owns(dataset) {
        return Err(format!("SSH 适配器不支持数据集 {dataset}"));
    }
    let path = target.root.join("data").join("ssh.db");
    let mut conn = Connection::open(&path).map_err(|e| format!("创建暂存 SSH 库失败: {e}"))?;
    crate::framework::store::migrate(&mut conn, store::MIGRATIONS)?;
    for record in records {
        insert_record(&mut conn, dataset, record)?;
    }
    let written = count_rows(&path, dataset)?;
    if written != records.len() {
        return Err(format!(
            "{dataset} 写入后读回 {written} 条，预期 {} 条",
            records.len()
        ));
    }
    Ok(records.len())
}

/// 按数据集写入一条记录（走插件既有的 upsert，不在导入侧另写 SQL）
///
/// 时间戳用「现在」：导入产生的是新空间里的新记录，`created_at` 表示本机登记时间；
/// 传输记录本身不带表内时间列（逻辑 schema 里没有它们）。
fn insert_record(conn: &Connection, dataset: &str, record: &Value) -> Result<(), String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    match dataset {
        DATASET_PROFILES => {
            let profile = decode::<ServerProfile>(record, "服务器档案")?;
            store::profiles::upsert_profile(conn, &profile, now_ms)
        }
        DATASET_GROUPS => {
            let group = decode::<SshGroup>(record, "服务器分组")?;
            store::profiles::upsert_group(conn, &group)
        }
        DATASET_TUNNELS => {
            let tunnel = decode::<TunnelConfig>(record, "隧道配置")?;
            store::tunnels::upsert_tunnel(conn, &tunnel, now_ms)
        }
        DATASET_BOOKMARKS => {
            let bookmark = decode::<SshBookmark>(record, "目录书签")?;
            store::bookmarks::upsert_bookmark(conn, &bookmark)
        }
        other => Err(format!("SSH 适配器不支持数据集 {other}")),
    }
}

/// 读回暂存库并按数据集计数（只读打开；用于写入后的自查）
pub(super) fn count_rows(path: &std::path::Path, dataset: &str) -> Result<usize, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("暂存 SSH 库不可读: {e}"))?;
    match dataset {
        DATASET_PROFILES => Ok(store::profiles::list_profiles(&conn)?.len()),
        DATASET_GROUPS => Ok(store::profiles::list_groups(&conn)?.len()),
        DATASET_TUNNELS | DATASET_BOOKMARKS => children_rows(&conn, dataset),
        other => Err(format!("SSH 适配器不支持数据集 {other}")),
    }
}

/// 子记录（隧道 / 书签）计数：逐档案走既有 listing，不另写一套 SQL
fn children_rows(conn: &Connection, dataset: &str) -> Result<usize, String> {
    let mut total = 0;
    for profile in store::profiles::list_profiles(conn)? {
        total += match dataset {
            DATASET_TUNNELS => store::tunnels::list_tunnels(conn, &profile.id)?.len(),
            _ => store::bookmarks::list_bookmarks(conn, &profile.id)?.len(),
        };
    }
    Ok(total)
}

/* ── 合并/覆盖导入写入（L3）── */

/// 在事务连接内按决策写入全部数据集块（纯核：无 AppHandle，可注入内存库测试）
///
/// 顺序按 `dataset_order()`（分组 → 档案 → 隧道 → 书签），保证子记录落库时档案已在；
/// 覆盖模式先清空四张表（子表在前）再整体写入。
pub(super) fn apply_merge_blocks(
    conn: &Connection,
    dataset_blocks: &[(String, Vec<Value>)],
    mode: ImportMode,
    id_map: &IdMap,
    decisions: &BTreeMap<(String, String), ItemDecision>,
) -> Result<BTreeMap<String, usize>, String> {
    if mode == ImportMode::Overwrite {
        store::bookmarks::clear_bookmarks(conn)?;
        store::tunnels::clear_tunnels(conn)?;
        store::profiles::clear_profiles(conn)?;
        store::profiles::clear_groups(conn)?;
    }
    let order = dataset_order();
    let mut sorted: Vec<&(String, Vec<Value>)> = dataset_blocks.iter().collect();
    sorted.sort_by_key(|(dataset, _)| {
        order
            .iter()
            .position(|known| known == dataset)
            .unwrap_or(order.len())
    });
    let mut counts = BTreeMap::new();
    for (dataset, records) in sorted {
        if !owns(dataset) {
            return Err(format!("SSH 适配器不支持数据集 {dataset}"));
        }
        let mut written = 0usize;
        for record in records {
            if apply_merge_record(conn, dataset, record, mode, id_map, decisions)? {
                written += 1;
            }
        }
        counts.insert(dataset.clone(), written);
    }
    Ok(counts)
}

/// 单条记录的合并/覆盖写入；返回是否真的写了（跳过类决策不写）
fn apply_merge_record(
    conn: &Connection,
    dataset: &str,
    record: &Value,
    mode: ImportMode,
    id_map: &IdMap,
    decisions: &BTreeMap<(String, String), ItemDecision>,
) -> Result<bool, String> {
    let (source_id, _) = record_identity(dataset, record)?;
    // 覆盖模式没有逐条决策：一律写入
    let decision = match mode {
        ImportMode::Overwrite => ItemDecision::Insert,
        _ => match decisions.get(&(dataset.to_string(), source_id.clone())) {
            Some(decision) => *decision,
            // 计划外的记录不写（宁可少写，不写计划外的数据）
            None => return Ok(false),
        },
    };
    match decision {
        ItemDecision::Insert
        | ItemDecision::PendingReference
        | ItemDecision::Replace
        | ItemDecision::KeepBoth => {}
        ItemDecision::Identical
        | ItemDecision::Skip
        | ItemDecision::RestorePrompt
        | ItemDecision::Excluded => return Ok(false),
    }
    let target_id = id_map
        .get(&(dataset.to_string(), source_id.clone()))
        .ok_or_else(|| format!("{dataset} 记录 {source_id} 缺少目标 id（计划未落位）"))?;
    let rewritten = rewrite_record(dataset, record, target_id, id_map)?;
    insert_record(conn, dataset, &rewritten)?;
    Ok(true)
}

/// 写入前的记录改写：id 换成目标 id；引用字段按 id_map 改写；
/// 凭证引用一律置空（凭证永不随包，导入后重新补录）
pub(super) fn rewrite_record(
    dataset: &str,
    record: &Value,
    target_id: &str,
    id_map: &IdMap,
) -> Result<Value, String> {
    let mut value = record.clone();
    let map = value
        .as_object_mut()
        .ok_or_else(|| format!("{dataset} 记录不是对象"))?;
    map.insert("id".to_string(), Value::String(target_id.to_string()));
    match dataset {
        DATASET_PROFILES => {
            map.insert("credentialRef".to_string(), Value::Null);
            // 分组引用：映射到目标分组 id；分组没进包（待补录）则置空，不悬空
            let group_id = map
                .get("groupId")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(group_id) = group_id {
                let mapped = id_map.get(&(DATASET_GROUPS.to_string(), group_id)).cloned();
                map.insert(
                    "groupId".to_string(),
                    mapped.map(Value::String).unwrap_or(Value::Null),
                );
            }
        }
        DATASET_TUNNELS | DATASET_BOOKMARKS => {
            // 档案引用必须有目标 id：档案没写入的子记录是孤儿，宁可报错也不留悬空引用
            let profile_id = map
                .get("profileId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{dataset} 记录缺少档案引用"))?;
            let mapped = id_map
                .get(&(DATASET_PROFILES.to_string(), profile_id.clone()))
                .cloned()
                .ok_or_else(|| {
                    format!("{dataset} 记录引用的档案 {profile_id} 没有目标 id（依赖未导入）")
                })?;
            map.insert("profileId".to_string(), Value::String(mapped));
        }
        _ => {}
    }
    Ok(value)
}
