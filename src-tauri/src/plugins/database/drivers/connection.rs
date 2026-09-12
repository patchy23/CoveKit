//! 数据库驱动层 · 连接生命周期（建立 / 测试 / 断开）
//! 职责：按驱动类型分派建连（connect）、只探测不落表的一次性连通性测试（test_connection）、
//! 移除会话并释放资源（disconnect），以及 agent 侧车会话所需的进程管理与参数组装。
//! 不变量：连接配置先经 normalize_config 归一化（端口/默认库名），
//! 会话条目写入注册表后由 session 模块持有；mysql 池必须先归还连接再 disconnect（否则挂起），
//! agent 会话关闭后若已无其它会话共享该进程则 shutdown 并从运行时表移除。

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mysql_async::prelude::Queryable;
use tauri::State;

use crate::plugins::database::agent::{AgentClient, AgentConnectParams, DriverStore};
use crate::plugins::database::models::{ConnConfig, DbType};

use super::probe::probe_version;
use super::session::{AgentRuntimeState, DbSession, DbSessionEntry, DbState};
use super::{mysql, postgres, sqlite};

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
