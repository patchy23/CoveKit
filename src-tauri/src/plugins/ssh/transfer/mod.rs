//! SSH 数据集的传输适配（sync-202609-001 L2 · owner = `ssh`）
//!
//! 导出哪些、按什么粒度勾选（任务书 §13.1 / §13.7）：
//! - `ssh.profiles` 是唯一可勾选的数据集（勾选粒度 = 档案）。
//! - `ssh.groups` / `ssh.tunnels` / `ssh.bookmarks` 跟随所选档案带出，不提供单独勾选：
//!   单独搬运一个隧道或书签而没有档案，落地后只是孤儿行。
//! - `known_hosts` 主机密钥**不带出**（任务书 D4：认机器不认人，换了机器本来就该重新确认）。
//!
//! 记录 = 逻辑结构（`ServerProfile` / `SshGroup` / `TunnelConfig` / `SshBookmark` 的 serde 形态），
//! 不是数据表原始行：`created_at` / `updated_at` 属本机事实，导入侧按导入时间重写。
//! 因此这里读库只用于取逻辑字段，写入侧（C3 导入）复用同一批结构反序列化——格式即契约。
//!
//! 依赖边有两类用途，不要求一一对应：
//! - `CatalogEntry::dependencies`：勾选闭包的输入（档案 → 凭证/分组/隧道/书签）。
//! - `enumerate_references`：清单里的记录级引用（档案 → 凭证/分组；隧道与书签 → 所属档案），
//!   供导入侧排序与完整性校验；隧道/书签引用档案这件事不构成「勾选档案」的反向跟随。

use std::collections::BTreeMap;

use rusqlite::{Connection, OpenFlags};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tauri::AppHandle;

use crate::framework::data_transfer::adapter::{self, DatasetAdapter, StagingTarget};
use crate::framework::data_transfer::types::{
    CatalogEntry, DatasetDescriptor, DatasetPull, DependencyEdge, ImportContext, ImportPlanItem,
    TransportPolicy,
};
use crate::framework::store::plugin_db_path;

use super::models::{ServerProfile, SshBookmark, SshGroup, TunnelConfig};
use super::store;

/// 数据集名：服务器档案（唯一可勾选）
pub(crate) const DATASET_PROFILES: &str = "ssh.profiles";
/// 数据集名：服务器分组（跟随带出）
pub(crate) const DATASET_GROUPS: &str = "ssh.groups";
/// 数据集名：隧道配置（跟随带出）
pub(crate) const DATASET_TUNNELS: &str = "ssh.tunnels";
/// 数据集名：目录书签（跟随带出）
pub(crate) const DATASET_BOOKMARKS: &str = "ssh.bookmarks";
/// 数据集 schema 版本（逻辑结构变更时 +1）
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// 依赖边类型：档案引用的凭证（落到 `vault.credentials`）
const EDGE_CREDENTIAL: &str = "credential";
/// 依赖边类型：档案所属分组
const EDGE_GROUP: &str = "group";
/// 依赖边类型：档案下的隧道
const EDGE_TUNNEL: &str = "tunnel";
/// 依赖边类型：档案下的书签
const EDGE_BOOKMARK: &str = "bookmark";
/// 依赖边类型：子记录所属档案（导入侧据此排序）
const EDGE_PROFILE: &str = "profile";

/// 凭证数据集名（跨 owner 引用：SSH 只存引用，秘密本体在公共 Vault）
const CREDENTIAL_DATASET: &str = "vault.credentials";

/// 不可单独勾选时的说明
const NOTE_GROUP_FOLLOWS: &str = "由所选档案的分组自动带出，不可单独勾选";
/// 隧道不可单独勾选时的说明
const NOTE_TUNNEL_FOLLOWS: &str = "随所选服务器档案自动带出，不可单独勾选";
/// 书签不可单独勾选时的说明
const NOTE_BOOKMARK_FOLLOWS: &str = "随所选服务器档案自动带出，不可单独勾选";
/// 未绑定凭证的档案提示（导入后需在本机补全）
const NOTE_NO_CREDENTIAL: &str = "未绑定凭证，导入后需补全凭证";

/// SSH 数据集适配器（进程级单例）
struct SshAdapter;

