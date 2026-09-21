//! 数据库工作台插件 · 门面（命令薄层 + 插件装配）
//! 命令前缀 `dbc_`（与既有 sqlite 插件的 `db_` 前缀区分；注册表全局唯一）。
//! 结构：models.rs（契约）/ dialect/（方言纯函数）/ drivers/（会话注册表 + 驱动执行）/
//! catalog/（查询执行、元数据、分页与 Redis 命令）/ store.rs（本地库）/ secrets.rs（公共 Vault 凭据）/ agent/（侧车驱动）。
//! close_hooks.rs（退出清理：取消查询 → 结束 agent 子进程 → 清会话注册表）。

pub(crate) mod admin;
pub(crate) mod agent;
pub(crate) mod catalog;
pub(crate) mod close_hooks;
mod credential_refs;
pub(crate) mod dialect;
mod drafts;
pub(crate) mod drivers;
#[cfg(test)]
mod execution_tests;
pub(crate) mod files;
pub(crate) mod models;
pub(crate) mod results;
pub(crate) mod secrets;
pub(crate) mod sql_analysis;
pub(crate) mod store;
mod transfer;
use std::collections::HashMap;
use std::sync::Mutex;

use tauri::State;

use crate::plugins::database::models::{ConnConfig, HistoryEntry, SavedEntry};
use crate::plugins::database::store::StoreState;

