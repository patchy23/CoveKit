//! SQL 工作页会话：每页独占连接，短锁只保护注册表，IO 在页级异步锁内串行。
use super::{mysql, postgres, sqlite, DbSession, DbSessionEntry};
use crate::plugins::database::{
    agent::AgentClient,
    models::{ConnConfig, DbType, ExecutionScope},
};
use mysql_async::prelude::Queryable;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;

/// 页级连接表；关闭/断开时移除所属条目，不能复用活动事务。
#[derive(Default)]
pub struct WorkspaceState(
    pub Mutex<HashMap<(String, String), Arc<AsyncMutex<Option<WorkspaceSession>>>>>,
);

/// 不同驱动的独占工作连接。
pub(crate) enum WorkspaceConnection {
    Mysql(mysql_async::Conn, mysql_async::Pool),
    Postgres(deadpool_postgres::ClientWrapper),
    Sqlite(Arc<Mutex<rusqlite::Connection>>),
    Redis(redis::aio::ConnectionManager),
    Agent(Arc<AgentClient>, String),
}

/// 会话作用域在创建时固定；事务中不能更改目标。
pub struct WorkspaceSession {
    /// 创建时固定的执行目标。
    pub(crate) scope: ExecutionScope,
    /// 本工作页独占的真实连接。
    pub(crate) connection: WorkspaceConnection,
    /// 客户端已知事务状态；错误时不得伪装提交成功。
    pub(crate) transaction: bool,
    /// 创建时配置快照，用于判断连接是否已失效。
    pub(crate) config: ConnConfig,
}

impl WorkspaceState {
    /// 取得页级槽位，限制同时打开的工作连接数。
    pub(crate) fn slot(
        &self,
        connection: &str,
        workspace: &str,
    ) -> Result<Arc<AsyncMutex<Option<WorkspaceSession>>>, String> {
        if workspace.trim().is_empty() {
            return Err("DB_SESSION_REQUIRED: 缺少 SQL 页签身份".into());
        }
        let mut sessions = self.0.lock().map_err(|e| e.to_string())?;
        let key = (connection.to_string(), workspace.to_string());
        if !sessions.contains_key(&key) && sessions.len() >= 32 {
            return Err("DB_SESSION_LIMIT: 请先关闭部分 SQL 页签".into());
        }
        Ok(Arc::clone(
            sessions
                .entry(key)
                .or_insert_with(|| Arc::new(AsyncMutex::new(None))),
        ))
    }
}

/// 从保存配置创建工作页连接；密码只用于建连，不写会话 DTO。
pub(crate) async fn open(
    entry: &DbSessionEntry,
    scope: ExecutionScope,
    password: &str,
) -> Result<WorkspaceSession, String> {
    let mut config: ConnConfig = entry.config.clone();
    if !scope.database.is_empty() && !matches!(config.db_type, DbType::Oracle | DbType::Sqlite) {
        config.database.clone_from(&scope.database);
    }
    let connection = match &entry.session {
        DbSession::Mysql(_) => {
            let pool = mysql::mysql_pool(&config, password).await?;
            let mut conn = pool.get_conn().await.map_err(|e| e.to_string())?;
            if config.readonly {
                conn.query_drop("SET SESSION TRANSACTION READ ONLY")
                    .await
                    .map_err(|e| e.to_string())?;
            }
            WorkspaceConnection::Mysql(conn, pool)
        }
        DbSession::Postgres(_) => {
            let pool = postgres::pg_pool(&config, password).await?;
            let client = pool.get().await.map_err(|e| e.to_string())?;
            let client = deadpool_postgres::Object::take(client);
            if config.readonly {
                client
                    .batch_execute("SET default_transaction_read_only = on")
                    .await
                    .map_err(|e| e.to_string())?;
            }
            if !scope.schema.is_empty() {
                client
                    .query_one(
                        "SELECT set_config('search_path', quote_ident($1), false)",
                        &[&scope.schema],
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            }
            WorkspaceConnection::Postgres(client)
        }
        DbSession::Sqlite(_) => {
            let conn = tokio::task::spawn_blocking(move || sqlite::sqlite_conn(&config))
                .await
                .map_err(|e| e.to_string())??;
            WorkspaceConnection::Sqlite(conn)
        }
        DbSession::Redis(_) => {
            WorkspaceConnection::Redis(super::connection::redis_mgr(&config, password).await?)
        }
        DbSession::Agent { client, .. } => {
            let session_id = uuid::Uuid::new_v4().to_string();
            let params = super::connection::connect_params(&config, password, &session_id);
            client.open_session(&params).await?;
            if !scope.schema.is_empty() && config.db_type == DbType::Oracle {
                let schema = scope.schema.replace('"', "\"\"");
                client
                    .execute_query(
                        &session_id,
                        &format!("ALTER SESSION SET CURRENT_SCHEMA = \"{schema}\""),
                        1,
                    )
                    .await?;
            }
            WorkspaceConnection::Agent(Arc::clone(client), session_id)
        }
    };
    Ok(WorkspaceSession {
        scope,
        connection,
        transaction: false,
        config: entry.config.clone(),
    })
}

/// 显式关闭会话，事务回滚由连接关闭保证；agent 使用协议关闭自己的 session。
pub(crate) async fn close(session: WorkspaceSession) -> Result<(), String> {
    match session.connection {
        WorkspaceConnection::Mysql(mut conn, pool) => {
            let rollback = if session.transaction {
                conn.query_drop("ROLLBACK").await.map_err(|e| e.to_string())
            } else {
                Ok(())
            };
            let disconnect = conn.disconnect().await.map_err(|e| e.to_string());
            let pool_close = pool.disconnect().await.map_err(|e| e.to_string());
            rollback.and(disconnect).and(pool_close)?;
        }
        WorkspaceConnection::Agent(client, id) => {
            client.close_session(&id).await?;
        }
        _ => {}
    }
    Ok(())
}

/// 断开所属连接前先取消其查询，再逐页关闭；不会影响其他连接。
pub(crate) async fn close_connection(
    workspaces: &WorkspaceState,
    cancellations: &super::DbCancelState,
    connection_id: &str,
) -> Result<(), String> {
    let handles: Vec<_> = cancellations
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .values()
        .filter(|handle| handle.connection_id == connection_id)
        .cloned()
        .collect();
    let mut failures = Vec::new();
    for handle in handles {
        match tokio::time::timeout(std::time::Duration::from_secs(5), handle.cancel()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failures.push(error),
            Err(_) => failures.push("查询取消超时".into()),
        }
    }
    let slots = {
        let mut registry = workspaces.0.lock().map_err(|e| e.to_string())?;
        let keys: Vec<_> = registry
            .keys()
            .filter(|(owner, _)| owner == connection_id)
            .cloned()
            .collect();
        keys.into_iter()
            .filter_map(|key| registry.remove(&key))
            .collect::<Vec<_>>()
    };
    for slot in slots {
        let cleanup = async {
            let mut guard = slot.lock().await;
            if let Some(session) = guard.take() {
                close(session).await?;
            }
            Ok::<(), String>(())
        };
        match tokio::time::timeout(std::time::Duration::from_secs(12), cleanup).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failures.push(error),
            Err(_) => failures.push("工作会话关闭超时，后台查询尚未确认结束".into()),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("；"))
    }
}
