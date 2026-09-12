//! 数据库驱动层 · 会话探测（版本与时延）
//! 职责：建连后对已建立的会话做 best-effort 探测 —— 取数据库版本并测量往返时延；
//! 探测失败不阻断连接（返回空版本 + 实测时延），由连接层决定降级值。
//! 各驱动一条探测查询：mysql/postgres 取方言 version_sql() 首行首列，sqlite 取
//! sqlite_version()，redis 解析 INFO server 的 redis_version，agent 侧车先取
//! connection_info 再回退 execute_query。

use std::time::Instant;

use mysql_async::prelude::Queryable;

use crate::plugins::database::dialect::dialect_for;
use crate::plugins::database::models::{ConnConfig, DbType};

use super::session::DbSession;

/// 版本与时延探测（各驱动一条探测查询；失败返回空版本）
/// 由连接层在建连后调用；返回 (版本, 时延毫秒)，时延始终为 Some（实测值）。
pub(crate) async fn probe_version(
    session: &DbSession,
    config: &ConnConfig,
) -> (String, Option<u64>) {
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
