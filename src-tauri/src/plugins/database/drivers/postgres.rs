//! PostgreSQL 驱动：连接池构建 + 查询执行 + 单元格字符串化

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
        .connect_timeout(Duration::from_millis(
            config.connect_timeout_ms.clamp(1000, 120_000),
        ));

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
            Duration::from_millis(config.connect_timeout_ms.clamp(1000, 120_000)),
            pool.get(),
        )
        .await
        .map_err(|_| {
            format!(
                "PostgreSQL 连接超时（{} ms）",
                config.connect_timeout_ms.clamp(1000, 120_000)
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
        Duration::from_millis(config.connect_timeout_ms.clamp(1000, 120_000)),
        pool.get(),
    )
    .await
    .map_err(|_| {
        format!(
            "PostgreSQL 连接超时（{} ms）",
            config.connect_timeout_ms.clamp(1000, 120_000)
        )
    })?
    .map_err(|e| format!("PostgreSQL 连接失败: {e}"))?;
    Ok(pool)
}

/// 在独占的工作连接上逐条执行；使用文本协议保留 NUMERIC/UUID/扩展类型精度。
pub(crate) async fn execute_postgres_client(
    client: &tokio_postgres::Client,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    use crate::plugins::database::{models::DbValue, results::ResultBudget};
    use futures_util::TryStreamExt;
    let mut budget = ResultBudget::new(max_rows);
    let mut outcomes = Vec::new();
    for sql in crate::plugins::database::sql_analysis::split(
        crate::plugins::database::models::DbType::Postgresql,
        sql,
    )? {
        let outcome: Result<QueryResult, String> = async {
            // prepare 只读取列元数据，不执行 SQL；实际语句只通过 simple_query_raw 执行一次。
            let prepared = client.prepare(&sql).await.map_err(pg_error)?;
            let mut result = QueryResult::empty();
            result.column_types = prepared
                .columns()
                .iter()
                .map(|c| c.type_().name().to_string())
                .collect();
            let stream = client.simple_query_raw(&sql).await.map_err(pg_error)?;
            futures_util::pin_mut!(stream);
            while let Some(message) = stream.try_next().await.map_err(pg_error)? {
                match message {
                    tokio_postgres::SimpleQueryMessage::RowDescription(columns) => {
                        result.columns = columns.iter().map(|c| c.name().to_string()).collect();
                        result.is_query = true;
                    }
                    tokio_postgres::SimpleQueryMessage::Row(row) => {
                        let mut values = Vec::with_capacity(row.len());
                        for i in 0..row.len() {
                            let value = row.try_get(i).map_err(pg_error)?;
                            let native = result
                                .column_types
                                .get(i)
                                .map(String::as_str)
                                .unwrap_or("text");
                            values.push(match value {
                                None => DbValue::null(),
                                Some(value) => {
                                    let kind = match native {
                                        "int2" | "int4" | "int8" | "oid" => "integer",
                                        "numeric" | "money" => "decimal",
                                        "float4" | "float8" => "float",
                                        "bool" => "boolean",
                                        "json" | "jsonb" => "json",
                                        "bytea" => "binary",
                                        "date" | "time" | "timetz" | "timestamp"
                                        | "timestamptz" | "interval" => "temporal",
                                        _ => "text",
                                    };
                                    let text = if kind == "binary" {
                                        value.strip_prefix("\\x").unwrap_or(value).to_string()
                                    } else if kind == "boolean" {
                                        (value == "t").to_string()
                                    } else {
                                        value.to_string()
                                    };
                                    DbValue::text(kind, text)
                                }
                            });
                        }
                        budget.push(&mut result, values);
                    }
                    tokio_postgres::SimpleQueryMessage::CommandComplete(count) => {
                        result.rows_affected = count
                    }
                    _ => {}
                }
            }
            Ok(result)
        }
        .await;
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

fn pg_error(error: tokio_postgres::Error) -> String {
    match error.as_db_error() {
        Some(db) => format!("PostgreSQL [{}]: {}", db.code().code(), db.message()),
        None => format!("PostgreSQL: {error}"),
    }
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
