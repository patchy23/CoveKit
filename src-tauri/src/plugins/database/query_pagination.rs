//! 单条安全只读查询的服务端分页计划；原始 SQL 保留在内层，分页只作用于其结果。
use crate::plugins::database::{models::DbType, sql_analysis};
use sqlparser::{
    ast::{Expr, OrderByKind, Query, SelectItem, SetExpr, Statement},
    parser::Parser,
};

pub(crate) struct QueryPageSql {
    pub(crate) sql: String,
    pub(crate) hidden_column: Option<String>,
}

/// 为安全重放的单条 SELECT 统计其完整结果；原始 LIMIT/OFFSET 保留在派生表内。
pub(crate) fn build_count_sql(kind: DbType, original_sql: &str) -> Option<String> {
    if !sql_analysis::streamable(kind, original_sql) {
        return None;
    }
    let statements =
        Parser::parse_sql(sql_analysis::parser_dialect(kind).as_ref(), original_sql).ok()?;
    let [Statement::Query(query)] = statements.as_slice() else {
        return None;
    };
    if !safe_wrapper_output(query) {
        return None;
    }
    let inner = without_terminal_semicolon(kind, original_sql)?;
    let alias = if matches!(kind, DbType::Oracle | DbType::Dameng) {
        "covekit_count"
    } else {
        "AS covekit_count"
    };
    Some(format!("SELECT COUNT(*) FROM (\n{inner}\n) {alias}"))
}

/// 为已确认可重放的单条只读 SELECT 生成目标页 SQL；不改写或丢弃用户 LIMIT/OFFSET。
pub(crate) fn build_page_sql(
    kind: DbType,
    original_sql: &str,
    page: u32,
    page_size: u32,
) -> Option<QueryPageSql> {
    if !sql_analysis::streamable(kind, original_sql) || page == 0 || page_size == 0 {
        return None;
    }
    let statements =
        Parser::parse_sql(sql_analysis::parser_dialect(kind).as_ref(), original_sql).ok()?;
    let [Statement::Query(query)] = statements.as_slice() else {
        return None;
    };
    if !safe_wrapper_output(query) {
        return None;
    }
    let offset = u64::from(page - 1).checked_mul(u64::from(page_size))?;
    let limit = u64::from(page_size).checked_add(1)?;
    let order = outer_order(query)?;
    let inner = without_terminal_semicolon(kind, original_sql)?;

    let (sql, hidden_column) = match kind {
        DbType::Oracle | DbType::Dameng => {
            if offset == 0 {
                (
                    format!(
                        "SELECT * FROM (\n{inner}\n) covekit_page WHERE ROWNUM <= {limit}{order}"
                    ),
                    None,
                )
            } else {
                let column = unused_helper_name(original_sql, "__covekit_page_row");
                let end = offset.checked_add(limit)?;
                (
                    format!(
                        "SELECT * FROM (SELECT covekit_page_source.*, ROWNUM AS \"{column}\" FROM (\n{inner}\n) covekit_page_source WHERE ROWNUM <= {end}) WHERE \"{column}\" > {offset}{order}"
                    ),
                    Some(column),
                )
            }
        }
        DbType::Mysql
        | DbType::Polardb
        | DbType::Postgresql
        | DbType::Vastbase
        | DbType::Kingbase
        | DbType::Sqlite => (
            format!(
                "SELECT * FROM (\n{inner}\n) AS covekit_page{order} LIMIT {limit} OFFSET {offset}"
            ),
            None,
        ),
        DbType::Redis => return None,
    };
    Some(QueryPageSql { sql, hidden_column })
}

/// MySQL 等驱动要求派生表输出列名唯一；不明确时沿用原 SQL 游标执行。
fn safe_wrapper_output(query: &Query) -> bool {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return false;
    };
    let has_wildcard = select.projection.iter().any(|item| {
        matches!(
            item,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
        )
    });
    if has_wildcard {
        if select.projection.len() != 1
            || select.from.len() != 1
            || select.from.iter().any(|from| !from.joins.is_empty())
        {
            return false;
        }
    }
    let mut names = Vec::<String>::new();
    for item in &select.projection {
        if matches!(
            item,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
        ) {
            continue;
        }
        let Some(name) = projection_name(item) else {
            return false;
        };
        let name = name.to_lowercase();
        if names.contains(&name) {
            return false;
        }
        names.push(name);
    }
    true
}

