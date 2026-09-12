//! 数据库驱动层 · 查询执行与取消分派
//! 职责：查询开始前按会话类型构造取消句柄（build_cancel_handle，各驱动取消能力不同），
//! 以及把 agent 侧车的 JSON RPC 结果转换为统一查询结果（execute_agent + json_cell_str）。
//! 不变量：取消元数据（pg cancel_token / mysql 线程 id / agent 会话）在执行开始后由
//! 命令层补齐，此处只提供与驱动匹配的初始句柄；所有驱动都带通用 aborted 标志，
//! 驱动无原生取消能力时仅标记该标志（查询结果不因此伪装成功或失败）。

use crate::plugins::database::agent::AgentClient;
use crate::plugins::database::models::QueryResult;

use super::session::{DbSession, DbSessionEntry};
// 取消句柄类型经门面（mod.rs 重导出）引用：`drivers::CancelHandle` 是拆分前的 crate 内公开路径，
// 保持它可达避免消费方跟随文件移动改 import。
use super::CancelHandle;

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
