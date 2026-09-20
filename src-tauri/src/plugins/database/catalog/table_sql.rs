//! 服务端筛选与排序的纯 SQL 构造，值只生成占位符。
use super::table::quote;
use crate::plugins::database::models::{DbColumnInfo, DbType, DbValue, TableOptions};

/// 按方言生成绑定位置；索引从零开始。
pub(crate) fn placeholder(kind: DbType, index: usize) -> String {
    if kind == DbType::Postgresql {
        format!("{}{}", '$', index + 1)
    } else {
        "?".into()
    }
}
/// 校验真实列和操作符，将所有筛选值转为绑定参数。
pub(crate) fn predicates(
    kind: DbType,
    columns: &[DbColumnInfo],
    options: &TableOptions,
) -> Result<(String, Vec<DbValue>), String> {
    if options.filters.len() > 20 || options.sort.len() > 10 {
        return Err("最多 20 条筛选和 10 个排序列".into());
    }
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    for filter in &options.filters {
        if !columns.iter().any(|c| c.name == filter.column) {
            return Err(format!("筛选列不存在：{}", filter.column));
        }
        let column = quote(kind, &filter.column);
        if filter.operator == "isNull" {
            clauses.push(format!("{column} IS NULL"));
            continue;
        }
        if filter.operator == "notNull" {
            clauses.push(format!("{column} IS NOT NULL"));
            continue;
        }
        if matches!(filter.operator.as_str(), "in" | "between") {
            let count = filter.values.len();
            if count == 0 || count > 100 || (filter.operator == "between" && count != 2) {
                return Err("IN 需要 1 至 100 个值，范围需要两个端点".into());
            }
            if filter.values.iter().any(|value| value.kind == "null") {
                return Err("NULL 筛选请使用为空或非空".into());
            }
            let tokens = filter
                .values
                .iter()
                .map(|value| {
                    let token = placeholder(kind, values.len());
                    values.push(value.clone());
                    token
                })
                .collect::<Vec<_>>();
            clauses.push(if filter.operator == "in" {
                format!("{column} IN ({})", tokens.join(", "))
            } else {
                format!("{column} BETWEEN {} AND {}", tokens[0], tokens[1])
            });
            continue;
        }
        let operator = match filter.operator.as_str() {
            "eq" => "=",
            "ne" => "<>",
            "lt" => "<",
            "le" => "<=",
            "gt" => ">",
            "ge" => ">=",
            "like" => "LIKE",
            _ => return Err("筛选操作符无效".into()),
        };
        if filter.value.kind == "null" {
            return Err("NULL 筛选请使用为空或非空".into());
        }
        clauses.push(format!(
            "{column} {operator} {}",
            placeholder(kind, values.len())
        ));
        values.push(filter.value.clone());
    }
    Ok((
        if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        },
        values,
    ))
}
/// 校验排序列并追加主键，返回排序 SQL 与稳定性标记。
pub(crate) fn ordering(
    kind: DbType,
    columns: &[DbColumnInfo],
    options: &TableOptions,
) -> Result<String, String> {
    let mut names: Vec<&String> = Vec::new();
    let mut parts = Vec::new();
    for sort in &options.sort {
        if !columns.iter().any(|c| c.name == sort.column) {
            return Err(format!("排序列不存在：{}", sort.column));
        }
        if names.contains(&&sort.column) {
            continue;
        }
        names.push(&sort.column);
        parts.push(format!(
            "{} {}",
            quote(kind, &sort.column),
            if sort.descending { "DESC" } else { "ASC" }
        ));
    }
    for key in columns.iter().filter(|c| c.key == "PK") {
        if !names.contains(&&key.name) {
            parts.push(quote(kind, &key.name));
        }
    }
    Ok(if parts.is_empty() {
        String::new()
    } else {
        format!(" ORDER BY {}", parts.join(", "))
    })
}
/// PG 二进制协议返回文本投影，避免 NUMERIC/扩展类型的有损中间转换。
pub(crate) fn projection(kind: DbType, columns: &[DbColumnInfo]) -> String {
    columns
        .iter()
        .map(|c| {
            let name = quote(kind, &c.name);
            if kind == DbType::Postgresql {
                format!("CAST({name} AS TEXT) AS {name}")
            } else {
                name
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
/// 依据原生列信息还原 PostgreSQL 文本协议单元格类型。
pub(crate) fn restore_pg_types(
    result: &mut crate::plugins::database::models::QueryResult,
    columns: &[DbColumnInfo],
) {
    for row in &mut result.values {
        for (cell, column) in row.iter_mut().zip(columns) {
            if cell.kind == "null" {
                continue;
            }
            let native = column.data_type.to_lowercase();
            cell.kind = if [
                "smallint", "integer", "bigint", "int2", "int4", "int8", "oid",
            ]
            .contains(&native.as_str())
            {
                "integer"
            } else if native.starts_with("numeric") || native.starts_with("decimal") {
                "decimal"
            } else if native == "real" || native == "double precision" {
                "float"
            } else if native == "boolean" || native == "bool" {
                "boolean"
            } else if native == "bytea" {
                "binary"
            } else if native.starts_with("json") {
                "json"
            } else if native.contains("time") || native == "date" || native == "interval" {
                "temporal"
            } else {
                "text"
            }
            .into();
            if cell.kind == "binary" {
                if let Some(value) = &mut cell.value {
                    if let Some(hex) = value.strip_prefix("\\x") {
                        *value = hex.to_string();
                    }
                }
            } else if cell.kind == "boolean" {
                if let Some(value) = &mut cell.value {
                    *value = (*value == "true" || *value == "t").to_string();
                }
            }
        }
    }
    result.rows = result
        .values
        .iter()
        .map(|row| row.iter().map(DbValue::display).collect())
        .collect();
}
