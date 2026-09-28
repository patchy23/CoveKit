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

/// 默认主键升序的续读边界；列身份只取后端元数据，前端仅提供上批完整末行原值。
/// 自定义排序、无主键、代理驱动或可空主键的 NULL 边界继续使用 OFFSET。
pub(crate) fn keyset_boundary(
    kind: DbType,
    columns: &[DbColumnInfo],
    options: &TableOptions,
    after: Option<&[DbValue]>,
    params: &mut Vec<DbValue>,
) -> Result<Option<String>, String> {
    let Some(after) = after else { return Ok(None) };
    if !options.sort.is_empty()
        || !matches!(
            kind,
            DbType::Mysql | DbType::Polardb | DbType::Postgresql | DbType::Sqlite
        )
    {
        return Ok(None);
    }
    let keys: Vec<_> = columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.key == "PK")
        .collect();
    if keys.is_empty() {
        return Ok(None);
    }
    if after.len() != columns.len() {
        return Err("表结构或续读边界已变化，请刷新表数据".into());
    }
    let mut null_key = false;
    for (index, _) in &keys {
        let value = &after[*index];
        if value.kind == "null" && value.value.is_none() {
            null_key = true;
            continue;
        }
        validate_boundary_value(value)?;
    }
    if null_key {
        return Ok(None);
    }
    // (k1 > ?) OR (k1 = ? AND k2 > ?)；条件最外层括号保留已有筛选的 AND 语义。
    let mut alternatives = Vec::with_capacity(keys.len());
    for end in 0..keys.len() {
        let mut terms = Vec::with_capacity(end + 1);
        for (position, (index, column)) in keys.iter().take(end + 1).enumerate() {
            let token = placeholder(kind, params.len());
            params.push(after[*index].clone());
            terms.push(format!(
                "{} {} {token}",
                quote(kind, &column.name),
                if position == end { ">" } else { "=" }
            ));
        }
        alternatives.push(format!("({})", terms.join(" AND ")));
    }
    Ok(Some(format!("({})", alternatives.join(" OR "))))
}

