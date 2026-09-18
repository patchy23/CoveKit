//! SSH 数据集目录、依赖闭包与只读导出。

use super::records::edge;
use super::records::non_empty;
use super::CREDENTIAL_DATASET;
use super::DATASET_BOOKMARKS;
use super::DATASET_GROUPS;
use super::DATASET_PROFILES;
use super::DATASET_TUNNELS;
use super::EDGE_BOOKMARK;
use super::EDGE_CREDENTIAL;
use super::EDGE_GROUP;
use super::EDGE_TUNNEL;
use super::NOTE_BOOKMARK_FOLLOWS;
use super::NOTE_GROUP_FOLLOWS;
use super::NOTE_NO_CREDENTIAL;
use super::NOTE_TUNNEL_FOLLOWS;
use super::SCHEMA_VERSION;
use crate::framework::data_transfer::types::CatalogEntry;
use crate::framework::data_transfer::types::DatasetDescriptor;
use crate::framework::data_transfer::types::DatasetPull;
use crate::framework::data_transfer::types::TransportPolicy;
use crate::framework::store::plugin_db_path;
use crate::plugins::ssh::models::SshBookmark;
use crate::plugins::ssh::models::TunnelConfig;
use crate::plugins::ssh::store;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::AppHandle;

/// 打开 SSH 插件库的只读连接并执行（库文件不存在 = 从未用过该工具，按空数据处理）
pub(super) fn with_read_conn<T>(
    app: &AppHandle,
    action: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let path = plugin_db_path(app, "ssh")?;
    if !path.exists() {
        return action(&empty_conn()?);
    }
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("SSH 数据库不可读: {e}"))?;
    action(&conn)
}

/// 空库连接（带同一份表结构）：库文件尚不存在时用它跑同一套查询，避免两套分支
fn empty_conn() -> Result<Connection, String> {
    let mut conn = Connection::open_in_memory().map_err(|e| format!("内存库打开失败: {e}"))?;
    crate::framework::store::migrate(&mut conn, store::MIGRATIONS)?;
    Ok(conn)
}

/// 声明四个数据集（纯函数：给定连接即可覆盖）
pub(super) fn describe(conn: &Connection) -> Result<Vec<DatasetDescriptor>, String> {
    let groups = store::profiles::list_groups(conn)?;
    let profiles = store::profiles::list_profiles(conn)?;

    let mut profile_entries = Vec::with_capacity(profiles.len());
    let mut tunnel_entries = Vec::new();
    let mut bookmark_entries = Vec::new();
    for profile in &profiles {
        let tunnels = store::tunnels::list_tunnels(conn, &profile.id)?;
        let bookmarks = store::bookmarks::list_bookmarks(conn, &profile.id)?;
        let mut dependencies = Vec::new();
        if let Some(id) = non_empty(profile.credential_ref.as_deref()) {
            dependencies.push(edge(EDGE_CREDENTIAL, &profile.id, id));
        }
        if let Some(id) = non_empty(profile.group_id.as_deref()) {
            dependencies.push(edge(EDGE_GROUP, &profile.id, id));
        }
        for tunnel in &tunnels {
            dependencies.push(edge(EDGE_TUNNEL, &profile.id, &tunnel.id));
            tunnel_entries.push(CatalogEntry {
                dataset: DATASET_TUNNELS.to_string(),
                id: tunnel.id.clone(),
                label: tunnel.name.clone(),
                detail: format!("{}:{}", tunnel.listen_host, tunnel.listen_port),
                dependencies: Vec::new(),
                note: Some(NOTE_TUNNEL_FOLLOWS.to_string()),
            });
        }
        for bookmark in &bookmarks {
            dependencies.push(edge(EDGE_BOOKMARK, &profile.id, &bookmark.id));
            bookmark_entries.push(CatalogEntry {
                dataset: DATASET_BOOKMARKS.to_string(),
                id: bookmark.id.clone(),
                label: bookmark.name.clone(),
                detail: bookmark.path.clone(),
                dependencies: Vec::new(),
                note: Some(NOTE_BOOKMARK_FOLLOWS.to_string()),
            });
        }
        // 未绑定凭证的档案在预览里标出来：导入后本机没有对应凭证，连接会失败
        let note = match non_empty(profile.credential_ref.as_deref()) {
            Some(_) => None,
            None => Some(NOTE_NO_CREDENTIAL.to_string()),
        };
        profile_entries.push(CatalogEntry {
            dataset: DATASET_PROFILES.to_string(),
            id: profile.id.clone(),
            label: profile.name.clone(),
            detail: format!("{}@{}:{}", profile.username, profile.host, profile.port),
            dependencies,
            note,
        });
    }

    let group_entries = groups
        .iter()
        .map(|group| CatalogEntry {
            dataset: DATASET_GROUPS.to_string(),
            id: group.id.clone(),
            label: group.name.clone(),
            detail: format!("排序 {}", group.sort_order),
            dependencies: Vec::new(),
            note: Some(NOTE_GROUP_FOLLOWS.to_string()),
        })
        .collect::<Vec<_>>();

    Ok(vec![
        descriptor(
            DATASET_PROFILES,
            "服务器档案",
            true,
            vec![
                (EDGE_CREDENTIAL, CREDENTIAL_DATASET),
                (EDGE_GROUP, DATASET_GROUPS),
                (EDGE_TUNNEL, DATASET_TUNNELS),
                (EDGE_BOOKMARK, DATASET_BOOKMARKS),
            ],
            None,
            profile_entries,
        ),
        descriptor(
            DATASET_GROUPS,
            "服务器分组",
            false,
            Vec::new(),
            Some(NOTE_GROUP_FOLLOWS),
            group_entries,
        ),
        descriptor(
            DATASET_TUNNELS,
            "隧道配置",
            false,
            Vec::new(),
            Some(NOTE_TUNNEL_FOLLOWS),
            tunnel_entries,
        ),
        descriptor(
            DATASET_BOOKMARKS,
            "目录书签",
            false,
            Vec::new(),
            Some(NOTE_BOOKMARK_FOLLOWS),
            bookmark_entries,
        ),
    ])
}

