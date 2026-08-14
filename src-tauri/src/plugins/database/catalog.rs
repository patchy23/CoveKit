//! 元数据与查询执行命令（catalog / execute / cancel / explain / redis 键浏览）
//! 每个命令先取会话再分派驱动；单元格值统一字符串化（NULL → "NULL"，
//! 二进制 → <blob N bytes>）；查询取消按驱动能力实现（pg CancelToken /
//! mysql KILL QUERY / agent cancel_session / 其余仅标志位）。

use std::sync::atomic::Ordering;
use std::time::Instant;

use mysql_async::prelude::Queryable;
use mysql_async::Value as MysqlValue;
use tauri::State;

use crate::plugins::database::agent::AgentClient;
use crate::plugins::database::conn::{
    CancelHandle, DbCancelState, DbSession, DbSessionEntry, DbState,
};
use crate::plugins::database::dialect::dialect_for;
use crate::plugins::database::models::{DbColumnInfo, DbObjectInfo, DbTablePage, QueryResult};
use crate::plugins::database::redis;

/// 默认查询行数上限（防止大表拖垮 UI）
const DEFAULT_MAX_ROWS: u64 = 1000;

/// 取会话条目（连接不存在报错）
fn session(state: &State<'_, DbState>, conn_id: &str) -> Result<DbSessionEntry, String> {
    state.entry(conn_id)
}

// ──────────────────────────────────────────────────────────────────────────
// 查询执行
// ──────────────────────────────────────────────────────────────────────────

/// 执行 SQL（多语句拆分逐条执行；查询语句返回最后一条结果，非查询累计影响行数）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_execute(
    state: State<'_, DbState>,
    cancel_state: State<'_, DbCancelState>,
    conn_id: String,
    sql: String,
    max_rows: Option<u64>,
) -> Result<QueryResult, String> {
    let entry = session(&state, &conn_id)?;
    let started = Instant::now();
    let limit = max_rows.unwrap_or(DEFAULT_MAX_ROWS).max(1);

    // Redis 会话：查询页签执行的是 Redis 命令
    if let DbSession::Redis(mgr) = &entry.session {
        let mut mgr = mgr.clone();
        let value = redis::exec_command(&mut mgr, &sql).await?;
        return Ok(QueryResult {
            ok: true,
            columns: vec!["result".to_string()],
            rows: vec![vec![value]],
            rows_affected: 0,
            is_query: true,
            duration_ms: started.elapsed().as_millis() as u64,
            truncated: false,
            error: None,
        });
    }

    // 注册取消句柄（各驱动能力不同）
    let cancel = build_cancel_handle(&entry);
    let aborted = cancel.aborted.clone();
    let _ = cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(conn_id.clone(), cancel);

    let result = match &entry.session {
        DbSession::Mysql(pool) => execute_mysql(pool, &sql, limit).await,
        DbSession::Postgres(pool) => execute_postgres(pool, &sql, limit).await,
        DbSession::Sqlite(conn) => execute_sqlite(conn, &sql, limit),
        DbSession::Redis(_) => unreachable!("redis 已在上方处理"),
        DbSession::Agent { client, session_id } => {
            execute_agent(client, session_id, &sql, limit).await
        }
    };

    let _ = cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&conn_id);
    let _ = aborted.load(Ordering::Relaxed);
    let mut result = result?;
    result.duration_ms = started.elapsed().as_millis() as u64;
    Ok(result)
}

