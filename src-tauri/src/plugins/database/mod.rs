//! 数据库工作台插件 · 门面（命令薄层 + 插件装配）
//! 命令前缀 `dbc_`（与既有 sqlite 插件的 `db_` 前缀区分；注册表全局唯一）。
//! 结构：models.rs（契约）/ dialect.rs + mysql.rs + postgres.rs + sqlite.rs（方言）/
//! conn.rs（会话注册表）/ catalog.rs（查询与元数据）/ redis.rs（键浏览）/
//! store.rs（本地库）/ secrets.rs（stronghold 凭据）/ agent/（侧车驱动）。

pub(crate) mod agent;
pub(crate) mod catalog;
pub(crate) mod conn;
pub(crate) mod dialect;
pub(crate) mod models;
pub(crate) mod mysql;
pub(crate) mod postgres;
pub(crate) mod redis;
pub(crate) mod secrets;
pub(crate) mod sqlite;
pub(crate) mod store;
use std::collections::HashMap;
use std::sync::Mutex;

use tauri::State;

use crate::plugins::database::models::{ConnConfig, HistoryEntry, SavedEntry};
use crate::plugins::database::store::StoreState;

/// 连接管理命令 ───────────────────────────────────────────────────────────
/// 保存连接配置（含密码 → stronghold；已连接则更新会话配置）
/// 密码留空且连接已存在时保留原密码（编辑模式不修改密码）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connection_save(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    config: ConnConfig,
    password: String,
) -> Result<(), String> {
    let existed = store::list_connections(&app, &store_state)?
        .iter()
        .any(|c| c.id == config.id);
    store::save_connection(&app, &store_state, &config)?;
    if !password.is_empty() || !existed {
        secrets::secret_save(&app, &secrets_state, &config.id, &password)?;
    }
    Ok(())
}

/// 删除连接配置（并清理凭据；会话由前端先断开）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connection_delete(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    id: String,
) -> Result<(), String> {
    store::delete_connection(&app, &store_state, &id)?;
    let _ = secrets::secret_delete(&app, &secrets_state, &id);
    Ok(())
}

/// 连接列表（配置 + 会话状态合并；agent 会话附带存活校验）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connections(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    session_state: State<'_, conn::DbState>,
) -> Result<Vec<models::DbConnectionInfo>, String> {
    let configs = store::list_connections(&app, &store_state)?;
    Ok(conn::snapshot(&session_state, &configs).await)
}

/// 建立连接（读取 stronghold 凭据；成功后探测版本与时延）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connect(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    session_state: State<'_, conn::DbState>,
    runtimes: State<'_, conn::AgentRuntimeState>,
    id: String,
) -> Result<models::DbConnectionInfo, String> {
    let configs = store::list_connections(&app, &store_state)?;
    let config = configs
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| format!("连接配置不存在：{id}"))?;
    let password = secrets::secret_get(&app, &secrets_state, &id)?;
    let entry = conn::connect(&app, &session_state, &runtimes, &config, &password).await?;
    Ok(entry.to_info())
}

/// 断开连接
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_disconnect(
    session_state: State<'_, conn::DbState>,
    runtimes: State<'_, conn::AgentRuntimeState>,
    id: String,
) -> Result<(), String> {
    conn::disconnect(&session_state, &runtimes, &id).await
}

/// 测试连接（不保存、不落会话；返回版本信息）
/// 密码为空且该连接已存在时，使用 stronghold 已保存密码（编辑模式「留空=不修改」对称语义）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_test(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    runtimes: State<'_, conn::AgentRuntimeState>,
    config: ConnConfig,
    password: String,
) -> Result<String, String> {
    let password = if password.is_empty() {
        let existed = store::list_connections(&app, &store_state)?
            .iter()
            .any(|c| c.id == config.id);
        if existed {
            secrets::secret_get(&app, &secrets_state, &config.id)?
        } else {
            password
        }
    } else {
        password
    };
    conn::test_connection(&app, &runtimes, &config, &password).await
}

/// 查询历史与收藏 ─────────────────────────────────────────────────────────
/// 历史列表
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_history(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
) -> Result<Vec<HistoryEntry>, String> {
    store::list_history(&app, &store_state)
}

/// 追加历史
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_history_add(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    conn_id: String,
    sql: String,
    status: String,
    duration_ms: u64,
) -> Result<(), String> {
    store::add_history(&app, &store_state, &conn_id, &sql, &status, duration_ms)
}

/// 清空历史
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_history_clear(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
) -> Result<(), String> {
    store::clear_history(&app, &store_state)
}

/// 收藏列表
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_saved(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
) -> Result<Vec<SavedEntry>, String> {
    store::list_saved(&app, &store_state)
}

