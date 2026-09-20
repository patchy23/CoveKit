//! 表浏览：稳定键排序、额外一行探测下一页，精确统计独立执行。
use super::{bound::BoundConnection, metadata::columns_for_entry, table_sql};
use crate::plugins::database::{
    drivers::{self, DbSession, DbState},
    models::{DbTablePage, DbType, TableOptions},
};
use std::time::Instant;
use tauri::State;

/// 按方言引用标识符；不从显示名称反推 schema/对象身份。
pub(crate) fn quote(kind: DbType, name: &str) -> String {
    if matches!(kind, DbType::Mysql | DbType::Polardb) {
        format!("`{}`", name.replace('`', "``"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// 表数据每次只读取 pageSize+1，不用估算 COUNT 决定能否翻页。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_data(
    cancel_state: State<'_, crate::plugins::database::drivers::DbCancelState>,
    app: tauri::AppHandle,
    secrets_state: State<'_, crate::plugins::database::secrets::SecretsState>,
    database: Option<String>,
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
    page: u32,
    page_size: u32,
    options: Option<TableOptions>,
) -> Result<DbTablePage, String> {
    let mut task = super::bound::BoundTask::register(
        &cancel_state,
        uuid::Uuid::new_v4().to_string(),
        &conn_id,
    )?;
    let entry = crate::plugins::database::catalog::scoped_session(
        &app,
        &state,
        &secrets_state,
        &conn_id,
        database.as_deref(),
    )
    .await?;
    let started = Instant::now();
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let columns = columns_for_entry(&entry, schema.clone(), table.clone()).await?;
    if columns.is_empty() {
        return Err("表不存在、不可见或没有可浏览列，请刷新结构".into());
    }
    let kind = entry.config.db_type;
    let schema = schema.unwrap_or_else(|| match kind {
        DbType::Postgresql => "public".into(),
        DbType::Sqlite => String::new(),
        _ => entry.config.database.clone(),
    });
    let qualified = if schema.is_empty() || kind == DbType::Sqlite {
        quote(kind, &table)
    } else {
        format!("{}.{}", quote(kind, &schema), quote(kind, &table))
    };
    let keys: Vec<_> = columns
        .iter()
        .filter(|c| c.key == "PK")
        .map(|c| quote(kind, &c.name))
        .collect();
    let options = options.unwrap_or_default();
    let (filter, params) = table_sql::predicates(kind, &columns, &options)?;
    let order = table_sql::ordering(kind, &columns, &options)?;
    let offset = u64::from(page - 1) * u64::from(page_size);
    let count = u64::from(page_size) + 1;
    let sql = if kind == DbType::Oracle {
        format!("SELECT * FROM {qualified}{filter}{order} OFFSET {offset} ROWS FETCH NEXT {count} ROWS ONLY")
    } else {
        format!(
            "SELECT {} FROM {qualified}{filter}{order} LIMIT {count} OFFSET {offset}",
            table_sql::projection(kind, &columns)
        )
    };
    let mut result = if let DbSession::Agent { client, session_id } = &entry.session {
        if !params.is_empty() {
            return Err("DB_UNSUPPORTED: 该驱动未提供筛选参数绑定".into());
        }
        drivers::execute_agent(client, session_id, &sql, count, kind).await?
    } else {
        let mut conn = BoundConnection::open(&entry).await?;
        task.bind(&conn, &entry).await?;
        task.query_limited(&mut conn, &sql, &params, count).await?
    };
    if kind == DbType::Postgresql {
        table_sql::restore_pg_types(&mut result, &columns);
    }
    if !result.ok {
        return Err(result.error.unwrap_or_else(|| "表查询失败".into()));
    }
    if result.truncated && result.rows.len() <= page_size as usize {
        return Err("当前页数据超过 8 MiB 浏览上限，请减小每页行数或通过 SQL 选择较小字段".into());
    }
    let has_more = result.rows.len() > page_size as usize;
    result.rows.truncate(page_size as usize);
    result.values.truncate(page_size as usize);
    let total = offset + result.rows.len() as u64 + u64::from(has_more);
    Ok(DbTablePage {
        query_sql: sql,
        query_params: params,
        columns: result.columns,
        rows: result.rows,
        values: result.values,
        has_more,
        total,
        total_kind: if has_more { "lowerBound" } else { "pageEnd" }.into(),
        stable_order: !keys.is_empty(),
        page,
        page_size,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
    })
}

/// 生成明确的对象限定名。
pub(crate) fn qualified(
    entry: &crate::plugins::database::drivers::DbSessionEntry,
    schema: Option<&str>,
    table: &str,
) -> String {
    let kind = entry.config.db_type;
    let schema = schema.unwrap_or_else(|| match kind {
        DbType::Postgresql => "public",
        DbType::Sqlite => "",
        _ => &entry.config.database,
    });
    if schema.is_empty() || kind == DbType::Sqlite {
        quote(kind, table)
    } else {
        format!("{}.{}", quote(kind, schema), quote(kind, table))
    }
}

/// 精确统计显式触发，保留筛选；字符串返回避免 JS 数值精度丢失。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_count(
    cancel_state: State<'_, crate::plugins::database::drivers::DbCancelState>,
    request_id: String,
    app: tauri::AppHandle,
    secrets_state: State<'_, crate::plugins::database::secrets::SecretsState>,
    state: State<'_, DbState>,
    conn_id: String,
    database: Option<String>,
    schema: Option<String>,
    table: String,
    options: Option<TableOptions>,
) -> Result<String, String> {
    let mut task = super::bound::BoundTask::register(&cancel_state, request_id, &conn_id)?;
    let entry =
        super::scoped_session(&app, &state, &secrets_state, &conn_id, database.as_deref()).await?;
    let columns = columns_for_entry(&entry, schema.clone(), table.clone()).await?;
    let (filter, params) =
        table_sql::predicates(entry.config.db_type, &columns, &options.unwrap_or_default())?;
    let count = if entry.config.db_type == DbType::Postgresql {
        "CAST(COUNT(*) AS TEXT)"
    } else {
        "COUNT(*)"
    };
    let sql = format!(
        "SELECT {count} AS total FROM {}{filter}",
        qualified(&entry, schema.as_deref(), &table)
    );
    let mut conn = BoundConnection::open(&entry).await?;
    task.bind(&conn, &entry).await?;
    let result = task.query(&mut conn, &sql, &params).await?;
    result
        .rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .ok_or_else(|| "统计没有返回结果".into())
}