/// 组装一个数据集描述（记录数由条目数推出，保证与目录一致）
fn descriptor(
    name: &str,
    label: &str,
    selectable: bool,
    pulls: Vec<(&str, &str)>,
    note: Option<&str>,
    entries: Vec<CatalogEntry>,
) -> DatasetDescriptor {
    DatasetDescriptor {
        name: name.to_string(),
        label: label.to_string(),
        owner: "ssh".to_string(),
        policy: TransportPolicy::Portable,
        schema_version: SCHEMA_VERSION,
        selectable,
        contains_secret: false,
        default_selected: false,
        pulls: pulls
            .into_iter()
            .map(|(kind, dataset)| DatasetPull {
                kind: kind.to_string(),
                dataset: dataset.to_string(),
            })
            .collect(),
        note: note.map(str::to_string),
        record_count: entries.len(),
        entries,
    }
}

/// 按 id 取记录（纯函数）
pub(super) fn export(
    conn: &Connection,
    dataset: &str,
    ids: &[String],
) -> Result<Vec<Value>, String> {
    let index = records_index(conn, dataset)?;
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        let record = index
            .get(id)
            .ok_or_else(|| format!("{dataset} 不存在记录 {id}（列表可能已过期，请刷新后重试）"))?;
        records.push(record.clone());
    }
    Ok(records)
}

/// 数据集当前全部记录（id → 逻辑记录）：单次遍历建索引，导出与目录共用同一批行映射
pub(super) fn records_index(
    conn: &Connection,
    dataset: &str,
) -> Result<BTreeMap<String, Value>, String> {
    match dataset {
        DATASET_PROFILES => to_index(
            store::profiles::list_profiles(conn)?,
            "服务器档案",
            |item| &item.id,
        ),
        DATASET_GROUPS => to_index(
            store::profiles::list_groups(conn)?,
            "服务器分组",
            |item| &item.id,
        ),
        DATASET_TUNNELS | DATASET_BOOKMARKS => children_index(conn, dataset),
        other => Err(format!("SSH 适配器不支持数据集 {other}")),
    }
}

/// 子记录（隧道 / 书签）索引：两张表按 `profile_id` 分片，没有跨档案的既有查询入口，
/// 逐档案 listing 走既有行映射，不在适配器里另写一套 SQL
fn children_index(conn: &Connection, dataset: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut index = BTreeMap::new();
    for profile in store::profiles::list_profiles(conn)? {
        match dataset {
            DATASET_TUNNELS => {
                let tunnels: Vec<TunnelConfig> = store::tunnels::list_tunnels(conn, &profile.id)?;
                index.extend(to_value_map(tunnels, "隧道配置", |item| &item.id)?);
            }
            DATASET_BOOKMARKS => {
                let bookmarks: Vec<SshBookmark> =
                    store::bookmarks::list_bookmarks(conn, &profile.id)?;
                index.extend(to_value_map(bookmarks, "目录书签", |item| &item.id)?);
            }
            other => return Err(format!("SSH 适配器不支持数据集 {other}")),
        }
    }
    Ok(index)
}

/// 逻辑记录列表 → id 索引（id 取自身份字段，序列化失败即报错）
fn to_index<T>(
    items: Vec<T>,
    label: &str,
    id_of: impl Fn(&T) -> &String,
) -> Result<BTreeMap<String, Value>, String>
where
    T: serde::Serialize,
{
    to_value_map(items, label, id_of)
}

/// 序列化并建索引（统一错误文案：导出失败要说清是哪个数据集的哪条记录）
fn to_value_map<T>(
    items: Vec<T>,
    label: &str,
    id_of: impl Fn(&T) -> &String,
) -> Result<BTreeMap<String, Value>, String>
where
    T: serde::Serialize,
{
    let mut index = BTreeMap::new();
    for item in items {
        let id = id_of(&item).clone();
        let value =
            serde_json::to_value(&item).map_err(|e| format!("{label} {id} 序列化失败: {e}"))?;
        index.insert(id, value);
    }
    Ok(index)
}