/// 取消进行中的查询（按驱动能力：pg cancel_token / mysql KILL QUERY / agent cancel_session）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_cancel(state: State<'_, DbCancelState>, conn_id: String) -> Result<(), String> {
    let handle = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&conn_id)
        .ok_or("没有进行中的查询")?;
    handle.aborted.store(true, Ordering::Relaxed);
    if let Some(token) = &handle.pg_cancel {
        // PG：独立连接发送取消请求（TLS 时用 rustls 连接器）
        if handle.pg_ssl {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let tls_config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);
            let _ = token.cancel_query(tls).await;
        } else {
            let _ = token.cancel_query(tokio_postgres::NoTls).await;
        }
    }
    if let (Some(thread_id), Some((host, port, user, pass))) =
        (&handle.mysql_thread_id, &handle.mysql_conn)
    {
        // MySQL：新建连接发送 KILL QUERY（旧连接正在被查询占用）
        let opts = mysql_async::OptsBuilder::default()
            .ip_or_hostname(host.clone())
            .tcp_port(*port)
            .user(Some(user.clone()))
            .pass(Some(pass.clone()))
            .prefer_socket(false);
        // Conn::new 直接接受 Into<Opts>，无需手动转换
        if let Ok(mut kill_conn) = mysql_async::Conn::new(opts).await {
            let _ = kill_conn
                .query_drop(format!("KILL QUERY {thread_id}"))
                .await;
        }
    }
    if let Some((client, session_id)) = &handle.agent {
        let _ = client.cancel_session(session_id).await;
    }
    Ok(())
}

/// 构建取消句柄（按驱动类型取可用的取消方式）
fn build_cancel_handle(entry: &DbSessionEntry) -> CancelHandle {
    match &entry.session {
        DbSession::Postgres(_) => CancelHandle {
            aborted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pg_cancel: None,
            pg_ssl: entry.config.ssl,
            mysql_thread_id: None,
            mysql_conn: None,
            agent: None,
        },
        DbSession::Mysql(_) => CancelHandle {
            aborted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pg_cancel: None,
            pg_ssl: false,
            mysql_thread_id: None,
            mysql_conn: Some((
                entry.config.host.clone(),
                entry.config.port,
                entry.config.username.clone(),
                String::new(),
            )),
            agent: None,
        },
        DbSession::Agent { client, session_id } => CancelHandle {
            aborted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pg_cancel: None,
            pg_ssl: false,
            mysql_thread_id: None,
            mysql_conn: None,
            agent: Some((client.clone(), session_id.clone())),
        },
        _ => CancelHandle {
            aborted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pg_cancel: None,
            pg_ssl: false,
            mysql_thread_id: None,
            mysql_conn: None,
            agent: None,
        },
    }
}

// ──────────────────────────────────────────────────────────────────────────
// 各驱动执行实现
// ──────────────────────────────────────────────────────────────────────────

/// MySQL：查询走 query_iter（带行数上限），非查询走 exec_drop
async fn execute_mysql(
    pool: &mysql_async::Pool,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect =
        dialect_for(crate::plugins::database::models::DbType::Mysql).expect("mysql 方言存在");
    let statements = dialect.split_statements(sql);
    if statements.is_empty() {
        return Ok(QueryResult {
            ok: true,
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            is_query: false,
            duration_ms: 0,
            truncated: false,
            error: None,
        });
    }
    let mut conn = pool
        .get_conn()
        .await
        .map_err(|e| format!("取连接失败: {e}"))?;
    let mut result = QueryResult {
        ok: true,
        columns: Vec::new(),
        rows: Vec::new(),
        rows_affected: 0,
        is_query: false,
        duration_ms: 0,
        truncated: false,
        error: None,
    };
    for stmt in statements {
        if dialect.is_query_sql(&stmt) {
            let mut query = conn
                .query_iter(&stmt)
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            let columns = query
                .columns_ref()
                .iter()
                .map(|c| c.name_str().to_string())
                .collect::<Vec<_>>();
            let mut rows = Vec::new();
            let mut truncated = false;
            let mut count = 0u64;
            while let Some(row) = query
                .next()
                .await
                .map_err(|e| format!("读取结果失败: {e}"))?
            {
                count += 1;
                if count > max_rows {
                    truncated = true;
                    break;
                }
                let mut cells = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    cells.push(mysql_cell_str(&row, i));
                }
                rows.push(cells);
            }
            result = QueryResult {
                ok: true,
                columns,
                rows,
                rows_affected: count,
                is_query: true,
                duration_ms: 0,
                truncated,
                error: None,
            };
        } else {
            conn.query_drop(&stmt)
                .await
                .map_err(|e| format!("执行失败: {e}"))?;
            result.rows_affected += conn.affected_rows();
            result.is_query = false;
        }
    }
    Ok(result)
}

