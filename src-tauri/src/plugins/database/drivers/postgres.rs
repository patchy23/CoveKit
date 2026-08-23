//! PostgreSQL 驱动：连接池构建 + 查询执行 + 单元格字符串化

use crate::plugins::database::dialect::dialect_for;
use std::time::Duration;

use crate::plugins::database::models::{ConnConfig, QueryResult};

/// 按连接配置创建并预检 PostgreSQL 连接池，按需启用 TLS。
pub(crate) async fn pg_pool(
    config: &ConnConfig,
    password: &str,
) -> Result<deadpool_postgres::Pool, String> {
    use tokio_postgres::NoTls;

    let mut builder = tokio_postgres::Config::new();
    builder
        .host(&config.host)
        .port(config.port)
        .user(&config.username)
        .password(password)
        .dbname(&config.database)
        .connect_timeout(Duration::from_millis(config.connect_timeout_ms.max(30000)));

    if config.ssl {
        // TLS：rustls ring 后端（tokio-postgres-rustls 默认 feature 即 ring）
        let config_ref = builder;
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);
        let manager = deadpool_postgres::Manager::new(config_ref, tls);
        let pool = deadpool_postgres::Pool::builder(manager)
            .max_size(4)
            .build()
            .map_err(|e| format!("PG 连接池构建失败: {e}"))?;
        // 借出一个连接校验连通性（校验后自动归还池；带超时防挂起）
        let _check = tokio::time::timeout(
            Duration::from_millis(config.connect_timeout_ms.max(30000)),
            pool.get(),
        )
        .await
        .map_err(|_| {
            format!(
                "PostgreSQL 连接超时（{} ms）",
                config.connect_timeout_ms.max(30000)
            )
        })?
        .map_err(|e| format!("PostgreSQL 连接失败: {e}"))?;
        return Ok(pool);
    }

    let manager = deadpool_postgres::Manager::new(builder.clone(), NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .map_err(|e| format!("PG 连接池构建失败: {e}"))?;
    // 借出一个连接校验连通性（校验后自动归还池；带超时防挂起）
    let _check = tokio::time::timeout(
        Duration::from_millis(config.connect_timeout_ms.max(30000)),
        pool.get(),
    )
    .await
    .map_err(|_| {
        format!(
            "PostgreSQL 连接超时（{} ms）",
            config.connect_timeout_ms.max(30000)
        )
    })?
    .map_err(|e| format!("PostgreSQL 连接失败: {e}"))?;
    Ok(pool)
}

/// 打开 SQLite 文件（路径不存在自动创建；Arc 共享供会话表按值克隆）
pub(crate) async fn execute_postgres(
    pool: &deadpool_postgres::Pool,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    let dialect =
        dialect_for(crate::plugins::database::models::DbType::Postgresql).expect("pg 方言存在");
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
    let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
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
            let rows = client
                .query(&stmt, &[])
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            let columns: Vec<String> = rows
                .first()
                .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default();
            let truncated = rows.len() as u64 > max_rows;
            let data = rows
                .iter()
                .take(max_rows as usize)
                .map(|row| {
                    (0..columns.len())
                        .map(|i| pg_cell_str(row, i))
                        .collect::<Vec<_>>()
                })
                .collect();
            result = QueryResult {
                ok: true,
                columns,
                rows: data,
                rows_affected: rows.len() as u64,
                is_query: true,
                duration_ms: 0,
                truncated,
                error: None,
            };
        } else {
            let affected = client
                .execute(&stmt, &[])
                .await
                .map_err(|e| format!("执行失败: {e}"))?;
            result.rows_affected += affected;
            result.is_query = false;
        }
    }
    Ok(result)
}

/// SQLite：rusqlite 锁内执行
pub(crate) fn pg_cell_str(row: &tokio_postgres::Row, index: usize) -> String {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
    if let Ok(v) = row.try_get::<usize, Option<String>>(index) {
        return v.unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<i64>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |n| n.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<f64>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |n| n.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<bool>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |b| b.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<NaiveDateTime>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |d| d.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<DateTime<Utc>>>(index) {
        return v.map_or_else(
            || "NULL".to_string(),
            |d| d.format("%Y-%m-%d %H:%M:%S").to_string(),
        );
    }
    if let Ok(v) = row.try_get::<usize, Option<NaiveDate>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |d| d.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<Vec<u8>>>(index) {
        return v.map_or_else(
            || "NULL".to_string(),
            |b| format!("<blob {} bytes>", b.len()),
        );
    }
    if let Ok(v) = row.try_get::<usize, Option<serde_json::Value>>(index) {
        return v.map_or_else(|| "NULL".to_string(), |j| j.to_string());
    }
    "NULL".to_string()
}

/// SQLite 单元格 → 字符串
pub(crate) async fn pg_keys(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let rows = client
        .query(
            "SELECT a.attname, \
                    CASE WHEN ix.indisprimary THEN 'PK' WHEN ix.indisunique THEN 'UK' ELSE '' END \
             FROM pg_catalog.pg_index ix \
             JOIN pg_catalog.pg_class c ON c.oid = ix.indrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(ix.indkey) \
             WHERE n.nspname = $1 AND c.relname = $2 AND (ix.indisprimary OR ix.indisunique)",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("键标记探测失败: {e}"))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let name: String = row.try_get::<usize, String>(0).ok()?;
            let key: String = row.try_get::<usize, String>(1).ok()?;
            if key.is_empty() {
                None
            } else {
                Some((name, key))
            }
        })
        .collect())
}

/// 通用字符串列表查询（mysql）
pub(crate) async fn query_strings_pg(
    pool: &deadpool_postgres::Pool,
    sql: &str,
) -> Result<Vec<String>, String> {
    let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
    let rows = client
        .query(sql, &[])
        .await
        .map_err(|e| format!("查询失败: {e}"))?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<usize, String>(0).ok())
        .collect())
}
