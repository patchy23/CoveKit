//! 数据库驱动层：会话注册表 + 连接生命周期 + 查询执行与取消
//! mod.rs：会话注册表（DbState/会话条目/取消句柄）与连接/断开/快照/探测；
//! 各驱动文件：连接构建 + 查询执行 + 单元格字符串化。

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mysql_async::prelude::Queryable;
use rusqlite::Connection as SqliteConn;
use tauri::State;

use crate::plugins::database::agent::{AgentClient, AgentConnectParams, DriverStore};
use crate::plugins::database::dialect::dialect_for;
use crate::plugins::database::models::{
    ConnConfig, ConnStatus, DbConnectionInfo, DbType, QueryResult,
};

pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;

/// agent 运行时共享表：driver key → 进程客户端（同驱动多会话共享一个进程）
pub struct AgentRuntimeState(pub Mutex<HashMap<&'static str, Arc<AgentClient>>>);

/// 会话注册表 State（命令层通过本结构取会话）
pub struct DbState(pub Mutex<HashMap<String, DbSessionEntry>>);

/// 查询取消注册表 State（conn_id → 取消句柄）
pub struct DbCancelState(pub Mutex<HashMap<String, CancelHandle>>);

/// 会话本体：按驱动分派的连接句柄
#[derive(Clone)]
pub enum DbSession {
    /// mysql_async 连接池（mysql/polardb）
    Mysql(mysql_async::Pool),
    /// deadpool-postgres 连接池
    Postgres(deadpool_postgres::Pool),
    /// rusqlite 单连接（SQLite 文件库，Arc 共享 + 锁内串行；会话表按值克隆）
    Sqlite(Arc<Mutex<SqliteConn>>),
    /// redis 连接管理器（多路复用）
    Redis(::redis::aio::ConnectionManager),
    /// agent 侧车会话（进程客户端 + 会话 id）
    Agent {
        /// 进程客户端（与同驱动其它会话共享）
        client: Arc<AgentClient>,
        /// 逻辑会话 id
        session_id: String,
    },
}

/// 会话条目：会话 + 连接元信息（版本/时延探测结果缓存）
#[derive(Clone)]
pub struct DbSessionEntry {
    /// 连接配置（不含密码）
    pub config: ConnConfig,
    /// 会话本体
    pub session: DbSession,
    /// 连接时探测的版本号
    pub version: String,
    /// 连接时探测的时延（毫秒）
    pub latency_ms: u64,
    /// 建立时间（epoch 秒）
    pub connected_at: u64,
}

/// 取消句柄：进行中查询的取消方式（各驱动能力不同）
pub struct CancelHandle {
    /// 通用取消标志（所有驱动都设置；驱动无原生取消能力时仅标记）
    pub aborted: Arc<AtomicBool>,
    /// PostgreSQL 取消令牌（cancel_query 需要独立连接）
    pub pg_cancel: Option<tokio_postgres::CancelToken>,
    /// PostgreSQL 是否 TLS（取消连接需用 rustls 连接器）
    pub pg_ssl: bool,
    /// MySQL 会话线程 id（KILL QUERY 用）
    pub mysql_thread_id: Option<u32>,
    /// 会话连接信息（mysql KILL 需要新建连接）
    pub mysql_conn: Option<(String, u16, String, String)>,
    /// agent 会话取消（cancel_session RPC）
    pub agent: Option<(Arc<AgentClient>, String)>,
}

impl DbState {
    /// 取会话条目（不存在返回"未连接"错误）
    pub fn entry(&self, conn_id: &str) -> Result<DbSessionEntry, String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .get(conn_id)
            .cloned()
            .ok_or_else(|| format!("连接「{conn_id}」未建立，请先连接"))
    }
}

impl DbSessionEntry {
    /// 会话快照（前端连接列表展示）
    pub fn to_info(&self) -> DbConnectionInfo {
        DbConnectionInfo {
            id: self.config.id.clone(),
            label: self.config.label.clone(),
            db_type: self.config.db_type,
            env: self.config.env.clone(),
            status: ConnStatus::Online,
            version: self.version.clone(),
            latency_ms: self.latency_ms,
            readonly: self.config.readonly,
            host: self.config.host.clone(),
            database: self.config.database.clone(),
            error: None,
            connected_at: self.connected_at,
            port: self.config.port,
            username: self.config.username.clone(),
            ssl: self.config.ssl,
            connect_timeout_ms: self.config.connect_timeout_ms,
        }
    }

