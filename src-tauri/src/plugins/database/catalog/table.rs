//! 数据库查询与目录 · table

use super::session;
use crate::plugins::database::dialect::dialect_or_err;
use crate::plugins::database::drivers;
use crate::plugins::database::drivers::DbSession;
use crate::plugins::database::drivers::DbState;
use crate::plugins::database::models::DbTablePage;
use mysql_async::prelude::Queryable;
use std::time::Instant;
use tauri::State;

/// 表数据分页（native 走 LIMIT/OFFSET + COUNT；agent 走 maxRows 抓取后切片）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_data(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
    page: u32,
    page_size: u32,
) -> Result<DbTablePage, String> {
    let entry = session(&state, &conn_id)?;
    let started = Instant::now();
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);

    let (columns, rows, total) = match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let qualified = format!(
                "{}.{}",
                dialect.quote_ident(&database),
                dialect.quote_ident(&table)
            );
            let sql = dialect.paginate(
                &format!("SELECT * FROM {qualified}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?;
            let result = drivers::mysql::execute_mysql(pool, &sql, page_size as u64).await?;
            // 行数：information_schema.TABLES.TABLE_ROWS（估算；预编译语句下该列是 BIGINT
            // 而非字符串，直接取 String 会 FromRow panic——按 Value 取再转字符串，NULL 走兜底）
            let count_row = conn
                .exec_first::<Option<mysql_async::Value>, _, _>(
                    dialect.row_count_sql(),
                    (database.clone(), table.clone()),
                )
                .await
                .ok()
                .flatten()
                .flatten()
                .map(drivers::mysql::mysql_value_str)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(result.rows.len() as u64);
            (result.columns, result.rows, count_row)
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let qualified = format!(
                "{}.{}",
                dialect.quote_ident(&schema),
                dialect.quote_ident(&table)
            );
            let sql = dialect.paginate(
                &format!("SELECT * FROM {qualified}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let result = drivers::postgres::execute_postgres(pool, &sql, page_size as u64).await?;
            let count_row = client
                .query_one(dialect.row_count_sql(), &[&schema, &table])
                .await
                .ok()
                .map(|row| row.try_get::<usize, i64>(0).unwrap_or(0) as u64)
                .unwrap_or(result.rows.len() as u64);
            (result.columns, result.rows, count_row)
        }
        DbSession::Sqlite(conn) => {
            let dialect = dialect_or_err(crate::plugins::database::models::DbType::Sqlite)?;
            let quoted = dialect.quote_ident(&table);
            let sql = dialect.paginate(
                &format!("SELECT * FROM {quoted}"),
                page_size as u64,
                ((page - 1) * page_size) as u64,
            );
            let result = drivers::sqlite::execute_sqlite(conn, &sql, page_size as u64)?;
            let count = {
                let guard = conn.lock().map_err(|e| e.to_string())?;
                guard
                    .query_row(&format!("SELECT COUNT(*) FROM {quoted}"), [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .unwrap_or(result.rows.len() as i64)
                    .max(0) as u64
            };
            (result.columns, result.rows, count)
        }
        DbSession::Redis(_) => {
            return Ok(DbTablePage {
                columns: Vec::new(),
                rows: Vec::new(),
                total: 0,
                page,
                page_size,
                duration_ms: 0,
                error: Some("Redis 无表数据浏览（请用键浏览面板）".to_string()),
            });
        }
        DbSession::Agent { client, session_id } => {
            // agent：一次抓取 page*page_size 行（上限保护），前端切片
            let schema = schema.unwrap_or_default();
            let qualified = if schema.is_empty() {
                table.clone()
            } else {
                format!("{schema}.{table}")
            };
            let need = (page as u64) * (page_size as u64);
            let value = client
                .execute_query(session_id, &format!("SELECT * FROM {qualified}"), need)
                .await?;
            let columns = value
                .get("columns")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let all_rows = value
                .get("rows")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|row| {
                            row.as_array()
                                .map(|cells| {
                                    cells.iter().map(drivers::json_cell_str).collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let total = all_rows.len() as u64;
            let start = ((page - 1) * page_size) as usize;
            let page_rows = all_rows
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect();
            // 行数探测：SELECT COUNT(*)（失败时用抓取行数）
            let count = client
                .execute_query(session_id, &format!("SELECT COUNT(*) FROM {qualified}"), 1)
                .await
                .ok()
                .and_then(|v| {
                    v.get("rows")
                        .and_then(|r| r.as_array())
                        .and_then(|rows| rows.first())
                        .and_then(|row| row.as_array())
                        .and_then(|row| row.first())
                        .and_then(|cell| cell.as_u64())
                })
                .unwrap_or(total);
            (columns, page_rows, count)
        }
    };

    Ok(DbTablePage {
        columns,
        rows,
        total,
        page,
        page_size,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
    })
}