/// 派生表分页需要让简单的顶层排序列在外层仍可见；复杂或不明确的排序沿用游标。
fn outer_order(query: &Query) -> Option<String> {
    let Some(order_by) = &query.order_by else {
        return Some(String::new());
    };
    let OrderByKind::Expressions(expressions) = &order_by.kind else {
        return None;
    };
    if expressions.is_empty() {
        return Some(String::new());
    }
    if let SetExpr::Select(select) = query.body.as_ref() {
        if select.from.iter().any(|from| !from.joins.is_empty())
            && select.projection.iter().any(|item| {
                matches!(
                    item,
                    sqlparser::ast::SelectItem::Wildcard(_)
                        | sqlparser::ast::SelectItem::QualifiedWildcard(_, _)
                )
            })
        {
            return None;
        }
        let wildcard = select.projection.iter().any(|item| {
            matches!(
                item,
                sqlparser::ast::SelectItem::Wildcard(_)
                    | sqlparser::ast::SelectItem::QualifiedWildcard(_, _)
            )
        });
        for item in expressions {
            let name = match &item.expr {
                Expr::Identifier(identifier) => Some(identifier.value.as_str()),
                Expr::CompoundIdentifier(parts) => parts.last().map(|part| part.value.as_str()),
                Expr::Value(sqlparser::ast::ValueWithSpan {
                    value: sqlparser::ast::Value::Number(number, _),
                    ..
                }) if number.bytes().all(|byte| byte.is_ascii_digit()) => Some(""),
                _ => None,
            }?;
            if name.is_empty() || wildcard {
                continue;
            }
            let projected = select
                .projection
                .iter()
                .filter(|projection| {
                    projection_name(projection)
                        .is_some_and(|projected| projected.eq_ignore_ascii_case(name))
                })
                .count();
            if projected != 1 {
                return None;
            }
        }
    } else if expressions.iter().any(|item| {
        !matches!(
            item.expr,
            Expr::Value(sqlparser::ast::ValueWithSpan {
                value: sqlparser::ast::Value::Number(ref number, _),
                ..
            }) if number.bytes().all(|byte| byte.is_ascii_digit())
        )
    }) {
        return None;
    }
    let mut rendered = Vec::with_capacity(expressions.len());
    for item in expressions {
        let mut expr = item.expr.clone();
        match &mut expr {
            Expr::CompoundIdentifier(parts) if parts.len() > 1 => {
                let Some(last) = parts.pop() else {
                    return None;
                };
                expr = Expr::Identifier(last);
            }
            Expr::Identifier(_) | Expr::Value(_) => {}
            _ => return None,
        }
        let mut item_sql = expr.to_string();
        if let Some(asc) = item.options.asc {
            item_sql.push_str(if asc { " ASC" } else { " DESC" });
        }
        if let Some(nulls_first) = item.options.nulls_first {
            item_sql.push_str(if nulls_first {
                " NULLS FIRST"
            } else {
                " NULLS LAST"
            });
        }
        rendered.push(item_sql);
    }
    Some(format!(" ORDER BY {}", rendered.join(", ")))
}

fn projection_name(item: &sqlparser::ast::SelectItem) -> Option<String> {
    use sqlparser::ast::SelectItem;
    match item {
        SelectItem::UnnamedExpr(Expr::Identifier(identifier)) => Some(identifier.value.clone()),
        SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts)) => {
            parts.last().map(|part| part.value.clone())
        }
        SelectItem::UnnamedExpr(expr) => Some(expr.to_string()),
        SelectItem::ExprWithAlias { alias, .. } => Some(alias.value.clone()),
        _ => None,
    }
}