/// PostgreSQL：查询走 client.query，非查询走 client.execute
async fn execute_postgres(
    pool: &deadpool_postgres::Pool,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect =
        dialect_for(crate::plugins::database::models::DbType::Postgresql).expect("pg 方言存在");
    let statements = dialect.split_statements(sql);
    if statements.is_empty() {
        return Ok(QueryResult {
            ok: true,
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            is_query: false,
            duration_ms: 0,
            truncated: false,
            error: None,
        });
    }
    let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
    let mut result = QueryResult {
        ok: true,
        columns: Vec::new(),
        rows: Vec::new(),
        rows_affected: 0,
        is_query: false,
        duration_ms: 0,
        truncated: false,
        error: None,
    };
    for stmt in statements {
        if dialect.is_query_sql(&stmt) {
            let rows = client
                .query(&stmt, &[])
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            let columns: Vec<String> = rows
                .first()
                .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();
            let truncated = rows.len() as u64 > max_rows;
            let data = rows
                .iter()
                .take(max_rows as usize)
                .map(|row| {
                    (0..columns.len())
                        .map(|i| pg_cell_str(row, i))
                        .collect::<Vec<_>>()
                })
                .collect();
            result = QueryResult {
                ok: true,
                columns,
                rows: data,
                rows_affected: rows.len() as u64,
                is_query: true,
                duration_ms: 0,
                truncated,
                error: None,
            };
        } else {
            let affected = client
                .execute(&stmt, &[])
                .await
                .map_err(|e| format!("执行失败: {e}"))?;
            result.rows_affected += affected;
            result.is_query = false;
        }
    }
    Ok(result)
}

/// SQLite：rusqlite 锁内执行
fn execute_sqlite(
    conn: &std::sync::Mutex<rusqlite::Connection>,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect =
        dialect_for(crate::plugins::database::models::DbType::Sqlite).expect("sqlite 方言存在");
    let statements = dialect.split_statements(sql);
    if statements.is_empty() {
        return Ok(QueryResult {
            ok: true,
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            is_query: false,
            duration_ms: 0,
            truncated: false,
            error: None,
        });
    }
    let guard = conn.lock().map_err(|e| e.to_string())?;
    let mut result = QueryResult {
        ok: true,
        columns: Vec::new(),
        rows: Vec::new(),
        rows_affected: 0,
        is_query: false,
        duration_ms: 0,
        truncated: false,
        error: None,
    };
    for stmt in statements {
        if dialect.is_query_sql(&stmt) {
            let mut prepared = guard
                .prepare(&stmt)
                .map_err(|e| format!("语句解析失败: {e}"))?;
            let columns = prepared
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            let mut rows_iter = prepared.query([]).map_err(|e| format!("查询失败: {e}"))?;
            let mut rows = Vec::new();
            let mut count = 0u64;
            let mut truncated = false;
            while let Some(row) = rows_iter.next().map_err(|e| format!("读取结果失败: {e}"))?
            {
                count += 1;
                if count > max_rows {
                    truncated = true;
                    break;
                }
                let mut cells = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    cells.push(sqlite_cell_str(row, i)?);
                }
                rows.push(cells);
            }
            result = QueryResult {
                ok: true,
                columns,
                rows,
                rows_affected: count,
                is_query: true,
                duration_ms: 0,
                truncated,
                error: None,
            };
        } else {
            let affected = guard
                .execute(&stmt, [])
                .map_err(|e| format!("执行失败: {e}"))?;
            result.rows_affected += affected as u64;
            result.is_query = false;
        }
    }
    Ok(result)
}

/// agent：execute_query RPC（结果 JSON 转字符串）
async fn execute_agent(
    client: &AgentClient,
    session_id: &str,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let value = client.execute_query(session_id, sql, max_rows).await?;
    let columns = value
        .get("columns")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let rows = value
        .get("rows")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| cells.iter().map(json_cell_str).collect::<Vec<_>>())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let affected = value
        .get("affected_rows")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let truncated = value
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(QueryResult {
        ok: true,
        columns,
        rows,
        rows_affected: affected,
        is_query: true,
        duration_ms: 0,
        truncated,
        error: None,
    })
}

