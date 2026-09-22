//! SQL 工作区执行、后端风险确认及取消；执行目标和资源归属由后端校验。
use crate::plugins::database::{
    drivers::{
        self,
        workspace::{self, WorkspaceConnection},
        CancelHandle, DbCancelState, DbState, WorkspaceState,
    },
    models::{ExecutionScope, QueryResult},
    secrets::{self, SecretsState},
    sql_analysis,
    store::{self, StoreState},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    sync::{atomic::Ordering, Arc, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::State;

fn config_for(
    app: &tauri::AppHandle,
    store: &State<'_, StoreState>,
    conn_id: &str,
) -> Result<crate::plugins::database::models::ConnConfig, String> {
    store::list_connections(app, store)?
        .into_iter()
        .find(|c| c.id == conn_id)
        .ok_or_else(|| "DB_CONNECTION_MISSING: 连接配置不存在".into())
}
fn signature(
    config: &crate::plugins::database::models::ConnConfig,
    scope: &ExecutionScope,
    sql: &str,
    request: &str,
    timestamp: u64,
) -> Result<Vec<u8>, String> {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    let key = KEY.get_or_init(|| {
        let mut key = [0; 32];
        key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        key
    });
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|e| e.to_string())?;
    let payload =
        serde_json::to_vec(&(config, scope, sql, request, timestamp)).map_err(|e| e.to_string())?;
    mac.update(&payload);
    Ok(mac.finalize().into_bytes().to_vec())
}
fn now_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}
fn ticket(
    config: &crate::plugins::database::models::ConnConfig,
    scope: &ExecutionScope,
    sql: &str,
    request: &str,
) -> Result<String, String> {
    let now = now_seconds()?;
    let mac = signature(config, scope, sql, request, now)?;
    Ok(format!(
        "{now}:{}",
        mac.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
fn check_ticket(
    token: &str,
    config: &crate::plugins::database::models::ConnConfig,
    scope: &ExecutionScope,
    sql: &str,
    request: &str,
) -> Result<(), String> {
    let Some((time, digest)) = token.split_once(':') else {
        return Err("DB_RISK_CONFIRM_REQUIRED: 请重新确认执行目标和 SQL".into());
    };
    let time: u64 = time.parse().map_err(|_| "确认凭据格式无效")?;
    let now = now_seconds()?;
    if time > now || now - time > 120 {
        return Err("DB_RISK_CONFIRM_REQUIRED: 确认已过期".into());
    }
    let expected = signature(config, scope, sql, request, time)?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if expected != digest {
        return Err("DB_RISK_CONFIRM_REQUIRED: SQL 或连接配置已变化，请重新确认".into());
    }
    Ok(())
}

/// 执行预检；确认凭据绑定原文、连接配置、作用域及请求，不能由前端布尔值替代。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_prepare_execution(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    conn_id: String,
    sql: String,
    request_id: String,
    scope: Option<ExecutionScope>,
) -> Result<serde_json::Value, String> {
    let config = config_for(&app, &store_state, &conn_id)?;
    let scope = scope.unwrap_or_default();
    let (write, dangerous) = sql_analysis::assess(config.db_type, &sql)?;
    if config.readonly && write {
        return Err("DB_READ_ONLY: 当前连接只读，拒绝写入或无法确定安全性的语句".into());
    }
    Ok(serde_json::json!({
        "requiresConfirmation": dangerous,
        "confirmationToken": if dangerous { Some(ticket(&config, &scope, &sql, &request_id)?) } else { None },
        "target": format!("{} / {} / {}", config.label, scope.database, scope.schema),
        "summary": if dangerous { "包含结构变更、批量删除或无法静态确认影响的操作；请核对完整 SQL。" } else { "" }
    }))
}

struct Registration<'a> {
    state: &'a DbCancelState,
    id: &'a str,
    handle: CancelHandle,
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.handle.finished.store(true, Ordering::Release);
        if let Ok(mut map) = self.state.0.lock() {
            map.remove(self.id);
        }
    }
}

