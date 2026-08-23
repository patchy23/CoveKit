//! SQLite 驱动：文件连接 + 查询执行 + 单元格字符串化

use std::sync::{Arc, Mutex};

use rusqlite::Connection as SqliteConn;

use crate::plugins::database::dialect::dialect_for;
use crate::plugins::database::models::{ConnConfig, QueryResult};

/// 打开 SQLite 文件并包装为插件会话使用的线程安全连接。
pub(crate) fn sqlite_conn(config: &ConnConfig) -> Result<Arc<Mutex<SqliteConn>>, String> {
    let path = &config.host;
    if path.trim().is_empty() {
        return Err("SQLite 文件路径不能为空".to_string());
    }
    let conn = SqliteConn::open(path).map_err(|e| format!("SQLite 打开失败（{path}）: {e}"))?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// 构建 redis 连接管理器（多路复用；db 索引来自 database 字段的 db0/db1 形式）
pub(crate) fn execute_sqlite(
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
pub(crate) fn sqlite_cell_str(row: &rusqlite::Row, index: usize) -> Result<String, String> {
    let value = row.get_ref(index).map_err(|e| e.to_string())?;
    Ok(match value {
        rusqlite::types::ValueRef::Null => "NULL".to_string(),
        rusqlite::types::ValueRef::Integer(n) => n.to_string(),
        rusqlite::types::ValueRef::Real(f) => f.to_string(),
        rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        rusqlite::types::ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
    })
}
