//! 数据库查询与目录 · query

use super::session;
use super::DEFAULT_MAX_ROWS;
use crate::plugins::database::drivers;
use crate::plugins::database::drivers::redis;
use crate::plugins::database::drivers::DbCancelState;
use crate::plugins::database::drivers::DbSession;
use crate::plugins::database::drivers::DbState;
use crate::plugins::database::models::QueryResult;
use mysql_async::prelude::Queryable;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::State;

// ──────────────────────────────────────────────────────────────────────────
// 查询执行
// ──────────────────────────────────────────────────────────────────────────

/// 执行 SQL（多语句拆分逐条执行；查询语句返回最后一条结果，非查询累计影响行数）
///
/// `request_id` 是本次执行的请求身份（前端每次执行生成一个），取消句柄按它登记：
/// 同一连接可以有多个在途请求（同连接多页签），取消必须命中发起的那一个，不能按连接一刀切。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_execute(
    state: State<'_, DbState>,
    cancel_state: State<'_, DbCancelState>,
    conn_id: String,
    sql: String,
    max_rows: Option<u64>,
    request_id: String,
) -> Result<QueryResult, String> {
    let entry = session(&state, &conn_id)?;
    let request_id = request_id.trim().to_string();
    if request_id.is_empty() {
        return Err("执行请求缺少请求标识，无法登记取消句柄".to_string());
    }
    let started = Instant::now();
    let limit = max_rows.unwrap_or(DEFAULT_MAX_ROWS).max(1);

    // 注册取消句柄（各驱动能力不同；按请求身份登记，同连接并发请求互不覆盖）
    let cancel = drivers::build_cancel_handle(&entry);
    let aborted = cancel.aborted.clone();
    let _ = cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(request_id.clone(), cancel);

    // Redis 会话：查询页签执行的是 Redis 命令（其余走 SQL 驱动）
    let outcome = match &entry.session {
        DbSession::Redis(mgr) => {
            let mut mgr = mgr.clone();
            redis::exec_command(&mut mgr, &sql)
                .await
                .map(|value| QueryResult {
                    ok: true,
                    columns: vec!["result".to_string()],
                    rows: vec![vec![value]],
                    rows_affected: 0,
                    is_query: true,
                    duration_ms: started.elapsed().as_millis() as u64,
                    truncated: false,
                    error: None,
                })
        }
        DbSession::Mysql(pool) => drivers::mysql::execute_mysql(pool, &sql, limit).await,
        DbSession::Postgres(pool) => drivers::postgres::execute_postgres(pool, &sql, limit).await,
        DbSession::Sqlite(conn) => drivers::sqlite::execute_sqlite(conn, &sql, limit),
        DbSession::Agent { client, session_id } => {
            drivers::execute_agent(client, session_id, &sql, limit).await
        }
    };

    // 无论成功、失败还是被取消，都注销本次请求身份（失败也走统一清理）
    let _ = cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id);
    let _ = aborted.load(Ordering::Relaxed);
    let mut result = outcome?;
    result.duration_ms = started.elapsed().as_millis() as u64;
    Ok(result)
}

/// 取消进行中的查询（按请求身份命中：pg cancel_token / mysql KILL QUERY / agent cancel_session）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_cancel(state: State<'_, DbCancelState>, request_id: String) -> Result<(), String> {
    let handle = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(request_id.trim())
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

/// 导出 CSV 文件（结果集导出；路径由前端对话框选定）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_export_csv(path: String, text: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("保存路径不能为空".to_string());
    }
    std::fs::write(&path, text).map_err(|e| format!("CSV 写入失败（{path}）: {e}"))
}
