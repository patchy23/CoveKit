//! SQLite 驱动：文件连接 + 查询执行 + 单元格字符串化

use std::sync::{Arc, Mutex};

use rusqlite::Connection as SqliteConn;

use crate::plugins::database::models::{ConnConfig, QueryResult};

/// 打开 SQLite 文件并包装为插件会话使用的线程安全连接。
pub(crate) fn sqlite_conn(config: &ConnConfig) -> Result<Arc<Mutex<SqliteConn>>, String> {
    let path = &config.host;
    if path.trim().is_empty() {
        return Err("SQLite 文件路径不能为空".to_string());
    }
    let flags = if config.readonly {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
    };
    let conn = SqliteConn::open_with_flags(path, flags)
        .map_err(|e| format!("SQLite 打开失败（{path}）: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// 同步驱动入口；异步调用方必须放到阻塞任务，并使用独立工作连接。
pub(crate) fn execute_sqlite(
    conn: &std::sync::Mutex<rusqlite::Connection>,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    use crate::plugins::database::{models::DbValue, results::ResultBudget};
    let guard = conn.lock().map_err(|e| e.to_string())?;
    let mut outcomes = Vec::new();
    let mut budget = ResultBudget::new(max_rows);
    for sql in crate::plugins::database::sql_analysis::split(
        crate::plugins::database::models::DbType::Sqlite,
        sql,
    )? {
        let outcome: Result<QueryResult, String> = (|| {
            let mut prepared = guard
                .prepare(&sql)
                .map_err(|e| format!("语句解析失败: {e}"))?;
            let mut result = QueryResult::empty();
            result.columns = prepared
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            result.is_query = !result.columns.is_empty();
            let readonly = prepared.readonly();
            if !result.is_query {
                result.rows_affected =
                    prepared.execute([]).map_err(|e| format!("执行失败: {e}"))? as u64;
            } else {
                let mut rows = prepared.query([]).map_err(|e| format!("查询失败: {e}"))?;
                let mut count = 0;
                while let Some(row) = rows.next().map_err(|e| format!("读取结果失败: {e}"))? {
                    count += 1;
                    let mut values = Vec::with_capacity(result.columns.len());
                    for i in 0..result.columns.len() {
                        use rusqlite::types::ValueRef;
                        values.push(match row.get_ref(i).map_err(|e| e.to_string())? {
                            ValueRef::Null => DbValue::null(),
                            ValueRef::Integer(v) => DbValue::text("integer", v.to_string()),
                            ValueRef::Real(v) => DbValue::text("float", v.to_string()),
                            ValueRef::Text(v) => match std::str::from_utf8(v) {
                                Ok(text) => DbValue::text("text", text.to_string()),
                                Err(_) => DbValue::binary(v),
                            },
                            ValueRef::Blob(v) => DbValue::binary(v),
                        });
                    }
                    budget.push(&mut result, values);
                }
                result.rows_affected = if readonly { count } else { guard.changes() };
            }
            Ok(result)
        })();
        match outcome {
            Ok(result) => outcomes.push(result),
            Err(error) => {
                outcomes.push(QueryResult::failed(error));
                break;
            }
        }
    }
    Ok(QueryResult::script(outcomes))
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
            credential_id: None,
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
        path.push(format!("covekit-db-test-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// 空路径必须直接报错，不能静默建一个库文件
    #[test]
    fn empty_path_is_rejected() {
        let config = ConnConfig {
            credential_id: None,
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
    fn null_and_blob_cells_preserve_typed_values() {
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
                "0x0102".to_string(),
                "1.5".to_string()
            ]]
        );
        assert_eq!(page.values[0][0].kind, "null");
        assert_eq!(page.values[0][1].kind, "binary");
        assert_eq!(page.values[0][1].value.as_deref(), Some("0102"));
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
            .expect("执行通道应返回语句错误结果");
        assert!(!query_err.ok);
        let query_err = query_err.error.expect("失败结果必须包含错误原因");
        assert!(
            query_err.contains("语句解析失败") || query_err.contains("查询失败"),
            "错误信息应说明失败阶段: {query_err}"
        );
        let write_err = execute_sqlite(&conn, "INSERT INTO 不存在的表 (x) VALUES (1)", 10)
            .expect("执行通道应返回语句错误结果");
        assert!(!write_err.ok);
        let write_err = write_err.error.expect("失败结果必须包含错误原因");
        assert!(
            write_err.contains("语句解析失败") || write_err.contains("执行失败"),
            "错误信息应说明失败阶段: {write_err}"
        );
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
}
