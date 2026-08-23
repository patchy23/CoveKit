//! MySQL / PolarDB(MySQL 兼容) 驱动：连接池构建 + 查询执行 + 单元格字符串化

use mysql_async::prelude::Queryable;
use mysql_async::{Opts, OptsBuilder};

use crate::plugins::database::dialect::dialect_for;
use mysql_async::Value as MysqlValue;

use crate::plugins::database::models::{ConnConfig, QueryResult};

/// 按连接配置创建并预检 MySQL / PolarDB 连接池。
pub(crate) async fn mysql_pool(
    config: &ConnConfig,
    password: &str,
) -> Result<mysql_async::Pool, String> {
    // 连接参数对齐 dbx：库名可空（空则不 USE，避免「Unknown database」拒连）；
    // tcp keepalive 30s + nodelay；超时由下方预检的 tokio timeout 兜底
    let mut builder = OptsBuilder::default()
        .ip_or_hostname(config.host.clone())
        .tcp_port(config.port)
        .user(Some(config.username.clone()))
        .pass(Some(password.to_string()))
        .prefer_socket(false)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .tcp_nodelay(true);
    let database = config.database.trim();
    if !database.is_empty() {
        builder = builder.db_name(Some(database.to_string()));
    }
    if config.ssl {
        builder = builder.ssl_opts(Some(mysql_async::SslOpts::default()));
    }
    let opts: Opts = builder.into();
    let pool = mysql_async::Pool::new(opts);
    // 预检一条查询，验证凭据（带连接超时，防挂起）
    let timeout_ms = config.connect_timeout_ms.max(30000);
    eprintln!(
        "[mysql] 开始连接 {}:{} user={} db={:?}（超时 {timeout_ms} ms）",
        config.host, config.port, config.username, database
    );
    let t0 = std::time::Instant::now();
    let mut conn = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        pool.get_conn(),
    )
    .await
    .map_err(|_| format!("MySQL 连接超时（{timeout_ms} ms）"))?
    .map_err(|e| {
        eprintln!("[mysql] 连接失败：{e}");
        format!("MySQL 连接失败: {e}")
    })?;
    eprintln!("[mysql] 连接建立耗时 {:?}，预检查询...", t0.elapsed());
    let _: String = conn
        .query_first::<String, _>("SELECT 1")
        .await
        .map_err(|e| format!("MySQL 预检失败: {e}"))?
        .ok_or("预检无结果")?;
    eprintln!("[mysql] 预检完成，总耗时 {:?}", t0.elapsed());
    Ok(pool)
}

/// 构建 deadpool-postgres 连接池（TLS 走 rustls ring，与 http_ws 一致）
pub(crate) async fn execute_mysql(
    pool: &mysql_async::Pool,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect =
        dialect_for(crate::plugins::database::models::DbType::Mysql).expect("mysql 方言存在");
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
    let mut conn = pool
        .get_conn()
        .await
        .map_err(|e| format!("取连接失败: {e}"))?;
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
            let mut query = conn
                .query_iter(&stmt)
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            let columns = query
                .columns_ref()
                .iter()
                .map(|c| c.name_str().to_string())
                .collect::<Vec<_>>();
            let mut rows = Vec::new();
            let mut truncated = false;
            let mut count = 0u64;
            while let Some(row) = query
                .next()
                .await
                .map_err(|e| format!("读取结果失败: {e}"))?
            {
                count += 1;
                if count > max_rows {
                    truncated = true;
                    break;
                }
                let mut cells = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    cells.push(mysql_cell_str(&row, i));
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
            conn.query_drop(&stmt)
                .await
                .map_err(|e| format!("执行失败: {e}"))?;
            result.rows_affected += conn.affected_rows();
            result.is_query = false;
        }
    }
    Ok(result)
}

/// MySQL 单元格 → 展示字符串（NULL 显示 "NULL"；Bytes 按 UTF-8，其余走 as_sql）
pub(crate) fn mysql_cell_str(row: &mysql_async::Row, index: usize) -> String {
    let Some(value) = row.get::<MysqlValue, usize>(index) else {
        return "NULL".to_string();
    };
    mysql_value_str(value)
}

/// MySQL Value → 字符串（类型无关，供单元格/标量取值共用；NULL → "NULL"）
pub(crate) fn mysql_value_str(value: MysqlValue) -> String {
    match value {
        MysqlValue::NULL => "NULL".to_string(),
        MysqlValue::Bytes(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "NULL".to_string()),
        other => other.as_sql(false),
    }
}

/// MySQL 行内字符串字段（get 返回 Option<T>，NULL → 空串）
pub(crate) fn mysql_str(row: &mysql_async::Row, index: usize) -> String {
    row.get::<Option<String>, usize>(index)
        .flatten()
        .unwrap_or_default()
}

/// MySQL 键标记（COLUMN_KEY: PRI → PK, UNI → UK）
pub(crate) fn mysql_key(row: &mysql_async::Row, index: usize) -> String {
    match mysql_str(row, index).as_str() {
        "PRI" => "PK".to_string(),
        "UNI" => "UK".to_string(),
        _ => "—".to_string(),
    }
}

/// PostgreSQL 单元格 → 字符串（按类型链式 try_get）
pub(crate) async fn query_strings_mysql(
    pool: &mysql_async::Pool,
    sql: &str,
) -> Result<Vec<String>, String> {
    let mut conn = pool
        .get_conn()
        .await
        .map_err(|e| format!("取连接失败: {e}"))?;
    conn.query::<String, _>(sql)
        .await
        .map_err(|e| format!("查询失败: {e}"))
}