impl DatasetAdapter for SshAdapter {
    /// 归属 owner
    fn owner(&self) -> &'static str {
        "ssh"
    }

    /// 声明四个数据集与勾选闭包的跟随关系
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        with_read_conn(app, |conn| describe(conn))
    }

    /// 按 id 取指定数据集的记录（顺序与 `ids` 一致，缺 id 直接报错）
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        with_read_conn(app, |conn| export(conn, dataset, ids))
    }

    /// 记录体检：结构必须能反序列化回逻辑结构，关键字段不许为空
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        validate(dataset, records)
    }

    /// 记录级引用（清单 `dependencies` 用）
    fn enumerate_references(
        &self,
        dataset: &str,
        records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        references(dataset, records)
    }

    /// 导入判定：档案引用的凭证 / 分组没随包带来 → 标「待补全」，不当成完整导入
    fn plan_import(
        &self,
        dataset: &str,
        records: &[Value],
        context: &ImportContext,
    ) -> Result<Vec<ImportPlanItem>, String> {
        plan_import(dataset, records, context)
    }

    /// 写入新空间暂存的 SSH 库：建库 → 按数据集写入 → 读回自查
    fn apply_to_staging(
        &self,
        dataset: &str,
        records: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        apply_to_staging(dataset, records, target)
    }
}

/// 登记 SSH 适配器（插件装配阶段调用一次）
pub(crate) fn register() {
    static ADAPTER: SshAdapter = SshAdapter;
    adapter::register(&ADAPTER);
}