// ──────────────────────────────────────────────────────────────────────────
// 元数据命令
// ──────────────────────────────────────────────────────────────────────────

/// 数据库列表
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_databases(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<Vec<String>, String> {
    let entry = session(&state, &conn_id)?;
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("mysql 方言存在");
            query_strings_mysql(pool, dialect.databases_sql().ok_or("该类型无库列表")?).await
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("pg 方言存在");
            query_strings_pg(pool, dialect.databases_sql().ok_or("该类型无库列表")?).await
        }
        DbSession::Sqlite(_) => Ok(vec!["main".to_string()]),
        DbSession::Redis(_) => Ok(vec![entry.config.database.clone()]),
        DbSession::Agent { client, session_id } => client.list_databases(session_id).await,
    }
}

/// schema 列表（mysql/redis 无 schema 层返回空）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_schemas(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<Vec<String>, String> {
    let entry = session(&state, &conn_id)?;
    match &entry.session {
        DbSession::Mysql(_) | DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Postgres(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("pg 方言存在");
            query_strings_pg(pool, dialect.schemas_sql().ok_or("该类型无 schema 列表")?).await
        }
        DbSession::Sqlite(_) => Ok(vec!["main".to_string()]),
        DbSession::Agent { client, session_id } => client.list_schemas(session_id).await,
    }
}

/// 对象列表（表/视图等；schema 为 null 时用默认值）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_objects(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
) -> Result<Vec<DbObjectInfo>, String> {
    let entry = session(&state, &conn_id)?;
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("mysql 方言存在");
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let rows = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?
                .exec(dialect.objects_sql(), (database.clone(),))
                .await
                .map_err(|e| format!("对象列表失败: {e}"))?;
            Ok(rows
                .into_iter()
                .map(|row| DbObjectInfo {
                    kind: mysql_str(&row, 1),
                    name: mysql_str(&row, 0),
                })
                .collect())
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("pg 方言存在");
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let rows = client
                .query(dialect.objects_sql(), &[&schema])
                .await
                .map_err(|e| format!("对象列表失败: {e}"))?;
            Ok(rows
                .iter()
                .map(|row| DbObjectInfo {
                    kind: row.try_get::<usize, String>(1).unwrap_or_default(),
                    name: row.try_get::<usize, String>(0).unwrap_or_default(),
                })
                .collect())
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let mut stmt = guard
                .prepare(
                    dialect_for(crate::plugins::database::models::DbType::Sqlite)
                        .expect("sqlite 方言存在")
                        .objects_sql(),
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(DbObjectInfo {
                        kind: row.get::<_, String>(1)?,
                        name: row.get::<_, String>(0)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())
        }
        DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Agent { client, session_id } => {
            let schema = schema.unwrap_or_default();
            let objects = client.list_objects(session_id, &schema).await?;
            Ok(objects
                .into_iter()
                .map(|(kind, name)| DbObjectInfo { kind, name })
                .collect())
        }
    }
}