    /// 会话快照（agent 存活校验失败时标记离线并携带错误）
    fn to_info_with_error(&self, error: String) -> DbConnectionInfo {
        let mut info = self.to_info();
        info.status = ConnStatus::Offline;
        info.error = Some(error);
        info
    }
}

/// 归一化连接配置：端口缺省（0）时补默认端口（sqlite 除外），数据库为空时补默认库名
fn normalize_config(config: &ConnConfig) -> ConnConfig {
    let mut c = config.clone();
    if c.port == 0 && !c.db_type.is_sqlite() {
        c.port = c.db_type.default_port();
    }
    if c.database.trim().is_empty() {
        c.database = c.db_type.default_database().to_string();
    }
    c
}

/// 建立连接：按类型分派驱动，完成后探测版本与时延
pub async fn connect(
    app: &tauri::AppHandle,
    state: &State<'_, DbState>,
    runtimes: &State<'_, AgentRuntimeState>,
    config: &ConnConfig,
    password: &str,
) -> Result<DbSessionEntry, String> {
    let config = normalize_config(config);
    let started = Instant::now();
    let session = match config.db_type {
        DbType::Mysql | DbType::Polardb => {
            DbSession::Mysql(mysql::mysql_pool(&config, password).await?)
        }
        DbType::Postgresql => DbSession::Postgres(postgres::pg_pool(&config, password).await?),
        DbType::Sqlite => DbSession::Sqlite(sqlite::sqlite_conn(&config)?),
        DbType::Redis => DbSession::Redis(redis_mgr(&config, password).await?),
        DbType::Oracle | DbType::Vastbase | DbType::Kingbase => {
            let (client, session_id) = agent_session(app, runtimes, &config, password).await?;
            DbSession::Agent { client, session_id }
        }
        DbType::Dameng => {
            return Err("达梦驱动暂未支持（本版本未实现，后续版本提供）。".to_string())
        }
    };

    // 版本与时延探测（探测失败不阻断连接）
    let (version, latency_ms) = probe_version(&session, &config).await;
    let latency_ms = latency_ms.unwrap_or_else(|| started.elapsed().as_millis() as u64);
    let entry = DbSessionEntry {
        config: config.clone(),
        session,
        version,
        latency_ms,
        connected_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    let mut map = state.0.lock().map_err(|e| e.to_string())?;
    map.insert(config.id.clone(), entry.clone());
    Ok(entry)
}

/// 测试连接（不落会话表；成功返回版本信息文本）
pub async fn test_connection(
    app: &tauri::AppHandle,
    runtimes: &State<'_, AgentRuntimeState>,
    config: &ConnConfig,
    password: &str,
) -> Result<String, String> {
    let config = normalize_config(config);
    match config.db_type {
        DbType::Mysql | DbType::Polardb => {
            let pool = mysql::mysql_pool(&config, password).await?;
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("连接失败: {e}"))?;
            let version: String = conn
                .query_first::<String, _>("SELECT VERSION()")
                .await
                .map_err(|e| e.to_string())?
                .ok_or("版本探测无结果")?;
            // 必须先归还连接再关闭池：disconnect 会等待所有连接归还，conn 未 drop 时挂起
            drop(conn);
            let _ = pool.disconnect().await;
            Ok(version)
        }
        DbType::Postgresql => {
            let pool = postgres::pg_pool(&config, password).await?;
            let client = pool.get().await.map_err(|e| format!("连接失败: {e}"))?;
            let version: String = client
                .query_one("SELECT version()", &[])
                .await
                .map_err(|e| e.to_string())?
                .get::<usize, String>(0);
            Ok(version)
        }
        DbType::Sqlite => {
            let conn = sqlite::sqlite_conn(&config)?;
            let version: String = conn
                .lock()
                .map_err(|e| e.to_string())?
                .query_row("SELECT sqlite_version()", [], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            Ok(version)
        }
        DbType::Redis => {
            let mgr = redis_mgr(&config, password).await?;
            let info: String = ::redis::cmd("INFO")
                .arg("server")
                .query_async::<String>(&mut mgr.clone())
                .await
                .map_err(|e| format!("连接失败: {e}"))?;
            // INFO server 输出 redis_version:7.2.x
            let version = info
                .lines()
                .find(|l| l.starts_with("redis_version:"))
                .map(|l| l.trim_start_matches("redis_version:").to_string())
                .unwrap_or_else(|| "Redis".to_string());
            Ok(version)
        }
        DbType::Oracle | DbType::Vastbase | DbType::Kingbase => {
            let store = DriverStore::new(app)?;
            let binary = store.ensure_driver(app, config.db_type).await?;
            let client = agent_client_for(app, runtimes, config.db_type, &binary).await?;
            let params = connect_params(&config, password, "test-conn");
            client.test_connection(&params).await?;
            Ok("连接成功".to_string())
        }
        DbType::Dameng => Err("达梦驱动暂未支持（本版本未实现，后续版本提供）。".to_string()),
    }
}

/// 断开连接并移除会话（agent 会话关闭后若无其它会话共享进程则 shutdown）
pub async fn disconnect(
    state: &State<'_, DbState>,
    runtimes: &State<'_, AgentRuntimeState>,
    conn_id: &str,
) -> Result<(), String> {
    let entry = {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        map.remove(conn_id)
    };
    let Some(entry) = entry else {
        return Ok(());
    };
    match entry.session {
        DbSession::Mysql(pool) => {
            let _ = pool.disconnect().await;
        }
        DbSession::Postgres(pool) => {
            pool.close();
        }
        DbSession::Sqlite(_) => {}
        DbSession::Redis(_) => {}
        DbSession::Agent { client, session_id } => {
            // 会话关闭失败不阻断断开（进程可能已退出）
            let _ = client.close_session(&session_id).await;
            // 若没有其它会话共享同一 agent 进程，关闭进程并移除运行时条目
            let still_used = state
                .0
                .lock()
                .map(|m| {
                    m.values().any(|e| {
                        matches!(&e.session, DbSession::Agent { client: c, .. } if Arc::ptr_eq(c, &client))
                    })
                })
                .unwrap_or(false);
            if !still_used {
                let _ = client.shutdown().await;
                if let Ok(mut rt) = runtimes.0.lock() {
                    rt.retain(|_, c| !Arc::ptr_eq(c, &client));
                }
            }
        }
    }
    Ok(())
}

/// 全量会话快照（前端连接列表刷新；agent 会话做存活校验，进程被杀时标记离线）
pub async fn snapshot(state: &State<'_, DbState>, configs: &[ConnConfig]) -> Vec<DbConnectionInfo> {
    let sessions = state.0.lock().map(|m| m.clone()).unwrap_or_default();
    let mut out = Vec::with_capacity(configs.len());
    for config in configs {
        let info = match sessions.get(&config.id) {
            Some(entry) => {
                if let DbSession::Agent { client, session_id } = &entry.session {
                    let alive = tokio::time::timeout(
                        Duration::from_secs(1),
                        client.validate_session(session_id),
                    )
                    .await;
                    match alive {
                        Ok(Ok(())) => entry.to_info(),
                        Ok(Err(e)) => entry.to_info_with_error(format!("agent 进程不可达：{e}")),
                        Err(_) => {
                            entry.to_info_with_error("agent 存活校验超时（进程可能已退出）".into())
                        }
                    }
                } else {
                    entry.to_info()
                }
            }
            None => DbConnectionInfo {
                id: config.id.clone(),
                label: config.label.clone(),
                db_type: config.db_type,
                env: config.env.clone(),
                status: ConnStatus::Offline,
                version: String::new(),
                latency_ms: 0,
                readonly: config.readonly,
                host: config.host.clone(),
                database: config.database.clone(),
                error: None,
                connected_at: 0,
                port: config.port,
                username: config.username.clone(),
                ssl: config.ssl,
                connect_timeout_ms: config.connect_timeout_ms,
            },
        };
        out.push(info);
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────
// 各驱动连接构建
// ──────────────────────────────────────────────────────────────────────────

async fn agent_session(
    app: &tauri::AppHandle,
    runtimes: &State<'_, AgentRuntimeState>,
    config: &ConnConfig,
    password: &str,
) -> Result<(Arc<AgentClient>, String), String> {
    let store = DriverStore::new(app)?;
    let binary = store.ensure_driver(app, config.db_type).await?;
    let client = agent_client_for(app, runtimes, config.db_type, &binary).await?;
    let session_id = format!(
        "{}-{}",
        config.id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let params = connect_params(config, password, &session_id);
    client.open_session(&params).await?;
    Ok((client, session_id))
}

/// 取（或复用）某驱动类型的进程客户端
async fn agent_client_for(
    app: &tauri::AppHandle,
    runtimes: &State<'_, AgentRuntimeState>,
    db_type: DbType,
    binary: &std::path::Path,
) -> Result<Arc<AgentClient>, String> {
    let key = crate::plugins::database::agent::driver_key(db_type);
    if let Some(client) = runtimes.0.lock().map_err(|e| e.to_string())?.get(key) {
        return Ok(client.clone());
    }
    let store = DriverStore::new(app)?;
    let dir = store.driver_dir(db_type)?;
    let client = AgentClient::spawn(binary, &dir).await?;
    client.handshake().await?;
    let client = Arc::new(client);
    runtimes
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(key, client.clone());
    Ok(client)
}

/// 组装 agent 连接参数（snake_case，对齐 dbx ConnectParams）
fn connect_params(config: &ConnConfig, password: &str, session_id: &str) -> AgentConnectParams {
    AgentConnectParams {
        session_id: session_id.to_string(),
        session_role: "workload",
        host: config.host.clone(),
        port: config.port,
        database: config.database.clone(),
        username: config.username.clone(),
        password: password.to_string(),
        ssl: config.ssl,
    }
}

/// 版本与时延探测（各驱动一条探测查询；失败返回空版本）
async fn probe_version(session: &DbSession, config: &ConnConfig) -> (String, Option<u64>) {
    let started = Instant::now();
    let version = match session {
        DbSession::Mysql(pool) => {
            // 常量类型必然有方言；取不到时探测降级为空版本（探测本来就是 best-effort）
            let Some(dialect) = dialect_for(DbType::Mysql) else {
                return (String::new(), None);
            };
            query_first_string_mysql(pool, dialect.version_sql()).await
        }
        DbSession::Postgres(pool) => {
            let Some(dialect) = dialect_for(DbType::Postgresql) else {
                return (String::new(), None);
            };
            query_first_string_pg(pool, dialect.version_sql()).await
        }
        DbSession::Sqlite(conn) => conn.lock().ok().and_then(|conn| {
            conn.query_row("SELECT sqlite_version()", [], |r| r.get::<_, String>(0))
                .ok()
        }),
        DbSession::Redis(mgr) => {
            let mut mgr = mgr.clone();
            ::redis::cmd("INFO")
                .arg("server")
                .query_async::<String>(&mut mgr)
                .await
                .ok()
                .and_then(|info| {
                    info.lines()
                        .find(|l| l.starts_with("redis_version:"))
                        .map(|l| l.trim_start_matches("redis_version:").to_string())
                })
        }
        DbSession::Agent { client, session_id } => {
            // connection_info 取版本；失败则回退 execute_query SELECT version()
            let info = client.connection_info(session_id).await.ok();
            let from_info = info.and_then(|v| {
                v.get("version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            });
            if from_info.is_some() {
                from_info
            } else {
                client
                    .execute_query(session_id, "SELECT version()", 1)
                    .await
                    .ok()
                    .and_then(|v| {
                        v.get("rows")
                            .and_then(|r| r.as_array())
                            .and_then(|rows| rows.first())
                            .and_then(|row| row.as_array())
                            .and_then(|row| row.first())
                            .and_then(|cell| cell.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| Some(config.db_type.to_string()))
            }
        }
    };
    (
        version.unwrap_or_default(),
        Some(started.elapsed().as_millis() as u64),
    )
}

/// mysql 探测查询（取首行首列字符串）
async fn query_first_string_mysql(pool: &mysql_async::Pool, sql: &str) -> Option<String> {
    let mut conn = pool.get_conn().await.ok()?;
    conn.query_first::<String, _>(sql).await.ok().flatten()
}

/// pg 探测查询（取首行首列字符串）
async fn query_first_string_pg(pool: &deadpool_postgres::Pool, sql: &str) -> Option<String> {
    let client = pool.get().await.ok()?;
    client.query_one(sql, &[]).await.ok().map(|row| row.get(0))
}

/// 按当前会话类型构造查询取消句柄；具体取消元数据在执行开始后补齐。
pub(crate) fn build_cancel_handle(entry: &DbSessionEntry) -> CancelHandle {
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

/// 通过数据库侧车执行 SQL，并把 JSON RPC 结果转换为统一查询结果。
pub(crate) async fn execute_agent(
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

/// 将侧车返回的 JSON 单元格稳定转换为表格展示字符串。
pub(crate) fn json_cell_str(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// PostgreSQL 键标记探测（PK/UK）
async fn redis_mgr(
    config: &ConnConfig,
    password: &str,
) -> Result<::redis::aio::ConnectionManager, String> {
    let db_index = config
        .database
        .trim_start_matches("db")
        .parse::<u8>()
        .unwrap_or(0);
    let url = if password.is_empty() {
        format!("redis://{}:{}/{}", config.host, config.port, db_index)
    } else {
        format!(
            "redis://:{}@{}:{}/{}",
            urlencode(password),
            config.host,
            config.port,
            db_index
        )
    };
    let client =
        ::redis::Client::open(url.as_str()).map_err(|e| format!("Redis 地址解析失败: {e}"))?;
    // 连接管理器建立带超时（防挂起）
    let mgr = tokio::time::timeout(
        std::time::Duration::from_millis(config.connect_timeout_ms.max(30000)),
        ::redis::aio::ConnectionManager::new(client),
    )
    .await
    .map_err(|_| {
        format!(
            "Redis 连接超时（{} ms）",
            config.connect_timeout_ms.max(30000)
        )
    })?
    .map_err(|e| format!("Redis 连接失败: {e}"))?;
    // 预检 PING
    let pong: String = ::redis::cmd("PING")
        .query_async::<String>(&mut mgr.clone())
        .await
        .map_err(|e| format!("Redis 预检失败: {e}"))?;
    if pong != "PONG" {
        return Err(format!("Redis PING 异常: {pong}"));
    }
    Ok(mgr)
}

/// 简单 URL 编码（redis 密码含特殊字符时转义）
fn urlencode(input: &str) -> String {
    input
        .chars()
        .flat_map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
            other => {
                let bytes = other.to_string().into_bytes();
                bytes
                    .iter()
                    .map(|b| format!("%{b:02X}"))
                    .collect::<Vec<_>>()
                    .join("")
                    .chars()
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis密码url编码() {
        assert_eq!(urlencode("p@ss:wOrd/"), "p%40ss%3AwOrd%2F");
        assert_eq!(urlencode("simple"), "simple");
    }

    #[test]
    fn connect_params_use_snake_case() {
        let config = ConnConfig {
            id: "c1".into(),
            label: "测试".into(),
            db_type: DbType::Oracle,
            host: "127.0.0.1".into(),
            port: 1521,
            username: "scott".into(),
            database: "ORCL".into(),
            env: "开发".into(),
            readonly: false,
            ssl: true,
            connect_timeout_ms: 5000,
        };
        let params = connect_params(&config, "tiger", "sess-1");
        assert_eq!(params.session_id, "sess-1");
        assert_eq!(params.host, "127.0.0.1");
        assert_eq!(params.port, 1521);
        assert_eq!(params.database, "ORCL");
        assert_eq!(params.password, "tiger");
        assert!(params.ssl);
    }
}