/// 打开 SSH 插件库的只读连接并执行（库文件不存在 = 从未用过该工具，按空数据处理）
fn with_read_conn<T>(
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
fn describe(conn: &Connection) -> Result<Vec<DatasetDescriptor>, String> {
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
fn export(conn: &Connection, dataset: &str, ids: &[String]) -> Result<Vec<Value>, String> {
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
fn records_index(conn: &Connection, dataset: &str) -> Result<BTreeMap<String, Value>, String> {
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

/// 记录体检（纯函数）：结构能反序列化回逻辑结构 + 关键字段非空
fn validate(dataset: &str, records: &[Value]) -> Result<(), String> {
    match dataset {
        DATASET_PROFILES => {
            for record in records {
                let profile: ServerProfile = decode(record, "服务器档案")?;
                require("服务器档案", &profile.id, &profile.name)?;
                if profile.host.trim().is_empty() {
                    return Err(format!("服务器档案 {} 缺少主机地址", profile.id));
                }
                if profile.port == 0 {
                    return Err(format!("服务器档案 {} 端口非法（0）", profile.id));
                }
            }
        }
        DATASET_GROUPS => {
            for record in records {
                let group: SshGroup = decode(record, "服务器分组")?;
                require("服务器分组", &group.id, &group.name)?;
            }
        }
        DATASET_TUNNELS => {
            for record in records {
                let tunnel: TunnelConfig = decode(record, "隧道配置")?;
                require("隧道配置", &tunnel.id, &tunnel.name)?;
                if tunnel.profile_id.trim().is_empty() {
                    return Err(format!("隧道 {} 缺少所属档案", tunnel.id));
                }
                if tunnel.listen_port == 0 {
                    return Err(format!("隧道 {} 监听端口非法（0）", tunnel.id));
                }
            }
        }
        DATASET_BOOKMARKS => {
            for record in records {
                let bookmark: SshBookmark = decode(record, "目录书签")?;
                require("目录书签", &bookmark.id, &bookmark.name)?;
                if bookmark.profile_id.trim().is_empty() {
                    return Err(format!("书签 {} 缺少所属档案", bookmark.id));
                }
                if bookmark.path.trim().is_empty() {
                    return Err(format!("书签 {} 缺少远程路径", bookmark.id));
                }
            }
        }
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(())
}

/// 记录级引用（清单 `dependencies` 用）：档案 → 凭证/分组；隧道与书签 → 所属档案
fn references(dataset: &str, records: &[Value]) -> Result<Vec<DependencyEdge>, String> {
    let mut edges = Vec::new();
    match dataset {
        DATASET_PROFILES => {
            for record in records {
                let profile: ServerProfile = decode(record, "服务器档案")?;
                if let Some(id) = non_empty(profile.credential_ref.as_deref()) {
                    edges.push(edge(EDGE_CREDENTIAL, &profile.id, id));
                }
                if let Some(id) = non_empty(profile.group_id.as_deref()) {
                    edges.push(edge(EDGE_GROUP, &profile.id, id));
                }
            }
        }
        DATASET_TUNNELS => {
            for record in records {
                let tunnel: TunnelConfig = decode(record, "隧道配置")?;
                edges.push(edge(EDGE_PROFILE, &tunnel.id, &tunnel.profile_id));
            }
        }
        DATASET_BOOKMARKS => {
            for record in records {
                let bookmark: SshBookmark = decode(record, "目录书签")?;
                edges.push(edge(EDGE_PROFILE, &bookmark.id, &bookmark.profile_id));
            }
        }
        DATASET_GROUPS => {}
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(edges)
}

/// 导入判定：逐条给出「会写入 / 待补全」结论
///
/// 判定依据是**包内实际带了什么**（`ImportContext`），不是「本机现在有没有」：
/// 本机同 id 的凭证与包里引用的凭证是两回事，导入后要不要重绑由用户决定。
fn plan_import(
    dataset: &str,
    records: &[Value],
    context: &ImportContext,
) -> Result<Vec<ImportPlanItem>, String> {
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
fn profile_plan_item(record: &Value, context: &ImportContext) -> Result<ImportPlanItem, String> {
    let profile = decode::<ServerProfile>(record, "服务器档案")?;
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
    Ok(if missing.is_empty() {
        ImportPlanItem::added(DATASET_PROFILES, &profile.id, &profile.name)
    } else {
        ImportPlanItem::pending_reference(
            DATASET_PROFILES,
            &profile.id,
            &profile.name,
            &format!("{} 未随包带出，导入后需重新绑定", missing.join("、")),
        )
    })
}

/// 反序列化回逻辑结构：失败即「结构不认识」，报错而不是跳过（跳过等于静默丢记录）
fn decode<T: DeserializeOwned>(record: &Value, label: &str) -> Result<T, String> {
    serde_json::from_value(record.clone()).map_err(|e| format!("{label}记录结构不认识: {e}"))
}

/// 写入新空间暂存的插件库：建库（沿用插件同一份迁移）→ 逐条写入 → 读回自查
///
/// 只在 `target.root` 之内写文件；读回这一步是契约要求的自查（写坏了的包不能进空间）。
fn apply_to_staging(
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
fn insert_record(conn: &mut Connection, dataset: &str, record: &Value) -> Result<(), String> {
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
fn count_rows(path: &std::path::Path, dataset: &str) -> Result<usize, String> {
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

/// id 与名称必须非空（导入侧靠 id 建引用，靠名称在界面里区分条目）
fn require(label: &str, id: &str, name: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err(format!("{label}记录缺少 id"));
    }
    if name.trim().is_empty() {
        return Err(format!("{label} {id} 缺少名称"));
    }
    Ok(())
}

/// 构造依赖边（`fromId` 引用 `toId`；被引用的数据集由描述符的 `pulls` 按 `kind` 决定）
fn edge(kind: &str, from_id: &str, to_id: &str) -> DependencyEdge {
    DependencyEdge {
        kind: kind.to_string(),
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
    }
}

/// 取非空引用（空串 = 未设置，不产生引用边）
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}

/// 关联命令登记用的数据集名集合（导入侧按顺序落库：先档案，再子记录）
pub(crate) fn dataset_order() -> [&'static str; 4] {
    [
        DATASET_GROUPS,
        DATASET_PROFILES,
        DATASET_TUNNELS,
        DATASET_BOOKMARKS,
    ]
}

/// 供导入侧判断某数据集是否属于本插件
pub(crate) fn owns(dataset: &str) -> bool {
    dataset_order().contains(&dataset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::ssh::models::{AuthMethod, TunnelType};

    /// 造一条档案（默认未绑定凭证、未分组）
    fn profile(id: &str) -> ServerProfile {
        ServerProfile {
            id: id.into(),
            name: format!("档案 {id}"),
            host: "10.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth_method: AuthMethod::Password,
            credential_ref: None,
            group_id: None,
            remark: None,
            last_connected_at: None,
        }
    }

    /// 造一条隧道
    fn tunnel(id: &str, profile_id: &str) -> TunnelConfig {
        TunnelConfig {
            id: id.into(),
            profile_id: profile_id.into(),
            name: format!("隧道 {id}"),
            tunnel_type: TunnelType::Local,
            listen_host: "127.0.0.1".into(),
            listen_port: 8080,
            target_host: Some("127.0.0.1".into()),
            target_port: Some(80),
            auto_start: false,
        }
    }

    /// 导入判定：引用的凭证随包带来 → 会写入；没带来 → 待补全并说明缺什么
    #[test]
    fn plan_import_marks_missing_references() {
        use crate::framework::data_transfer::types::{CarriedBlock, ImportOutcome};
        use std::collections::{BTreeMap, BTreeSet};

        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        let record = serde_json::to_value(&bound).expect("序列化档案");

        let carried = BTreeMap::from([(
            CREDENTIAL_DATASET.to_string(),
            CarriedBlock {
                record_count: 1,
                ids: Some(BTreeSet::from(["cred-1".to_string()])),
            },
        )]);
        let with_credential = ImportContext {
            source_space_id: "default".into(),
            carried: carried.clone(),
        };
        let items =
            plan_import(DATASET_PROFILES, &[record.clone()], &with_credential).expect("判定成功");
        assert_eq!(items[0].outcome, ImportOutcome::Added);
        assert_eq!(items[0].id, "p1");

        // 包内只声明未携带（键存在但 ids 为空集）→ 待补全
        let declared_only = ImportContext {
            source_space_id: "default".into(),
            carried: BTreeMap::from([(
                CREDENTIAL_DATASET.to_string(),
                CarriedBlock {
                    record_count: 1,
                    ids: None,
                },
            )]),
        };
        let items = plan_import(DATASET_PROFILES, &[record], &declared_only).expect("判定成功");
        assert_eq!(items[0].outcome, ImportOutcome::PendingReference);
        assert!(
            items[0].note.as_deref().unwrap_or("").contains("cred-1"),
            "说明里要点名缺了哪条凭证：{:?}",
            items[0].note
        );
    }

    /// 写入暂存库：建库、写入、读回计数一致，且 id 原样保留
    #[test]
    fn apply_to_staging_writes_readable_db() {
        let root = std::env::temp_dir().join(format!("pb-ssh-staging-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data")).expect("建暂存数据目录");
        let target = StagingTarget {
            root: root.clone(),
            space_id: "11111111-1111-4111-8111-111111111111".into(),
            keyring: crate::framework::secure_store::ScopedKeyringStore::new(
                "com.patchy23.patchybox.tests",
            ),
        };

        let group = SshGroup {
            id: "g1".into(),
            name: "生产".into(),
            sort_order: 1,
        };
        let mut bound = profile("p1");
        bound.group_id = Some("g1".into());
        bound.credential_ref = Some("cred-1".into());
        let tunnel = tunnel("t1", "p1");
        let bookmark = SshBookmark {
            id: "b1".into(),
            profile_id: "p1".into(),
            name: "日志".into(),
            path: "/var/log".into(),
            sort: 1,
        };

        let written = |dataset: &str, value: &Value| -> usize {
            apply_to_staging(dataset, std::slice::from_ref(value), &target).expect("写入成功")
        };
        assert_eq!(
            written(
                DATASET_GROUPS,
                &serde_json::to_value(&group).expect("序列化分组")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_PROFILES,
                &serde_json::to_value(&bound).expect("序列化档案")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_TUNNELS,
                &serde_json::to_value(&tunnel).expect("序列化隧道")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_BOOKMARKS,
                &serde_json::to_value(&bookmark).expect("序列化书签")
            ),
            1
        );

        // 读回：条数一致、id 原样保留（导入必须保留传输标识）
        let conn = Connection::open(root.join("data").join("ssh.db")).expect("打开暂存库");
        let profiles = store::profiles::list_profiles(&conn).expect("读档案");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "p1");
        assert_eq!(profiles[0].credential_ref.as_deref(), Some("cred-1"));
        assert_eq!(
            store::profiles::list_groups(&conn).expect("读分组").len(),
            1
        );
        assert_eq!(
            store::tunnels::list_tunnels(&conn, "p1").expect("读隧道")[0].id,
            "t1"
        );
        let bookmarks = store::bookmarks::list_bookmarks(&conn, "p1").expect("读书签");
        assert_eq!(bookmarks[0].id, "b1");
        assert_eq!(bookmarks[0].path, "/var/log");
        assert_eq!(
            count_rows(&root.join("data").join("ssh.db"), DATASET_BOOKMARKS).expect("计数"),
            1
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 目录：档案可勾选，分组/隧道/书签不可勾选且条目数与记录数一致
    #[test]
    fn describe_marks_selectable_dataset_and_follows() {
        let conn = store::open_memory();
        store::profiles::upsert_group(
            &conn,
            &SshGroup {
                id: "g1".into(),
                name: "生产".into(),
                sort_order: 1,
            },
        )
        .expect("插入分组");
        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        bound.group_id = Some("g1".into());
        store::profiles::upsert_profile(&conn, &bound, 1).expect("插入档案");
        store::profiles::upsert_profile(&conn, &profile("p2"), 2).expect("插入档案");
        store::tunnels::upsert_tunnel(&conn, &tunnel("t1", "p1"), 3).expect("插入隧道");
        store::bookmarks::add_bookmark(&conn, "p1", "日志", "/var/log").expect("插入书签");

        let descriptors = describe(&conn).expect("目录生成");
        let by_name = |name: &str| {
            descriptors
                .iter()
                .find(|item| item.name == name)
                .expect("数据集存在")
        };
        let profiles = by_name(DATASET_PROFILES);
        assert!(profiles.selectable);
        assert_eq!(profiles.record_count, 2);
        let entry = profiles
            .entries
            .iter()
            .find(|item| item.id == "p1")
            .expect("档案条目存在");
        assert_eq!(entry.detail, "root@10.0.0.1:22");
        assert!(entry.note.is_none());
        let kinds: Vec<&str> = entry
            .dependencies
            .iter()
            .map(|item| item.kind.as_str())
            .collect();
        assert_eq!(kinds, vec!["credential", "group", "tunnel", "bookmark"]);
        assert_eq!(entry.dependencies[0].from_id, "p1");
        assert_eq!(entry.dependencies[0].to_id, "cred-1");

        // 未绑定凭证的档案在预览里标「待补全」
        let plain = profiles
            .entries
            .iter()
            .find(|item| item.id == "p2")
            .expect("档案条目存在");
        assert_eq!(plain.note.as_deref(), Some(NOTE_NO_CREDENTIAL));
        assert!(plain.dependencies.is_empty());

        assert!(!by_name(DATASET_GROUPS).selectable);
        assert_eq!(by_name(DATASET_GROUPS).record_count, 1);
        assert_eq!(by_name(DATASET_TUNNELS).record_count, 1);
        assert_eq!(by_name(DATASET_BOOKMARKS).record_count, 1);
        // 隧道条目的 detail 是监听地址，书签是远程路径
        assert_eq!(by_name(DATASET_TUNNELS).entries[0].detail, "127.0.0.1:8080");
        assert_eq!(by_name(DATASET_BOOKMARKS).entries[0].detail, "/var/log");
    }

    /// 导出按 id 顺序取记录；缺 id 报错而不是少带
    #[test]
    fn export_follows_id_order_and_rejects_missing() {
        let conn = store::open_memory();
        store::profiles::upsert_profile(&conn, &profile("p1"), 1).expect("插入档案");
        store::profiles::upsert_profile(&conn, &profile("p2"), 2).expect("插入档案");
        store::tunnels::upsert_tunnel(&conn, &tunnel("t1", "p1"), 3).expect("插入隧道");

        let records =
            export(&conn, DATASET_PROFILES, &["p2".into(), "p1".into()]).expect("导出成功");
        assert_eq!(records[0]["id"], serde_json::json!("p2"));
        assert_eq!(records[0]["name"], serde_json::json!("档案 p2"));
        assert_eq!(records[1]["id"], serde_json::json!("p1"));

        // 子记录表跨档案按 id 取
        let tunnels = export(&conn, DATASET_TUNNELS, &["t1".into()]).expect("导出成功");
        assert_eq!(tunnels[0]["profileId"], serde_json::json!("p1"));
        assert_eq!(tunnels[0]["listenPort"], serde_json::json!(8080));

        let missing = export(&conn, DATASET_PROFILES, &["nope".into()]).unwrap_err();
        assert!(missing.contains("不存在记录 nope"), "{missing}");
        // 未绑定凭证的档案可选字段不落空字段（序列化即契约，与前端一致）
        assert!(records[0].get("credentialRef").is_none());
    }

    /// 记录体检：缺主机 / 端口 0 / 缺 id 都被拒，合法记录通过
    #[test]
    fn validate_rejects_incomplete_records() {
        let good = serde_json::to_value(profile("p1")).expect("序列化");
        assert!(validate(DATASET_PROFILES, &[good.clone()]).is_ok());

        let mut no_host = good.clone();
        no_host["host"] = serde_json::json!("  ");
        assert!(validate(DATASET_PROFILES, &[no_host])
            .unwrap_err()
            .contains("缺少主机地址"));

        let mut zero_port = good.clone();
        zero_port["port"] = serde_json::json!(0);
        assert!(validate(DATASET_PROFILES, &[zero_port])
            .unwrap_err()
            .contains("端口非法"));

        let mut no_id = good.clone();
        no_id["id"] = serde_json::json!("");
        assert!(validate(DATASET_PROFILES, &[no_id])
            .unwrap_err()
            .contains("缺少 id"));

        // 结构不认识（认证方式写错）直接报错，不做兜底
        let mut bad_auth = good;
        bad_auth["authMethod"] = serde_json::json!("kerberos");
        assert!(validate(DATASET_PROFILES, &[bad_auth])
            .unwrap_err()
            .contains("结构不认识"));

        let mut bad_bookmark = serde_json::to_value(SshBookmark {
            id: "b1".into(),
            profile_id: "p1".into(),
            name: "日志".into(),
            path: "  ".into(),
            sort: 0,
        })
        .expect("序列化");
        bad_bookmark["path"] = serde_json::json!("");
        assert!(validate(DATASET_BOOKMARKS, &[bad_bookmark])
            .unwrap_err()
            .contains("缺少远程路径"));

        assert!(validate("ssh.known_hosts", &[]).is_err());
    }

    /// 清单依赖边：档案出凭证/分组边，隧道与书签出所属档案边
    #[test]
    fn references_expose_record_level_edges() {
        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        bound.group_id = Some("g1".into());
        let records = vec![serde_json::to_value(&bound).expect("序列化")];
        let edges = references(DATASET_PROFILES, &records).expect("边生成");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].kind, EDGE_CREDENTIAL);
        assert_eq!(edges[0].from_id, "p1");
        assert_eq!(edges[1].kind, EDGE_GROUP);
        assert_eq!(edges[1].to_id, "g1");

        let tunnel_records = vec![serde_json::to_value(tunnel("t1", "p1")).expect("序列化")];
        let tunnel_edges = references(DATASET_TUNNELS, &tunnel_records).expect("边生成");
        assert_eq!(tunnel_edges.len(), 1);
        assert_eq!(tunnel_edges[0].kind, EDGE_PROFILE);
        assert_eq!(tunnel_edges[0].from_id, "t1");
        assert_eq!(tunnel_edges[0].to_id, "p1");

        assert!(references(DATASET_GROUPS, &[]).expect("边生成").is_empty());
    }

    /// 归属判断与落库顺序：分组在档案前、子记录在档案后
    #[test]
    fn owns_and_order_cover_four_datasets() {
        assert!(owns(DATASET_TUNNELS));
        assert!(!owns("ssh.known_hosts"));
        assert_eq!(
            dataset_order().to_vec(),
            vec![
                DATASET_GROUPS,
                DATASET_PROFILES,
                DATASET_TUNNELS,
                DATASET_BOOKMARKS
            ]
        );
    }
}