/// 表结构列信息
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_columns(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<DbColumnInfo>, String> {
    let entry = session(&state, &conn_id)?;
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("mysql 方言存在");
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let rows = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?
                .exec(dialect.columns_sql(), (database.clone(), table.clone()))
                .await
                .map_err(|e| format!("列信息失败: {e}"))?;
            Ok(rows
                .into_iter()
                .map(|row| DbColumnInfo {
                    name: mysql_str(&row, 0),
                    data_type: mysql_str(&row, 1),
                    nullable: if mysql_str(&row, 2) == "NO" {
                        "否"
                    } else {
                        "是"
                    }
                    .to_string(),
                    default_value: mysql_str(&row, 3),
                    key: mysql_key(&row, 5),
                    comment: mysql_str(&row, 4),
                })
                .collect())
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("pg 方言存在");
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let rows = client
                .query(dialect.columns_sql(), &[&schema, &table])
                .await
                .map_err(|e| format!("列信息失败: {e}"))?;
            // 键标记探测（PK/UK）
            let keys = pg_keys(&client, &schema, &table).await?;
            Ok(rows
                .iter()
                .map(|row| {
                    let name: String = row.try_get::<usize, String>(0).unwrap_or_default();
                    DbColumnInfo {
                        name: name.clone(),
                        data_type: row.try_get::<usize, String>(1).unwrap_or_default(),
                        nullable: row.try_get::<usize, String>(2).unwrap_or_default(),
                        default_value: row.try_get::<usize, String>(3).unwrap_or_default(),
                        key: keys.get(&name).cloned().unwrap_or_else(|| "—".to_string()),
                        comment: row.try_get::<usize, String>(4).unwrap_or_default(),
                    }
                })
                .collect())
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let safe = table.replace(['"', '`'], "");
            let mut stmt = guard
                .prepare(&format!("PRAGMA table_xinfo(\"{safe}\")"))
                .map_err(|e| format!("列信息失败: {e}"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(DbColumnInfo {
                        name: row.get::<_, String>(1)?,
                        data_type: row.get::<_, String>(2)?,
                        nullable: if row.get::<_, i64>(3)? != 0 {
                            "否"
                        } else {
                            "是"
                        }
                        .to_string(),
                        default_value: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                        key: if row.get::<_, i64>(5)? > 0 {
                            "PK".to_string()
                        } else {
                            "—".to_string()
                        },
                        comment: String::new(),
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())
        }
        DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Agent { client, session_id } => {
            let schema = schema.unwrap_or_default();
            let value = client.get_columns(session_id, &schema, &table).await?;
            let items = value
                .get("columns")
                .or_else(|| value.get("items"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            Ok(items
                .iter()
                .map(|item| DbColumnInfo {
                    name: item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    data_type: item
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    nullable: item
                        .get("nullable")
                        .and_then(|v| v.as_bool())
                        .map(|b| if b { "是" } else { "否" }.to_string())
                        .unwrap_or_default(),
                    default_value: item
                        .get("default")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    key: item
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("—")
                        .to_string(),
                    comment: item
                        .get("comment")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect())
        }
    }
}

/// 表数据分页（native 走 LIMIT/OFFSET + COUNT；agent 走 maxRows 抓取后切片）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_data(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
    page: u32,
    page_size: u32,
) -> Result<DbTablePage, String> {
    let entry = session(&state, &conn_id)?;
    let started = Instant::now();
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);

    let (columns, rows, total) = match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("mysql 方言存在");
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let qualified = format!(
                "{}.{}",
                dialect.quote_ident(&database),
                dialect.quote_ident(&table)
            );
            let sql = dialect.paginate(
                &format!("SELECT * FROM {qualified}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?;
            let result = execute_mysql(pool, &sql, page_size as u64).await?;
            // 行数：information_schema.TABLES.TABLE_ROWS（估算；该列为字符串类型）
            let count_row = conn
                .exec_first::<String, _, _>(
                    dialect.row_count_sql(),
                    (database.clone(), table.clone()),
                )
                .await
                .ok()
                .flatten()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(result.rows.len() as u64);
            (result.columns, result.rows, count_row)
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_for(entry.config.db_type).expect("pg 方言存在");
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let qualified = format!(
                "{}.{}",
                dialect.quote_ident(&schema),
                dialect.quote_ident(&table)
            );
            let sql = dialect.paginate(
                &format!("SELECT * FROM {qualified}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let result = execute_postgres(pool, &sql, page_size as u64).await?;
            let count_row = client
                .query_one(dialect.row_count_sql(), &[&schema, &table])
                .await
                .ok()
                .map(|row| row.try_get::<usize, i64>(0).unwrap_or(0) as u64)
                .unwrap_or(result.rows.len() as u64);
            (result.columns, result.rows, count_row)
        }
        DbSession::Sqlite(conn) => {
            let dialect = dialect_for(crate::plugins::database::models::DbType::Sqlite)
                .expect("sqlite 方言存在");
            let quoted = dialect.quote_ident(&table);
            let sql = dialect.paginate(
                &format!("SELECT * FROM {quoted}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let result = execute_sqlite(conn, &sql, page_size as u64)?;
            let count = {
                let guard = conn.lock().map_err(|e| e.to_string())?;
                guard
                    .query_row(&format!("SELECT COUNT(*) FROM {quoted}"), [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .unwrap_or(result.rows.len() as i64)
                    .max(0) as u64
            };
            (result.columns, result.rows, count)
        }
        DbSession::Redis(_) => {
            return Ok(DbTablePage {
                columns: Vec::new(),
                rows: Vec::new(),
                total: 0,
                page,
                page_size,
                duration_ms: 0,
                error: Some("Redis 无表数据浏览（请用键浏览面板）".to_string()),
            });
        }
        DbSession::Agent { client, session_id } => {
            // agent：一次抓取 page*page_size 行（上限保护），前端切片
            let schema = schema.unwrap_or_default();
            let qualified = if schema.is_empty() {
                table.clone()
            } else {
                format!("{schema}.{table}")
            };
            let need = (page as u64) * (page_size as u64);
            let value = client
                .execute_query(session_id, &format!("SELECT * FROM {qualified}"), need)
                .await?;
            let columns = value
                .get("columns")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let all_rows = value
                .get("rows")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|row| {
                            row.as_array()
                                .map(|cells| cells.iter().map(json_cell_str).collect::<Vec<_>>())
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let total = all_rows.len() as u64;
            let start = ((page - 1) * page_size) as usize;
            let page_rows = all_rows
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect();
            // 行数探测：SELECT COUNT(*)（失败时用抓取行数）
            let count = client
                .execute_query(session_id, &format!("SELECT COUNT(*) FROM {qualified}"), 1)
                .await
                .ok()
                .and_then(|v| {
                    v.get("rows")
                        .and_then(|r| r.as_array())
                        .and_then(|rows| rows.first())
                        .and_then(|row| row.as_array())
                        .and_then(|row| row.first())
                        .and_then(|cell| cell.as_u64())
                })
                .unwrap_or(total);
            (columns, page_rows, count)
        }
    };

    Ok(DbTablePage {
        columns,
        rows,
        total,
        page,
        page_size,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
    })
}

/// 执行计划（方言前缀包装；oracle agent 不支持）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_explain(
    state: State<'_, DbState>,
    conn_id: String,
    sql: String,
) -> Result<Vec<String>, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type);
    let Some(dialect) = dialect else {
        return Err("该数据库类型暂不支持执行计划".to_string());
    };
    let explain_sql = dialect.explain_sql(&sql);
    let result = match &entry.session {
        DbSession::Mysql(pool) => execute_mysql(pool, &explain_sql, 200).await?,
        DbSession::Postgres(pool) => execute_postgres(pool, &explain_sql, 200).await?,
        DbSession::Sqlite(conn) => execute_sqlite(conn, &explain_sql, 200)?,
        _ => return Err("该数据库类型暂不支持执行计划".to_string()),
    };
    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "执行计划失败".to_string()));
    }
    Ok(result.rows.iter().map(|row| row.join(" | ")).collect())
}

/// 导出 CSV 文件（结果集导出；路径由前端对话框选定）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_export_csv(path: String, text: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("保存路径不能为空".to_string());
    }
    std::fs::write(&path, text).map_err(|e| format!("CSV 写入失败（{path}）: {e}"))
}

/// Redis 键列表（SCAN 游标）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_keys(
    state: State<'_, DbState>,
    conn_id: String,
    pattern: String,
    cursor: u64,
) -> Result<(u64, Vec<String>), String> {
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(mgr) = &entry.session else {
        unreachable!("已校验 is_redis");
    };
    let mut mgr = mgr.clone();
    redis::scan_keys(&mut mgr, &pattern, cursor, 200).await
}

/// Redis 键信息（TYPE/TTL/值预览）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_key_info(
    state: State<'_, DbState>,
    conn_id: String,
    key: String,
) -> Result<crate::plugins::database::models::RedisKeyInfo, String> {
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(mgr) = &entry.session else {
        unreachable!("已校验 is_redis");
    };
    let mut mgr = mgr.clone();
    redis::key_info(&mut mgr, &key).await
}

// ──────────────────────────────────────────────────────────────────────────
// 单元格字符串化
// ──────────────────────────────────────────────────────────────────────────

/// MySQL 单元格 → 字符串（NULL → "NULL"；字节 → UTF-8 或长度标注）
fn mysql_cell_str(row: &mysql_async::Row, index: usize) -> String {
    let Some(value) = row.get::<MysqlValue, usize>(index) else {
        return "NULL".to_string();
    };
    match value {
        MysqlValue::NULL => "NULL".to_string(),
        MysqlValue::Bytes(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "NULL".to_string()),
        other => other.as_sql(false),
    }
}

/// MySQL 行内字符串字段（get 返回 Option<T>，NULL → 空串）
fn mysql_str(row: &mysql_async::Row, index: usize) -> String {
    row.get::<Option<String>, usize>(index)
        .flatten()
        .unwrap_or_default()
}

/// MySQL 键标记（COLUMN_KEY: PRI → PK, UNI → UK）
fn mysql_key(row: &mysql_async::Row, index: usize) -> String {
    match mysql_str(row, index).as_str() {
        "PRI" => "PK".to_string(),
        "UNI" => "UK".to_string(),
        _ => "—".to_string(),
    }
}

/// PostgreSQL 单元格 → 字符串（按类型链式 try_get）
fn pg_cell_str(row: &tokio_postgres::Row, index: usize) -> String {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
    if let Ok(v) = row.try_get::<usize, Option<String>>(index) {
        return v.unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<i64>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |n| n.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<f64>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |n| n.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<bool>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |b| b.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<NaiveDateTime>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |d| d.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<DateTime<Utc>>>(index) {
        return v.map_or_else(
            || "NULL".to_string(),
            |d| d.format("%Y-%m-%d %H:%M:%S").to_string(),
        );
    }
    if let Ok(v) = row.try_get::<usize, Option<NaiveDate>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |d| d.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<Vec<u8>>>(index) {
        return v.map_or_else(
            || "NULL".to_string(),
            |b| format!("<blob {} bytes>", b.len()),
        );
    }
    if let Ok(v) = row.try_get::<usize, Option<serde_json::Value>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |j| j.to_string());
    }
    "NULL".to_string()
}