/// SQL 页级执行；结束前持有独占 lane，避免取消与下一次执行交错。
#[tauri::command(rename_all = "camelCase")]
#[allow(clippy::too_many_arguments)]
pub async fn dbc_execute(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    cancel_state: State<'_, DbCancelState>,
    workspaces: State<'_, WorkspaceState>,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, SecretsState>,
    conn_id: String,
    sql: String,
    max_rows: Option<u64>,
    request_id: String,
    workspace_id: Option<String>,
    scope: Option<ExecutionScope>,
    confirmation_token: Option<String>,
) -> Result<QueryResult, String> {
    let log_started = std::time::Instant::now();
    let mut log_cancelled = false;
    let result: Result<QueryResult, String> = async {
    if request_id.trim().is_empty() {
        return Err("执行请求缺少请求标识".into());
    }
    let mut entry = state.entry(&conn_id)?;
    let config = config_for(&app, &store_state, &conn_id)?;
    if config.db_type != entry.config.db_type {
        return Err("连接类型已修改，请重新连接".into());
    }
    entry.config = config;
    let scope = scope.unwrap_or_else(|| ExecutionScope {
        database: entry.config.database.clone(),
        schema: String::new(),
    });
    let (write, dangerous) = sql_analysis::assess(entry.config.db_type, &sql)?;
    if entry.config.readonly && write {
        return Err("DB_READ_ONLY: 当前连接只读，拒绝写操作".into());
    }
    if dangerous {
        check_ticket(
            confirmation_token.as_deref().unwrap_or(""),
            &entry.config,
            &scope,
            &sql,
            &request_id,
        )?;
    }
    let workspace_id = workspace_id.as_deref().unwrap_or(&request_id);
    let slot = workspaces.slot(&conn_id, workspace_id)?;
    let mut slot = slot
        .try_lock()
        .map_err(|_| "DB_SESSION_BUSY: 当前页签正在执行或关闭")?;
    let mut handle = CancelHandle::pending();
    handle.connection_id = conn_id.clone();
    handle.workspace_id = workspace_id.to_string();
    {
        let mut map = cancel_state.0.lock().map_err(|e| e.to_string())?;
        if map.contains_key(&request_id) {
            return Err("执行请求标识重复".into());
        }
        map.insert(request_id.clone(), handle.clone());
    }
    let registration = Registration {
        state: &cancel_state,
        id: &request_id,
        handle: handle.clone(),
    };
    if slot
        .as_ref()
        .is_some_and(|session| session.scope != scope || session.config != entry.config)
    {
        if slot.as_ref().is_some_and(|session| session.transaction) {
            return Err("DB_TRANSACTION_ACTIVE: 请先提交或回滚，再切换执行目标".into());
        }
        if let Some(old) = slot.take() {
            workspace::close(old).await?;
        }
    }
    if slot.is_none() {
        let password = secrets::secret_get(&app, &secrets_state, &conn_id)?;
        *slot = Some(
            tokio::time::timeout(
                Duration::from_millis(entry.config.connect_timeout_ms.clamp(1000, 120_000)),
                workspace::open(&entry, scope, &password),
            )
            .await
            .map_err(|_| "DB_TIMEOUT: 建立工作会话超时")??,
        );
    }
    if !state.is_current(&entry)? {
        if let Some(session) = slot.take() {
            workspace::close(session).await?;
        }
        return Err("DB_CONNECTION_SUPERSEDED: 连接已断开或重新配置，SQL 未执行".into());
    }
    let session = slot.as_mut().ok_or("工作会话初始化失败")?;
    session.ambiguous_edit_source |= sql_analysis::may_change_resolution(entry.config.db_type, &sql);
    match &session.connection {
        WorkspaceConnection::Mysql(conn, pool) => {
            handle.mysql_thread_id = Some(conn.id());
            handle.mysql_pool = Some(pool.clone());
        }
        WorkspaceConnection::Postgres(client) => {
            handle.pg_cancel = Some(client.cancel_token());
            handle.pg_ssl = entry.config.ssl;
        }
        WorkspaceConnection::Sqlite(conn) => {
            handle.sqlite = Some(Arc::new(
                conn.lock()
                    .map_err(|e| e.to_string())?
                    .get_interrupt_handle(),
            ));
        }
        WorkspaceConnection::Agent(client, id) => {
            handle.agent = Some((Arc::clone(client), id.clone()));
        }
        WorkspaceConnection::Redis(_) => {}
    }
    cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(request_id.clone(), handle.clone());
    if handle.aborted.load(Ordering::Acquire) {
        return Err("DB_CANCELLED: 已取消，语句尚未发往服务器".into());
    }
    let limit = max_rows.unwrap_or(1000).clamp(1, 100_000);
    let started = Instant::now();
    let execution = async {
        match &mut session.connection {
            WorkspaceConnection::Mysql(conn, _) => {
                drivers::mysql::execute_mysql_conn(conn, &sql, limit).await
            }
            WorkspaceConnection::Postgres(client) => {
                drivers::postgres::execute_postgres_client(client, &sql, limit).await
            }
            WorkspaceConnection::Sqlite(conn) => {
                let conn = Arc::clone(conn);
                let text = sql.clone();
                tokio::task::spawn_blocking(move || {
                    drivers::sqlite::execute_sqlite(&conn, &text, limit)
                })
                .await
                .map_err(|e| e.to_string())?
            }
            WorkspaceConnection::Agent(client, id) => {
                drivers::execute_agent(client, id, &sql, limit, entry.config.db_type).await
            }
            WorkspaceConnection::Redis(manager) => drivers::redis::exec_command(manager, &sql)
                .await
                .map(|value| {
                    let mut result = QueryResult::empty();
                    result.columns = vec!["result".into()];
                    result.rows = vec![vec![value.clone()]];
                    result.values = vec![vec![crate::plugins::database::models::DbValue::text(
                        "text", value,
                    )]];
                    result.is_query = true;
                    result
                }),
        }
    };
    let (outcome, timed_out) = {
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => (result, false),
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                let cancellation = tokio::time::timeout(Duration::from_secs(5), handle.cancel()).await;
                let finish = tokio::time::timeout(Duration::from_secs(5), &mut execution).await;
                let message = if matches!(cancellation, Ok(Ok(()))) && finish.is_ok() {
                    "DB_TIMEOUT: 查询超时，执行通道已结束；请核对脚本中已完成的操作"
                } else { "DB_OUTCOME_UNKNOWN: 查询超时，无法确认服务器结果；请勿自动重试写操作" };
                (Err(message.to_string()), true)
            }
        }
    };
    // 与 cancel RPC 同步收尾；finished 置位后旧句柄不能影响后继查询。
    let gate = handle.gate.lock().await;
    handle.finished.store(true, Ordering::Release);
    log_cancelled = handle.aborted.load(Ordering::Acquire) && !timed_out;
    drop(gate);
    let result = outcome.map(|mut result| {
        if let Ok(parts) = sql_analysis::split(entry.config.db_type, &sql) {
            if result.statements.is_empty() {
                if result.ok {
                    session.transaction = sql_analysis::transaction_after(
                        entry.config.db_type,
                        &sql,
                        session.transaction,
                    );
                }
            } else {
                for (index, outcome) in result.statements.iter().enumerate() {
                    let Some(part) = parts.get(outcome.statement_index.unwrap_or(index)) else {
                        break;
                    };
                    if !outcome.ok {
                        break;
                    }
                    session.transaction = sql_analysis::transaction_after(
                        entry.config.db_type,
                        part,
                        session.transaction,
                    );
                }
            }
        }
        result.transaction_active = session.transaction;
        if result.ok && result.is_query && result.statements.is_empty() && !session.transaction && !entry.config.readonly && !session.ambiguous_edit_source {
            result.edit_target = sql_analysis::edit_target(entry.config.db_type, &sql, &conn_id, &session.scope, &entry.config.database);
        }
        result.duration_ms = started.elapsed().as_millis() as u64;
        result
    });
    if timed_out {
        if let Some(old) = slot.take() {
            workspace::close(old).await?;
        }
    }
    drop(registration);
    result
    }.await;
    match &result {
        _ if log_cancelled => log::info!("查询取消后收尾 request={request_id}"),
        Ok(value) if value.ok => log::info!(
            "查询完成 request={request_id} elapsed_ms={} rows={} transaction={}",
            log_started.elapsed().as_millis(),
            value.rows.len(),
            value.transaction_active
        ),
        Err(error) if error.starts_with("DB_CANCELLED:") => {
            log::info!("查询已取消 request={request_id}")
        }
        Ok(_) => log::error!(
            "查询执行失败 request={request_id} elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "查询未完成 request={request_id} elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 请求取消；原生驱动发送真实请求，Redis 不能撤销已发送命令。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_cancel(state: State<'_, DbCancelState>, request_id: String) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        let handle = state
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get(request_id.trim())
            .cloned();
        if let Some(handle) = handle {
            tokio::time::timeout(Duration::from_secs(5), handle.cancel())
                .await
                .map_err(|_| "取消请求超时，原查询可能仍在运行")??;
        }
        Ok(())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_cancel elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_cancel elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 关闭内部 SQL 页签的独占会话；不触碰其他页签或目录连接池。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_workspace_close(
    workspaces: State<'_, WorkspaceState>,
    conn_id: String,
    workspace_id: String,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        let slot = workspaces
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&(conn_id, workspace_id));
        if let Some(slot) = slot {
            let mut session = slot.lock().await;
            if let Some(session) = session.take() {
                workspace::close(session).await?;
            }
        }
        Ok(())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_workspace_close elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_workspace_close elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 导出已加载 CSV；异步 IO 不阻塞 tokio 执行线程。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_export_csv(path: String, text: String) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        if path.trim().is_empty() {
            return Err("保存路径不能为空".into());
        }
        tokio::fs::write(&path, text)
            .await
            .map_err(|e| format!("CSV 写入失败: {e}"))
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_export_csv elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_export_csv elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