/// 原 SQL 已经在用户方言中验证为单查询；此处仅移除终结分号，注释与正文原样保留。
fn without_terminal_semicolon(kind: DbType, sql: &str) -> Option<String> {
    use sqlparser::tokenizer::{Token, Tokenizer};
    let dialect = sql_analysis::parser_dialect(kind);
    let tokens = Tokenizer::new(dialect.as_ref(), sql)
        .tokenize_with_location()
        .ok()?;
    let index = tokens
        .iter()
        .rposition(|token| !matches!(token.token, Token::Whitespace(_)))?;
    let token = tokens.get(index)?;
    if token.token != Token::SemiColon {
        return Some(sql.trim().to_string());
    }
    let start = byte_offset(sql, token.span.start);
    let end = byte_offset(sql, token.span.end);
    Some(
        format!("{}{}", &sql[..start], &sql[end..])
            .trim()
            .to_string(),
    )
}

fn byte_offset(sql: &str, location: sqlparser::tokenizer::Location) -> usize {
    let mut line = 1;
    let mut column = 1;
    for (index, character) in sql.char_indices() {
        if (line, column) == (location.line, location.column) {
            return index;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    sql.len()
}

fn unused_helper_name(sql: &str, base: &str) -> String {
    let mut name = base.to_string();
    let mut suffix = 0_u32;
    while sql.to_lowercase().contains(&name.to_lowercase()) {
        suffix += 1;
        name = format!("{base}_{suffix}");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_original_limit_without_replacing_its_semantics() {
        let plan = build_page_sql(
            DbType::Postgresql,
            "SELECT id FROM items ORDER BY id LIMIT 20 OFFSET 3;",
            2,
            5,
        )
        .expect("可安全分页");
        assert!(plan.sql.contains("LIMIT 20 OFFSET 3"));
        assert!(plan.sql.ends_with("LIMIT 6 OFFSET 5"));
    }

    #[test]
    fn emits_dialect_pagination_and_only_adds_oracle_helper_after_first_page() {
        let oracle = build_page_sql(DbType::Oracle, "SELECT id FROM items ORDER BY id", 2, 5)
            .expect("Oracle 分页");
        assert!(oracle.sql.contains("ROWNUM <= 11"));
        assert!(oracle.sql.contains("> 5"));
        assert!(oracle.hidden_column.is_some());
        let first =
            build_page_sql(DbType::Oracle, "SELECT id FROM items", 1, 5).expect("Oracle 首批");
        assert!(first.sql.contains("ROWNUM <= 6"));
        assert!(first.hidden_column.is_none());
    }

    #[test]
    fn declines_statements_that_must_not_be_replayed_or_have_ambiguous_order() {
        for sql in [
            "SELECT custom_fn(id) FROM items",
            "SELECT * FROM items FOR UPDATE",
            "SELECT id FROM items; DELETE FROM items",
            "SELECT * FROM left_table l JOIN right_table r ON l.id = r.id ORDER BY l.id",
            "SELECT *, id FROM items",
            "SELECT *, * FROM items",
        ] {
            assert!(
                build_page_sql(DbType::Postgresql, sql, 1, 100).is_none(),
                "{sql}"
            );
        }
    }

    #[test]
    fn counts_only_safe_single_selects_and_preserves_user_limits() {
        let sql = build_count_sql(
            DbType::Postgresql,
            "SELECT id FROM items ORDER BY id LIMIT 20 OFFSET 3;",
        )
        .expect("可安全统计");
        assert!(sql.contains("LIMIT 20 OFFSET 3"));
        assert!(sql.ends_with(") AS covekit_count"));
        for sql in [
            "SELECT custom_fn(id) FROM items",
            "SELECT id FROM items; DELETE FROM items",
            "SELECT * FROM items FOR UPDATE",
        ] {
            assert!(build_count_sql(DbType::Postgresql, sql).is_none(), "{sql}");
        }
        let oracle = build_count_sql(DbType::Oracle, "SELECT id FROM items").unwrap();
        assert!(oracle.ends_with(") covekit_count"));
    }
}