fn validate_boundary_value(value: &DbValue) -> Result<(), String> {
    let text = value
        .value
        .as_deref()
        .ok_or("主键续读边界缺少原值，请刷新表数据")?;
    if text.len() > 1024 * 1024 {
        return Err("主键续读边界超过 1 MiB，请缩小查询范围".into());
    }
    let valid = match value.kind.as_str() {
        "text" | "temporal" | "json" => true,
        "integer" => {
            let digits = text
                .strip_prefix('-')
                .or_else(|| text.strip_prefix('+'))
                .unwrap_or(text);
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        }
        "decimal" | "float" => {
            text.bytes()
                .all(|byte| byte.is_ascii_digit() || b".+-eE".contains(&byte))
                && text.parse::<f64>().is_ok()
        }
        "boolean" => matches!(text, "true" | "false" | "0" | "1"),
        "binary" => text.len() % 2 == 0 && text.bytes().all(|byte| byte.is_ascii_hexdigit()),
        _ => false,
    };
    if !valid {
        return Err("主键续读边界类型或值无效，请刷新表数据".into());
    }
    Ok(())
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

#[cfg(test)]
mod keyset_tests {
    use super::*;
    use crate::plugins::database::models::{TableFilter, TableSort};

    fn columns() -> Vec<DbColumnInfo> {
        [("tenant", "PK"), ("name", "PK"), ("payload", "—")]
            .into_iter()
            .map(|(name, key)| DbColumnInfo {
                name: name.into(),
                data_type: "text".into(),
                nullable: "否".into(),
                default_value: String::new(),
                key: key.into(),
                comment: String::new(),
            })
            .collect()
    }

    fn boundary() -> Vec<DbValue> {
        vec![
            DbValue::text("integer", "7".into()),
            DbValue::text("text", "a' OR 1=1 --".into()),
            DbValue::text("text", "正文不作为边界".into()),
        ]
    }

    #[test]
    fn composite_boundary_is_bound_and_preserves_filter_parameter_positions() {
        let columns = columns();
        let options = TableOptions {
            filters: vec![TableFilter {
                column: "payload".into(),
                operator: "eq".into(),
                value: DbValue::text("text", "filter".into()),
                values: vec![],
            }],
            sort: vec![],
        };
        let (filter, mut params) = predicates(DbType::Postgresql, &columns, &options).unwrap();
        let sql = keyset_boundary(
            DbType::Postgresql,
            &columns,
            &options,
            Some(&boundary()),
            &mut params,
        )
        .unwrap()
        .unwrap();
        assert_eq!(filter, " WHERE \"payload\" = $1");
        assert_eq!(
            sql,
            "((\"tenant\" > $2) OR (\"tenant\" = $3 AND \"name\" > $4))"
        );
        assert!(!sql.contains("OR 1=1"));
        assert_eq!(
            params
                .iter()
                .map(|value| value.value.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["filter", "7", "7", "a' OR 1=1 --"]
        );
    }

    #[test]
    fn unsupported_order_driver_or_null_key_falls_back_without_parameters() {
        let columns = columns();
        let mut params = vec![];
        let custom = TableOptions {
            filters: vec![],
            sort: vec![TableSort {
                column: "name".into(),
                descending: true,
            }],
        };
        assert!(keyset_boundary(
            DbType::Mysql,
            &columns,
            &custom,
            Some(&boundary()),
            &mut params
        )
        .unwrap()
        .is_none());
        assert!(keyset_boundary(
            DbType::Oracle,
            &columns,
            &TableOptions::default(),
            Some(&boundary()),
            &mut params
        )
        .unwrap()
        .is_none());
        let mut nullable = boundary();
        nullable[0] = DbValue {
            kind: "null".into(),
            value: None,
        };
        assert!(keyset_boundary(
            DbType::Sqlite,
            &columns,
            &TableOptions::default(),
            Some(&nullable),
            &mut params
        )
        .unwrap()
        .is_none());
        let no_keys: Vec<_> = columns
            .into_iter()
            .map(|mut column| {
                column.key.clear();
                column
            })
            .collect();
        assert!(keyset_boundary(
            DbType::Mysql,
            &no_keys,
            &TableOptions::default(),
            Some(&boundary()),
            &mut params
        )
        .unwrap()
        .is_none());
        assert!(params.is_empty());
    }

    #[test]
    fn invalid_boundaries_are_rejected_before_changing_parameters() {
        let columns = columns();
        let options = TableOptions::default();
        let mut params = vec![];
        assert!(keyset_boundary(
            DbType::Mysql,
            &columns,
            &options,
            Some(&boundary()[..2]),
            &mut params
        )
        .is_err());
        for invalid in [
            DbValue::text("integer", "7 OR 1=1".into()),
            DbValue::text("binary", "zz".into()),
            DbValue::text("unexpected", "7".into()),
            DbValue {
                kind: "integer".into(),
                value: None,
            },
        ] {
            let mut row = boundary();
            row[0] = invalid;
            assert!(
                keyset_boundary(DbType::Mysql, &columns, &options, Some(&row), &mut params)
                    .is_err()
            );
            assert!(params.is_empty());
        }
    }

    #[test]
    fn sqlite_composite_key_continues_after_deleted_boundary_without_skipping() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (tenant INTEGER, name TEXT, payload TEXT, PRIMARY KEY(tenant,name)); INSERT INTO t VALUES (1,'a','x'),(1,'b','x'),(2,'a','x'),(2,'b','x'); DELETE FROM t WHERE tenant=1 AND name='a';").unwrap();
        let columns = columns();
        let row = vec![
            DbValue::text("integer", "1".into()),
            DbValue::text("text", "a".into()),
            DbValue::text("text", "x".into()),
        ];
        let mut params = vec![];
        let clause = keyset_boundary(
            DbType::Sqlite,
            &columns,
            &TableOptions::default(),
            Some(&row),
            &mut params,
        )
        .unwrap()
        .unwrap();
        let sql = format!(
            "SELECT tenant,name FROM t WHERE {clause} ORDER BY tenant,name LIMIT 3 OFFSET 0"
        );
        let mut statement = conn.prepare(&sql).unwrap();
        let result = statement
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|value| value.value.as_deref())),
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            result,
            vec![(1, "b".into()), (2, "a".into()), (2, "b".into())]
        );
    }
}