/// SQLite 单元格 → 字符串
fn sqlite_cell_str(row: &rusqlite::Row, index: usize) -> Result<String, String> {
    let value = row.get_ref(index).map_err(|e| e.to_string())?;
    Ok(match value {
        rusqlite::types::ValueRef::Null => "NULL".to_string(),
        rusqlite::types::ValueRef::Integer(n) => n.to_string(),
        rusqlite::types::ValueRef::Real(f) => f.to_string(),
        rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        rusqlite::types::ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
    })
}

/// agent JSON 单元格 → 字符串
fn json_cell_str(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// PostgreSQL 键标记探测（PK/UK）
async fn pg_keys(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let rows = client
        .query(
            "SELECT a.attname, \
                    CASE WHEN ix.indisprimary THEN 'PK' WHEN ix.indisunique THEN 'UK' ELSE '' END \
             FROM pg_catalog.pg_index ix \
             JOIN pg_catalog.pg_class c ON c.oid = ix.indrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(ix.indkey) \
             WHERE n.nspname = $1 AND c.relname = $2 AND (ix.indisprimary OR ix.indisunique)",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("键标记探测失败: {e}"))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let name: String = row.try_get::<usize, String>(0).ok()?;
            let key: String = row.try_get::<usize, String>(1).ok()?;
            if key.is_empty() {
                None
            } else {
                Some((name, key))
            }
        })
        .collect())
}

/// 通用字符串列表查询（mysql）
async fn query_strings_mysql(pool: &mysql_async::Pool, sql: &str) -> Result<Vec<String>, String> {
    let mut conn = pool
        .get_conn()
        .await
        .map_err(|e| format!("取连接失败: {e}"))?;
    conn.query::<String, _>(sql)
        .await
        .map_err(|e| format!("查询失败: {e}"))
}

/// 通用字符串列表查询（pg）
async fn query_strings_pg(
    pool: &deadpool_postgres::Pool,
    sql: &str,
) -> Result<Vec<String>, String> {
    let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
    let rows = client
        .query(sql, &[])
        .await
        .map_err(|e| format!("查询失败: {e}"))?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<usize, String>(0).ok())
        .collect())
}