/// 连接管理命令 ───────────────────────────────────────────────────────────
/// 保存连接配置（含密码 → 公共 Vault；已连接则更新会话配置）
/// 密码留空且连接已存在时保留原密码（编辑模式不修改密码）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connection_save(
    session_state: State<'_, drivers::DbState>,
    runtimes: State<'_, drivers::AgentRuntimeState>,
    workspaces: State<'_, drivers::WorkspaceState>,
    cancellations: State<'_, drivers::DbCancelState>,
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    mut config: ConnConfig,
    password: String,
    clear_password: Option<bool>,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        let existed = store::list_connections(&app, &store_state)?
            .iter()
            .any(|c| c.id == config.id);
        session_state.next_generation(&config.id)?;
        drivers::workspace::close_connection(&workspaces, &cancellations, &config.id).await?;
        drivers::disconnect(&session_state, &runtimes, &config.id).await?;
        let _credential_lock = secrets::CONFIG_CREDENTIAL_LOCK
            .lock()
            .map_err(|e| e.to_string())?;
        if clear_password.unwrap_or(false) || config.db_type.is_sqlite() {
            config.credential_id = None;
        }
        let password = if clear_password.unwrap_or(false) || config.db_type.is_sqlite() {
            String::new()
        } else if let Some(password) = secrets::referenced_password(&app, &mut config)? {
            password
        } else {
            password
        };
        if config.credential_id.is_none() && !password.is_empty() {
            config.credential_id = Some(secrets::import_password(&app, &config, &password)?);
        }
        let update_secret = clear_password.unwrap_or(false)
            || !password.is_empty()
            || !existed
            || transfer::credential_pending(&app, &config.id)?;
        let previous = if update_secret {
            secrets::secret_snapshot(&app, &secrets_state, &config.id)?
        } else {
            None
        };
        if update_secret {
            secrets::secret_save(&app, &secrets_state, &config.id, &password)?;
        }
        if let Err(error) = store::save_connection(&app, &store_state, &config) {
            if update_secret {
                let rollback = if let Some(previous) = previous {
                    secrets::secret_save(&app, &secrets_state, &config.id, &previous)
                } else {
                    secrets::secret_delete(&app, &secrets_state, &config.id)
                };
                rollback.map_err(|failure| format!("{error}；恢复原凭据失败：{failure}"))?;
            }
            return Err(error);
        }
        if update_secret || config.credential_id.is_some() {
            transfer::credential_saved(&app, &config.id)?;
        }
        Ok(())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_connection_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_connection_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 删除连接配置（并清理凭据；会话由前端先断开）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connection_delete(
    session_state: State<'_, drivers::DbState>,
    runtimes: State<'_, drivers::AgentRuntimeState>,
    workspaces: State<'_, drivers::WorkspaceState>,
    cancellations: State<'_, drivers::DbCancelState>,
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    id: String,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        session_state.next_generation(&id)?;
        drivers::workspace::close_connection(&workspaces, &cancellations, &id).await?;
        drivers::disconnect(&session_state, &runtimes, &id).await?;
        store::delete_connection(&app, &store_state, &id)?;
        secrets::secret_delete(&app, &secrets_state, &id)
            .map_err(|e| format!("连接配置已删除，但凭据清理失败：{e}"))?;
        Ok(())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_connection_delete elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_connection_delete elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 连接列表（配置 + 会话状态合并；agent 会话附带存活校验）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connections(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    session_state: State<'_, drivers::DbState>,
    secrets_state: State<'_, secrets::SecretsState>,
) -> Result<Vec<models::DbConnectionInfo>, String> {
    let configs = secrets::migrate_references(&app, &secrets_state, &store_state)?;
    Ok(drivers::snapshot(&session_state, &configs).await)
}

/// 建立连接（读取 公共 Vault 凭据；成功后探测版本与时延）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_connect(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    session_state: State<'_, drivers::DbState>,
    runtimes: State<'_, drivers::AgentRuntimeState>,
    id: String,
) -> Result<models::DbConnectionInfo, String> {
    let log_started = std::time::Instant::now();
    let result: Result<models::DbConnectionInfo, String> = async {
        let configs = store::list_connections(&app, &store_state)?;
        let config = configs
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| format!("连接配置不存在：{id}"))?;
        let password = secrets::secret_get(&app, &secrets_state, &id)?;
        let timeout =
            std::time::Duration::from_millis(config.connect_timeout_ms.clamp(1000, 120_000) + 5000);
        let entry = tokio::time::timeout(
            timeout,
            drivers::connect(&app, &session_state, &runtimes, &config, &password),
        )
        .await
        .map_err(|_| "数据库连接超时，未建立工作会话")??;
        Ok(entry.to_info())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_connect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_connect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 断开连接
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_disconnect(
    workspaces: State<'_, drivers::WorkspaceState>,
    cancellations: State<'_, drivers::DbCancelState>,
    session_state: State<'_, drivers::DbState>,
    runtimes: State<'_, drivers::AgentRuntimeState>,
    id: String,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        session_state.next_generation(&id)?;
        let cleanup = drivers::workspace::close_connection(&workspaces, &cancellations, &id).await;
        let disconnect = drivers::disconnect(&session_state, &runtimes, &id).await;
        cleanup.and(disconnect)
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_disconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_disconnect elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 测试连接（不保存、不落会话；返回版本信息）
/// 密码为空且该连接已存在时，使用 公共 Vault 已保存密码（编辑模式「留空=不修改」对称语义）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_test(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, secrets::SecretsState>,
    runtimes: State<'_, drivers::AgentRuntimeState>,
    mut config: ConnConfig,
    password: String,
    clear_password: Option<bool>,
) -> Result<String, String> {
    let log_started = std::time::Instant::now();
    let result: Result<String, String> = async {
        let password = if clear_password.unwrap_or(false) {
            String::new()
        } else if let Some(password) = secrets::referenced_password(&app, &mut config)? {
            password
        } else if password.is_empty() && !config.db_type.is_sqlite() {
            let existed = store::list_connections(&app, &store_state)?
                .iter()
                .any(|c| c.id == config.id);
            if existed {
                let saved = secrets::secret_get(&app, &secrets_state, &config.id)?;

                saved
            } else {
                String::new()
            }
        } else {
            password
        };
        drivers::test_connection(&app, &runtimes, &config, &password).await
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_test elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_test elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
    scope: Option<models::ExecutionScope>,
) -> Result<(), String> {
    store::add_history(
        &app,
        &store_state,
        &conn_id,
        &sql,
        &status,
        duration_ms,
        &scope.unwrap_or_default(),
    )
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
        "note": "agent 驱动需手动放置到上述目录（大文件不随安装包分发）",
    }))
}

