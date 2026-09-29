//! MySQL / PolarDB(MySQL 兼容) 驱动：连接池构建 + 查询执行 + 单元格字符串化

use mysql_async::prelude::Queryable;
use mysql_async::{Opts, OptsBuilder};

use mysql_async::Value as MysqlValue;

use crate::plugins::database::models::{ConnConfig, QueryResult};

/// 将 MySQL 协议列类型转换为用户熟悉的 SQL 类型名。
pub(crate) fn mysql_column_type_name(
    column_type: mysql_async::consts::ColumnType,
    binary: bool,
) -> &'static str {
    use mysql_async::consts::ColumnType::*;
    match column_type {
        MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL => "DECIMAL",
        MYSQL_TYPE_TINY => "TINYINT",
        MYSQL_TYPE_SHORT => "SMALLINT",
        MYSQL_TYPE_LONG => "INT",
        MYSQL_TYPE_FLOAT => "FLOAT",
        MYSQL_TYPE_DOUBLE => "DOUBLE",
        MYSQL_TYPE_NULL => "NULL",
        MYSQL_TYPE_TIMESTAMP | MYSQL_TYPE_TIMESTAMP2 => "TIMESTAMP",
        MYSQL_TYPE_LONGLONG => "BIGINT",
        MYSQL_TYPE_INT24 => "MEDIUMINT",
        MYSQL_TYPE_DATE | MYSQL_TYPE_NEWDATE => "DATE",
        MYSQL_TYPE_TIME | MYSQL_TYPE_TIME2 => "TIME",
        MYSQL_TYPE_DATETIME | MYSQL_TYPE_DATETIME2 => "DATETIME",
        MYSQL_TYPE_YEAR => "YEAR",
        MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING => {
            if binary {
                "VARBINARY"
            } else {
                "VARCHAR"
            }
        }
        MYSQL_TYPE_BIT => "BIT",
        MYSQL_TYPE_TYPED_ARRAY => "ARRAY",
        MYSQL_TYPE_VECTOR => "VECTOR",
        MYSQL_TYPE_UNKNOWN => "UNKNOWN",
        MYSQL_TYPE_JSON => "JSON",
        MYSQL_TYPE_ENUM => "ENUM",
        MYSQL_TYPE_SET => "SET",
        MYSQL_TYPE_TINY_BLOB => {
            if binary {
                "TINYBLOB"
            } else {
                "TINYTEXT"
            }
        }
        MYSQL_TYPE_MEDIUM_BLOB => {
            if binary {
                "MEDIUMBLOB"
            } else {
                "MEDIUMTEXT"
            }
        }
        MYSQL_TYPE_LONG_BLOB => {
            if binary {
                "LONGBLOB"
            } else {
                "LONGTEXT"
            }
        }
        MYSQL_TYPE_BLOB => {
            if binary {
                "BLOB"
            } else {
                "TEXT"
            }
        }
        MYSQL_TYPE_STRING => {
            if binary {
                "BINARY"
            } else {
                "CHAR"
            }
        }
        MYSQL_TYPE_GEOMETRY => "GEOMETRY",
    }
}

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
                    .map(|c| {
                        mysql_column_type_name(c.column_type(), c.character_set() == 63).to_string()
                    })
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
                    let mut values = budget.row();
                    for (i, value) in row.unwrap_raw().into_iter().enumerate() {
                        let value = value.ok_or("MySQL 结果缺少单元格")?;
                        append_mysql_value(
                            &mut values,
                            value,
                            binary.get(i).copied().unwrap_or(false),
                            result.column_types.get(i).map(String::as_str).unwrap_or(""),
                        );
                    }
                    budget.finish_row(&mut result, values);
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
            let kind = mysql_bytes_kind(binary, native);
            if kind == "binary" {
                return DbValue::binary(&bytes);
            }
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

fn mysql_bytes_kind(binary: bool, native: &str) -> &'static str {
    match native {
        "DECIMAL" | "NEWDECIMAL" | "MYSQL_TYPE_DECIMAL" | "MYSQL_TYPE_NEWDECIMAL" => "decimal",
        "TINYINT"
        | "SMALLINT"
        | "MEDIUMINT"
        | "INT"
        | "BIGINT"
        | "YEAR"
        | "LONGLONG"
        | "MYSQL_TYPE_TINY"
        | "MYSQL_TYPE_SHORT"
        | "MYSQL_TYPE_INT24"
        | "MYSQL_TYPE_LONG"
        | "MYSQL_TYPE_LONGLONG"
        | "MYSQL_TYPE_YEAR" => "integer",
        "FLOAT" | "DOUBLE" | "MYSQL_TYPE_FLOAT" | "MYSQL_TYPE_DOUBLE" => "float",
        "JSON" | "MYSQL_TYPE_JSON" => "json",
        "DATE"
        | "TIME"
        | "DATETIME"
        | "TIMESTAMP"
        | "MYSQL_TYPE_DATE"
        | "MYSQL_TYPE_NEWDATE"
        | "MYSQL_TYPE_TIME"
        | "MYSQL_TYPE_TIME2"
        | "MYSQL_TYPE_DATETIME"
        | "MYSQL_TYPE_DATETIME2"
        | "MYSQL_TYPE_TIMESTAMP"
        | "MYSQL_TYPE_TIMESTAMP2" => "temporal",
        _ if binary => "binary",
        _ => "text",
    }
}

/// 查询路径移动驱动字段；仅在预算允许时展开二进制，导出路径仍使用完整值转换。
pub(crate) fn append_mysql_value(
    row: &mut crate::plugins::database::results::ResultRow,
    value: MysqlValue,
    binary: bool,
    native: &str,
) {
    match value {
        MysqlValue::Bytes(bytes) => {
            let kind = mysql_bytes_kind(binary, native);
            if kind == "binary" {
                row.binary(&bytes);
            } else {
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        row.value(crate::plugins::database::models::DbValue::text(kind, text));
                    }
                    Err(error) => row.binary(error.as_bytes()),
                }
            }
        }
        scalar => row.value(mysql_value(scalar, binary, native)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::database::results::ResultBudget;

    #[test]
    fn budget_conversion_matches_full_conversion_without_losing_precision() {
        for (value, binary, native) in [
            (
                MysqlValue::Bytes(b"12345678901234567890.01".to_vec()),
                true,
                "NEWDECIMAL",
            ),
            (MysqlValue::Bytes(vec![0, 255]), true, "BLOB"),
            (MysqlValue::Bytes(vec![255]), false, "VAR_STRING"),
            (
                MysqlValue::Bytes("中文".as_bytes().to_vec()),
                false,
                "VAR_STRING",
            ),
            (MysqlValue::UInt(u64::MAX), true, "LONGLONG"),
            (MysqlValue::NULL, false, "VAR_STRING"),
        ] {
            let expected = mysql_value(value.clone(), binary, native);
            let mut budget = ResultBudget::new(1);
            let mut row = budget.row();
            append_mysql_value(&mut row, value, binary, native);
            let mut result = QueryResult::empty();
            budget.finish_row(&mut result, row);
            assert_eq!(result.values[0][0].kind, expected.kind);
            assert_eq!(result.values[0][0].value, expected.value);
            assert_eq!(result.rows[0][0], expected.display());
        }
    }
}
