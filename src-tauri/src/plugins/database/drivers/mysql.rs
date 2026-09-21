//! MySQL / PolarDB(MySQL 兼容) 驱动：连接池构建 + 查询执行 + 单元格字符串化

use mysql_async::prelude::Queryable;
use mysql_async::{Opts, OptsBuilder};

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
    let timeout_ms = config.connect_timeout_ms.clamp(1000, 120_000);
    log::debug!("MySQL 开始连接 timeout_ms={timeout_ms}");
    let t0 = std::time::Instant::now();
    let mut conn = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        pool.get_conn(),
    )
    .await
    .map_err(|_| format!("MySQL 连接超时（{timeout_ms} ms）"))?
    .map_err(|e| format!("MySQL 连接失败: {e}"))?;
    log::debug!("连接建立耗时 {:?}，预检查询...", t0.elapsed());
    let _: String = conn
        .query_first::<String, _>("SELECT 1")
        .await
        .map_err(|e| format!("MySQL 预检失败: {e}"))?
        .ok_or("预检无结果")?;
    log::debug!("预检完成，总耗时 {:?}", t0.elapsed());
    Ok(pool)
}

/// 基于真实返回列读取结果，支持 RETURNING/SHOW，不按 SQL 前缀猜测。
pub(crate) async fn execute_mysql_conn(
    conn: &mut mysql_async::Conn,
    sql: &str,
    max_rows: u64,
) -> Result<QueryResult, String> {
    use crate::plugins::database::results::ResultBudget;
    let mut budget = ResultBudget::new(max_rows);
    let mut outcomes = Vec::new();
    for (statement_index, sql) in crate::plugins::database::sql_analysis::split(
        crate::plugins::database::models::DbType::Mysql,
        sql,
    )?
    .into_iter()
    .enumerate()
    {
        let outcome: Result<(), String> = async {
            let mut query = conn
                .query_iter(&sql)
                .await
                .map_err(|e| format!("MySQL: {e}"))?;
            loop {
                let mut result = QueryResult::empty();
                result.statement_index = Some(statement_index);
                result.columns = query
                    .columns_ref()
                    .iter()
                    .map(|c| c.name_str().to_string())
                    .collect();
                result.column_types = query
                    .columns_ref()
                    .iter()
                    .map(|c| format!("{:?}", c.column_type()))
                    .collect();
                result.is_query = !result.columns.is_empty();
                let affected = query.affected_rows();
                let binary: Vec<bool> = query
                    .columns_ref()
                    .iter()
                    .map(|c| c.character_set() == 63)
                    .collect();
                let mut count = 0;
                while let Some(row) = query
                    .next()
                    .await
                    .map_err(|e| format!("MySQL 读取结果: {e}"))?
                {
                    count += 1;
                    let values = (0..row.len())
                        .map(|i| {
                            let value = row.get::<MysqlValue, usize>(i).unwrap_or(MysqlValue::NULL);
                            mysql_value(
                                value,
                                binary.get(i).copied().unwrap_or(false),
                                result.column_types.get(i).map(String::as_str).unwrap_or(""),
                            )
                        })
                        .collect();
                    budget.push(&mut result, values);
                }
                result.rows_affected = if result.is_query { count } else { affected };
                if outcomes.len() >= 500 {
                    query
                        .drop_result()
                        .await
                        .map_err(|e| format!("MySQL 协议收尾: {e}"))?;
                    return Err("结果集超过 500 个展示上限；已完成协议收尾，后续结果未保留".into());
                }
                outcomes.push(result);
                if query.is_empty() {
                    break;
                }
            }
            query
                .drop_result()
                .await
                .map_err(|e| format!("MySQL 协议收尾: {e}"))?;
            Ok(())
        }
        .await;
        if let Err(error) = outcome {
            let mut result = QueryResult::failed(error);
            result.statement_index = Some(statement_index);
            outcomes.push(result);
            break;
        }
    }
    Ok(QueryResult::script(outcomes))
}

/// 无损映射 MySQL 值；大整数和精确小数保持字符串。
pub(crate) fn mysql_value(
    value: MysqlValue,
    binary: bool,
    native: &str,
) -> crate::plugins::database::models::DbValue {
    use crate::plugins::database::models::DbValue;
    match value {
        MysqlValue::NULL => DbValue::null(),
        MysqlValue::Int(v) => DbValue::text("integer", v.to_string()),
        MysqlValue::UInt(v) => DbValue::text("integer", v.to_string()),
        MysqlValue::Float(v) => DbValue::text("float", v.to_string()),
        MysqlValue::Double(v) => DbValue::text("float", v.to_string()),
        MysqlValue::Bytes(bytes) => {
            // 文本协议将数值也编码为 Bytes；先看类型，再判断 binary charset。
            let kind = if native.contains("DECIMAL") {
                "decimal"
            } else if ["TINY", "SHORT", "LONG", "INT24", "YEAR"]
                .iter()
                .any(|t| native.contains(t))
            {
                "integer"
            } else if native.contains("FLOAT") || native.contains("DOUBLE") {
                "float"
            } else if native.contains("JSON") {
                "json"
            } else if ["DATE", "TIME"].iter().any(|t| native.contains(t)) {
                "temporal"
            } else if binary {
                return DbValue::binary(&bytes);
            } else {
                "text"
            };
            match String::from_utf8(bytes) {
                Ok(text) => DbValue::text(kind, text),
                Err(error) => DbValue::binary(error.as_bytes()),
            }
        }
        temporal => DbValue::text(
            "temporal",
            temporal.as_sql(false).trim_matches('\'').to_string(),
        ),
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