// 模块静态清单：命令名、入库元数据与分派 handler 同源生成（AR07 §10.2）
crate::covekit_module! {
    owner: "database",
    feature: "database",
    commands: {
        dbc_connection_delete => "删除数据库连接配置与凭据",
        dbc_connections => "连接列表（配置 + 会话状态）",
        dbc_connect => "建立数据库连接（探测版本与时延）",
        dbc_disconnect => "断开数据库连接",
        dbc_test => "测试数据库连接（不保存）",
        drafts::dbc_drafts => "读取 SQL 恢复草稿",
        drafts::dbc_drafts_save => "保存 SQL 恢复草稿",
        dbc_history => "查询历史列表",
        dbc_history_add => "追加查询历史",
        dbc_history_clear => "清空查询历史",
        dbc_saved => "收藏 SQL 列表",
        dbc_saved_add => "添加收藏 SQL",
        dbc_saved_update => "更新收藏 SQL（编辑器二次保存）",
        dbc_saved_delete => "删除收藏 SQL",
        dbc_driver_status => "agent 驱动就绪状态（含目录指引）",
        catalog::query::dbc_execute => "执行 SQL（多语句拆分，查询返回表格）",
        catalog::query::dbc_cancel => "取消进行中的查询",
        catalog::query::dbc_prepare_execution => "SQL 风险与执行目标预检",
        catalog::query::dbc_workspace_close => "关闭 SQL 页签独占会话",
        catalog::metadata::dbc_databases => "数据库列表",
        catalog::metadata::dbc_schemas => "schema 列表",
        catalog::metadata::dbc_objects => "对象列表（表/视图等）",
        catalog::metadata::dbc_columns => "表结构列信息",
        catalog::export::dbc_export_query => "完整查询结果流式导出",
        catalog::csv_import::dbc_csv_preview => "预览 CSV 文件与列映射",
        catalog::csv_import::dbc_csv_import => "事务导入 CSV 文件",
        catalog::mutation::dbc_table_apply => "提交单表行变更",
        catalog::table::dbc_table_count => "按当前筛选精确统计",
        catalog::table::dbc_table_data => "表数据分页浏览",
        catalog::query::dbc_export_csv => "导出 CSV 文件（结果集导出）",
        files::dbc_sql_file_read => "打开 SQL 文件",
        files::dbc_sql_file_write => "保存 SQL 文件",
        files::dbc_export_rows => "导出类型化结果 CSV",
        catalog::redis::dbc_redis_keys => "Redis 键列表（SCAN）",
        catalog::redis::dbc_redis_key_info => "Redis 键信息（TYPE/TTL/预览）",
        admin::dbc_charset_options => "字符集与排序规则选项（建库对话框）",
        admin::dbc_users => "数据库用户清单（授权选择）",
        admin::dbc_create_database => "新建数据库（含可选分步授权）",
        admin::dbc_drop_database => "删除数据库（前端确认后调用）",
        admin::dbc_table_admin => "表维护（重命名/清空/删除）",
        admin::dbc_table_ddl => "表 DDL 查看",
        admin::dbc_table_indexes => "表索引清单",
        dbc_connection_save => "保存连接配置及公共凭证引用，旧会话失效",
    },
}

/// 插件注册：命令入库 + 全部 State 装配 + 关闭清理登记
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    transfer::register();
    credential_refs::register_provider();
    // 关闭清理登记（AR06）：查询取消、agent 子进程与连接会话由本模块自己清，
    // 框架只协调、超时与汇总（没有这一步，父进程退出后 agent 会变成孤儿进程）
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::exit_only(IPC_OWNER)
            .with_dispose(close_hooks::on_dispose),
    );
    builder
        .manage(drivers::DbState(
            Mutex::new(HashMap::new()),
            Mutex::new(HashMap::new()),
        ))
        .manage(drivers::DbCancelState(Mutex::new(HashMap::new())))
        .manage(drivers::WorkspaceState::default())
        .manage(drivers::AgentRuntimeState(Mutex::new(HashMap::new())))
        .manage(StoreState(Mutex::new(None)))
        .manage(secrets::SecretsState(Mutex::new(None)))
}
