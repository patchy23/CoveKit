//! SQLite 驱动：文件连接 + 查询执行 + 单元格字符串化

use std::sync::{Arc, Mutex};

use rusqlite::Connection as SqliteConn;

use crate::plugins::database::dialect::dialect_or_err;
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

/// 执行 SQL：按方言拆分多语句逐条执行；查询语句返回结果集（超过 max_rows 标记截断），
/// 非查询语句累计影响行数；Redis 会话不走这里。
pub(crate) fn execute_sqlite(
    conn: &std::sync::Mutex<rusqlite::Connection>,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect = dialect_or_err(crate::plugins::database::models::DbType::Sqlite)?;
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

/// 单元格字符串化：NULL → "NULL"，二进制 → <blob N bytes>，其余按文本解码（非法 UTF-8 用替换字符）。
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

#[cfg(test)]
mod tests {
    //! 真实存储用例：使用**临时文件库**（不是内存库）验证驱动层落盘、结果字符串化与错误路径。
    //! 本机没有 MySQL/PG/Redis 服务，这组用例是 AR05 驱动矩阵里唯一可本地实测的驱动。

    use std::path::{Path, PathBuf};

    use super::*;
    use crate::plugins::database::models::DbType;

    /// SQLite 连接配置（只需 host 指向文件；其余字段与前端默认值一致）
    fn sqlite_config(path: &Path) -> ConnConfig {
        ConnConfig {
            id: "conn-test".to_string(),
            label: "临时库".to_string(),
            db_type: DbType::Sqlite,
            host: path.to_string_lossy().to_string(),
            port: 0,
            username: String::new(),
            database: "main".to_string(),
            env: "dev".to_string(),
            readonly: false,
            ssl: false,
            connect_timeout_ms: 5_000,
        }
    }

    /// 申请一个独占的临时库文件路径（按 tag + 进程号区分，测试之间不共用文件）
    fn temp_db_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("patchybox-db-test-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// 空路径必须直接报错，不能静默建一个库文件
    #[test]
    fn empty_path_is_rejected() {
        let config = ConnConfig {
            host: String::new(),
            ..sqlite_config(&temp_db_path("empty"))
        };
        let err = sqlite_conn(&config).expect_err("空路径必须报错");
        assert!(err.contains("不能为空"), "错误信息应说明原因: {err}");
    }

    /// 写入落到真实文件：关闭连接后重新打开同一路径，数据仍在
    #[test]
    fn writes_survive_reopen_of_the_same_file() {
        let path = temp_db_path("persist");
        let config = sqlite_config(&path);
        let conn = sqlite_conn(&config).expect("打开临时库");
        let created = execute_sqlite(
            &conn,
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO t (name) VALUES ('甲'); INSERT INTO t (name) VALUES ('乙');",
            100,
        )
        .expect("建表与插入");
        assert!(!created.is_query, "建表/插入不应标记为查询结果");
        assert_eq!(created.rows_affected, 2, "两条 INSERT 的影响行数应累计");
        drop(conn);

        // 重新打开同一个文件：数据必须还在（证明是落盘的真实存储，不是内存库）
        let reopened = sqlite_conn(&config).expect("重新打开同一库文件");
        let page =
            execute_sqlite(&reopened, "SELECT id, name FROM t ORDER BY id", 100).expect("查询");
        assert!(page.is_query, "SELECT 应标记为查询结果");
        assert_eq!(page.columns, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(
            page.rows,
            vec![
                vec!["1".to_string(), "甲".to_string()],
                vec!["2".to_string(), "乙".to_string()]
            ]
        );
        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    /// NULL 与二进制不伪装成空字符串/文本
    #[test]
    fn null_and_blob_cells_use_placeholder_text() {
        let path = temp_db_path("cells");
        let conn = sqlite_conn(&sqlite_config(&path)).expect("打开临时库");
        execute_sqlite(&conn, "CREATE TABLE t (a TEXT, b BLOB, c REAL)", 10).expect("建表");
        execute_sqlite(
            &conn,
            "INSERT INTO t (a, b, c) VALUES (NULL, X'0102', 1.5)",
            10,
        )
        .expect("插入");
        let page = execute_sqlite(&conn, "SELECT a, b, c FROM t", 10).expect("查询");
        assert_eq!(
            page.rows,
            vec![vec![
                "NULL".to_string(),
                "<blob 2 bytes>".to_string(),
                "1.5".to_string()
            ]]
        );
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    /// 超出 max_rows 要标记截断，而不是静默少给几行
    #[test]
    fn row_limit_marks_result_truncated() {
        let path = temp_db_path("limit");
        let conn = sqlite_conn(&sqlite_config(&path)).expect("打开临时库");
        execute_sqlite(&conn, "CREATE TABLE t (n INTEGER)", 10).expect("建表");
        execute_sqlite(&conn, "INSERT INTO t (n) VALUES (1), (2), (3)", 10).expect("插入");
        let page = execute_sqlite(&conn, "SELECT n FROM t ORDER BY n", 2).expect("查询");
        assert_eq!(page.rows.len(), 2, "只返回上限行数");
        assert!(page.truncated, "超上限必须标记 truncated");
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    /// 错误的 SQL 必须返回错误（含表不存在），不得伪装成空结果
    #[test]
    fn invalid_sql_returns_error() {
        let path = temp_db_path("invalid");
        let conn = sqlite_conn(&sqlite_config(&path)).expect("打开临时库");
        let query_err = execute_sqlite(&conn, "SELECT * FROM 不存在的表", 10)
            .expect_err("查询不存在的表必须报错");
        assert!(
            query_err.contains("语句解析失败") || query_err.contains("查询失败"),
            "错误信息应说明失败阶段: {query_err}"
        );
        let write_err = execute_sqlite(&conn, "INSERT INTO 不存在的表 (x) VALUES (1)", 10)
            .expect_err("写入不存在的表必须报错");
        assert!(
            write_err.contains("执行失败"),
            "错误信息应说明失败阶段: {write_err}"
        );
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
}