/// 添加收藏（返回新记录 id）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_saved_add(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    title: String,
    sql: String,
) -> Result<i64, String> {
    store::add_saved(&app, &store_state, &title, &sql)
}

/// 更新收藏（SQL 编辑器二次保存）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_saved_update(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    id: i64,
    title: String,
    sql: String,
) -> Result<(), String> {
    store::update_saved(&app, &store_state, id, &title, &sql)
}

/// 删除收藏
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_saved_delete(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    id: i64,
) -> Result<(), String> {
    store::delete_saved(&app, &store_state, id)
}

/// 驱动诊断：agent 驱动是否就绪（前端提示如何获取驱动）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_driver_status(
    app: tauri::AppHandle,
    db_type: String,
) -> Result<serde_json::Value, String> {
    let db_type =
        models::DbType::parse(&db_type).ok_or_else(|| format!("未知数据库类型：{db_type}"))?;
    if !db_type.is_agent() {
        return Ok(serde_json::json!({ "ready": true, "kind": "native" }));
    }
    let store = agent::DriverStore::new(&app)?;
    let ready = store.agent_binary(db_type).is_some();
    let dir = store.driver_dir(db_type)?;
    let versions = store.versions();
    let version = versions.get(agent::driver_key(db_type)).cloned();
    Ok(serde_json::json!({
        "ready": ready,
        "kind": "agent",
        "dir": dir.display().to_string(),
        "version": version,
        "note": "agent 驱动需手动放置或配置镜像（设置 database.agentMirror，模板 {type}/{version}）",
    }))
}

/// 插件命令分派（应用级总 handler 按前缀路由到本函数）
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        dbc_connection_save,
        dbc_connection_delete,
        dbc_connections,
        dbc_connect,
        dbc_disconnect,
        dbc_test,
        dbc_history,
        dbc_history_add,
        dbc_history_clear,
        dbc_saved,
        dbc_saved_add,
        dbc_saved_update,
        dbc_saved_delete,
        dbc_driver_status,
        catalog::dbc_execute,
        catalog::dbc_cancel,
        catalog::dbc_databases,
        catalog::dbc_schemas,
        catalog::dbc_objects,
        catalog::dbc_columns,
        catalog::dbc_table_data,
        catalog::dbc_explain,
        catalog::dbc_export_csv,
        catalog::dbc_redis_keys,
        catalog::dbc_redis_key_info,
    ];
    handler(invoke)
}

/// 插件注册：命令入库 + 全部 State 装配
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        (
            "dbc_connection_save",
            "保存数据库连接配置（密码进 stronghold）",
        ),
        ("dbc_connection_delete", "删除数据库连接配置与凭据"),
        ("dbc_connections", "连接列表（配置 + 会话状态）"),
        ("dbc_connect", "建立数据库连接（探测版本与时延）"),
        ("dbc_disconnect", "断开数据库连接"),
        ("dbc_test", "测试数据库连接（不保存）"),
        ("dbc_history", "查询历史列表"),
        ("dbc_history_add", "追加查询历史"),
        ("dbc_history_clear", "清空查询历史"),
        ("dbc_saved", "收藏 SQL 列表"),
        ("dbc_saved_add", "添加收藏 SQL"),
        ("dbc_saved_update", "更新收藏 SQL（编辑器二次保存）"),
        ("dbc_saved_delete", "删除收藏 SQL"),
        ("dbc_driver_status", "agent 驱动就绪状态（含目录指引）"),
        ("dbc_execute", "执行 SQL（多语句拆分，查询返回表格）"),
        ("dbc_cancel", "取消进行中的查询"),
        ("dbc_databases", "数据库列表"),
        ("dbc_schemas", "schema 列表"),
        ("dbc_objects", "对象列表（表/视图等）"),
        ("dbc_columns", "表结构列信息"),
        ("dbc_table_data", "表数据分页浏览"),
        ("dbc_explain", "执行计划"),
        ("dbc_export_csv", "导出 CSV 文件（结果集导出）"),
        ("dbc_redis_keys", "Redis 键列表（SCAN）"),
        ("dbc_redis_key_info", "Redis 键信息（TYPE/TTL/预览）"),
    ])
    .expect("IPC 命令重复注册");
    builder
        .manage(conn::DbState(Mutex::new(HashMap::new())))
        .manage(conn::DbCancelState(Mutex::new(HashMap::new())))
        .manage(conn::AgentRuntimeState(Mutex::new(HashMap::new())))
        .manage(StoreState(Mutex::new(None)))
        .manage(secrets::SecretsState(Mutex::new(None)))
}
