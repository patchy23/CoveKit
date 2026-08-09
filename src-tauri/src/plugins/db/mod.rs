//! SQLite 数据库模块（第二批，sqlx 统一三方言——M3 接 MySQL/PG 时新增 ConnectOptions 即可）
//! 连接管理（单连接）+ 表列表 + SQL 执行（查询/非查询自动识别）+ 表数据浏览。
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

mod models;

use crate::plugins::db::models::{DbOpenResult, DbQueryResult};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::{Column, Row, Sqlite};
use std::path::Path;
use std::sync::Mutex;
use tauri::State;

/// 当前打开的数据库连接（State 注入；None = 未连接）
pub struct DbState(pub Mutex<Option<SqlitePool>>);

/// 取连接池（惰性初始化，路径不存在自动创建）
fn pool(state: &State<'_, DbState>) -> Result<sqlx::Pool<Sqlite>, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .cloned()
        .ok_or_else(|| "未连接数据库".into())
}

/// 打开（不存在则创建）SQLite 文件并列出表
#[tauri::command]
pub async fn db_open(state: State<'_, DbState>, path: String) -> Result<DbOpenResult, String> {
    if path.trim().is_empty() {
        return Ok(DbOpenResult {
            ok: false,
            tables: Vec::new(),
            error: Some("路径不能为空".into()),
        });
    }
    let opts = SqliteConnectOptions::new()
        .filename(Path::new(&path))
        .create_if_missing(true);
    let p = SqlitePool::connect_with(opts)
        .await
        .map_err(|e| format!("打开失败: {e}"))?;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&p)
    .await
    .map_err(|e| format!("读取表列表失败: {e}"))?;

    *state.0.lock().map_err(|e| e.to_string())? = Some(p);
    Ok(DbOpenResult {
        ok: true,
        tables,
        error: None,
    })
}

/// 关闭当前连接
#[tauri::command]
pub async fn db_close(state: State<'_, DbState>) -> Result<(), String> {
    let p = {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(p) = p {
        p.close().await;
    }
    Ok(())
}

/// 当前库的表列表
#[tauri::command]
pub async fn db_tables(state: State<'_, DbState>) -> Result<Vec<String>, String> {
    let p = pool(&state)?;
    sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&p)
    .await
    .map_err(|e| e.to_string())
}

/// 单元格值统一转字符串（NULL → "NULL"，二进制 → 长度标注）
fn cell_str(row: &sqlx::sqlite::SqliteRow, i: usize) -> String {
    if let Ok(v) = row.try_get::<i64, _>(i) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f64, _>(i) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<String, _>(i) {
        return v;
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(i) {
        return format!("<blob {} bytes>", v.len());
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(i) {
        return v.map_or("NULL".into(), |s| s);
    }
    String::new()
}

/// 执行 SQL：查询语句（SELECT/PRAGMA/EXPLAIN/WITH）返回表格，其余返回影响行数
#[tauri::command]
pub async fn db_execute(state: State<'_, DbState>, sql: String) -> Result<DbQueryResult, String> {
    let p = pool(&state)?;
    let trimmed = sql.trim();
    let upper = trimmed.to_uppercase();
    let is_query = ["SELECT", "PRAGMA", "EXPLAIN", "WITH", "SHOW", "DESCRIBE"]
        .iter()
        .any(|k| upper.starts_with(k));

    if is_query {
        match sqlx::query(trimmed).fetch_all(&p).await {
            Ok(rows) => {
                let columns = rows
                    .first()
                    .map(|r| {
                        r.columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let data = rows
                    .iter()
                    .map(|r| {
                        (0..columns.len())
                            .map(|i| cell_str(r, i))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                Ok(DbQueryResult {
                    ok: true,
                    columns,
                    rows: data,
                    rows_affected: rows.len() as u64,
                    is_query: true,
                    error: None,
                })
            }
            Err(e) => Ok(DbQueryResult {
                ok: false,
                columns: Vec::new(),
                rows: Vec::new(),
                rows_affected: 0,
                is_query: true,
                error: Some(e.to_string()),
            }),
        }
    } else {
        match sqlx::query(trimmed).execute(&p).await {
            Ok(res) => Ok(DbQueryResult {
                ok: true,
                columns: Vec::new(),
                rows: Vec::new(),
                rows_affected: res.rows_affected(),
                is_query: false,
                error: None,
            }),
            Err(e) => Ok(DbQueryResult {
                ok: false,
                columns: Vec::new(),
                rows: Vec::new(),
                rows_affected: 0,
                is_query: false,
                error: Some(e.to_string()),
            }),
        }
    }
}

/// 浏览表数据（SELECT * LIMIT n，表名做引号转义防注入）
#[tauri::command]
pub async fn db_query_table(
    state: State<'_, DbState>,
    table: String,
    limit: Option<u32>,
) -> Result<DbQueryResult, String> {
    let safe = table.replace('"', "");
    if safe.is_empty() {
        return Ok(DbQueryResult {
            ok: false,
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            is_query: true,
            error: Some("表名不能为空".into()),
        });
    }
    let n = limit.unwrap_or(100).min(1000);
    let sql = format!("SELECT * FROM \"{safe}\" LIMIT {n}");
    db_execute(state, sql).await
}

/// 分派数据库插件命令。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![db_open, db_close, db_tables, db_execute, db_query_table];
    handler(invoke)
}

/// 插件注册：命令 + 数据库连接 State（惰性初始化）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        ("db_open", "打开数据库（路径不存在自动创建）"),
        ("db_close", "关闭数据库"),
        ("db_tables", "表列表"),
        ("db_execute", "执行 SQL（查询/非查询自动识别）"),
        ("db_query_table", "浏览表数据"),
    ])
    .expect("IPC 命令重复注册");
    builder.manage(DbState(std::sync::Mutex::new(None)))
}
