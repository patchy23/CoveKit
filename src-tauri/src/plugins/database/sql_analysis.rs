//! 方言 tokenizer 保留原始 SQL 范围；AST 只做风险分析，不改写执行文本。
use crate::plugins::database::models::DbType;
use sqlparser::{
    ast::{Expr, Query, SetExpr, Statement, Visit, Visitor},
    dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect},
    parser::Parser,
    tokenizer::{Location, Token, Tokenizer, Whitespace},
};

/// 与实际驱动匹配的解析方言。
pub(crate) fn parser_dialect(kind: DbType) -> Box<dyn Dialect> {
    match kind {
        DbType::Mysql | DbType::Polardb => Box::new(MySqlDialect {}),
        DbType::Postgresql | DbType::Vastbase | DbType::Kingbase => Box::new(PostgreSqlDialect {}),
        DbType::Sqlite => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

/// 保守识别可能改变会话名称解析的语句，避免临时表结果误写同名持久表。
pub(crate) fn may_change_resolution(kind: DbType, sql: &str) -> bool {
    let dialect = parser_dialect(kind);
    let Ok(tokens) = Tokenizer::new(dialect.as_ref(), sql).tokenize() else {
        return true;
    };
    if tokens.iter().any(|token| matches!(token, Token::Word(word) if ["TEMP", "TEMPORARY", "CALL", "DO", "EXECUTE", "EXEC"].contains(&word.value.to_uppercase().as_str()))) { return true; }
    // 用户函数可创建临时表或改变名称解析；不猜测函数体是否纯读。
    struct Functions;
    impl Visitor for Functions {
        type Break = ();
        fn pre_visit_expr(&mut self, expr: &Expr) -> std::ops::ControlFlow<()> {
            if matches!(expr, Expr::Function(_)) {
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        }
    }
    Parser::parse_sql(dialect.as_ref(), sql).map_or(true, |statements| {
        statements.visit(&mut Functions).is_break()
    })
}

/// 返回无需猜测列来源的持久表候选；会话污染与完整原值由调用方继续核验。
pub(crate) fn edit_target(
    kind: DbType,
    sql: &str,
    conn_id: &str,
    scope: &super::models::ExecutionScope,
    default_database: &str,
) -> Option<super::models::ResultEditTarget> {
    use sqlparser::ast::{
        GroupByExpr, ObjectNamePart, SelectItem, SelectItemQualifiedWildcardKind, TableFactor,
    };
    if !matches!(
        kind,
        DbType::Mysql | DbType::Polardb | DbType::Postgresql | DbType::Sqlite
    ) {
        return None;
    }
    let parsed = Parser::parse_sql(parser_dialect(kind).as_ref(), sql).ok()?;
    let [Statement::Query(query)] = parsed.as_slice() else {
        return None;
    };
    if query.with.is_some() || !query.locks.is_empty() {
        return None;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    if select.distinct.is_some()
        || select.into.is_some()
        || select.having.is_some()
        || select.qualify.is_some()
        || select.exclude.is_some()
        || !select.lateral_views.is_empty()
        || !matches!(&select.group_by, GroupByExpr::Expressions(items, _) if items.is_empty())
        || !select.projection.iter().all(|item| match item {
            SelectItem::Wildcard(options) | SelectItem::QualifiedWildcard(_, options) => {
                *options == Default::default()
            }
            SelectItem::UnnamedExpr(Expr::Identifier(_)) => true,
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts)) => parts.len() == 2,
            _ => false,
        })
    {
        return None;
    }
    let [from] = select.from.as_slice() else {
        return None;
    };
    if !from.joins.is_empty() {
        return None;
    }
    let TableFactor::Table {
        name,
        alias,
        args: None,
        version: None,
        with_ordinality: false,
        ..
    } = &from.relation
    else {
        return None;
    };
    if alias
        .as_ref()
        .is_some_and(|alias| !alias.columns.is_empty())
    {
        return None;
    }
    // 限定前缀必须指向当前表，不能把复合类型字段展开误当成表列。
    let qualifier = alias
        .as_ref()
        .map(|alias| alias.name.to_string())
        .or_else(|| name.0.last().map(ToString::to_string))?;
    if select.projection.iter().any(|item| match item {
        SelectItem::QualifiedWildcard(SelectItemQualifiedWildcardKind::ObjectName(prefix), _) => {
            prefix.to_string() != qualifier
        }
        SelectItem::QualifiedWildcard(_, _) => true,
        SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts)) => {
            parts[0].to_string() != qualifier
        }
        _ => false,
    }) {
        return None;
    }
    let names: Vec<String> = name
        .0
        .iter()
        .map(|part| match part {
            ObjectNamePart::Identifier(id) => {
                Some(if kind == DbType::Postgresql && id.quote_style.is_none() {
                    id.value.to_lowercase()
                } else {
                    id.value.clone()
                })
            }
            _ => None,
        })
        .collect::<Option<_>>()?;
    if names.is_empty() || names.len() > 2 {
        return None;
    }
    // 未显式设置 PG search_path 时默认还可能优先解析同名用户 schema。
    if kind == DbType::Postgresql && names.len() == 1 && scope.schema.is_empty() {
        return None;
    }
    let mut database = if scope.database.is_empty() {
        default_database.to_owned()
    } else {
        scope.database.clone()
    };
    let mut schema = if kind == DbType::Postgresql {
        if scope.schema.is_empty() {
            "public".into()
        } else {
            scope.schema.clone()
        }
    } else {
        database.clone()
    };
    if names.len() == 2 {
        schema = names[0].clone();
        if matches!(kind, DbType::Mysql | DbType::Polardb) {
            database = schema.clone();
        }
        if kind == DbType::Sqlite && schema != "main" {
            return None;
        }
    }
    Some(super::models::ResultEditTarget {
        conn_id: conn_id.into(),
        database,
        schema,
        table: names.last()?.clone(),
    })
}

#[cfg(test)]
mod edit_target_tests {
    use super::*;
    #[test]
    fn preserves_qualified_targets_and_rejects_unknown_search_path() {
        let scope = super::super::models::ExecutionScope {
            database: "app".into(),
            schema: String::new(),
        };
        assert!(edit_target(DbType::Postgresql, "SELECT * FROM users", "c", &scope, "").is_none());
        let pg = edit_target(
            DbType::Postgresql,
            r#"SELECT u.* FROM "Custom"."Users" u"#,
            "c",
            &scope,
            "",
        )
        .unwrap();
        assert_eq!((pg.schema.as_str(), pg.table.as_str()), ("Custom", "Users"));
        let mysql = edit_target(
            DbType::Mysql,
            "SELECT * FROM other_db.users",
            "c",
            &scope,
            "",
        )
        .unwrap();
        assert_eq!(
            (mysql.database.as_str(), mysql.schema.as_str()),
            ("other_db", "other_db")
        );
        assert!(edit_target(DbType::Sqlite, "SELECT * FROM temp.users", "c", &scope, "").is_none());
        assert!(edit_target(
            DbType::Postgresql,
            "SELECT * FROM public.users AS u(name, id)",
            "c",
            &scope,
            ""
        )
        .is_none());
    }
    #[test]
    fn protects_against_session_local_sources() {
        for sql in [
            "CREATE TEMP TABLE users(id INT)",
            "CALL build_temp()",
            "SELECT build_temp()",
            "DO 'BEGIN NULL; END'",
        ] {
            assert!(may_change_resolution(DbType::Postgresql, sql), "{sql}");
        }
        assert!(!may_change_resolution(
            DbType::Postgresql,
            "SELECT * FROM users WHERE name='TEMP' /* CALL */"
        ));
    }
    #[test]
    fn accepts_direct_projection_and_rejects_ambiguous_sources() {
        let scope = super::super::models::ExecutionScope {
            database: "app".into(),
            schema: "public".into(),
        };
        let target = edit_target(
            DbType::Postgresql,
            "SELECT id, name FROM custom.Items WHERE id > 1",
            "c",
            &scope,
            "",
        )
        .unwrap();
        assert_eq!(
            (target.schema.as_str(), target.table.as_str()),
            ("custom", "items")
        );
        for sql in [
            "SELECT a.* FROM a JOIN b ON a.id=b.id",
            "SELECT count(*) FROM a",
            "SELECT DISTINCT * FROM a",
            "SELECT * FROM a GROUP BY id",
            "WITH a AS (SELECT * FROM b) SELECT * FROM a",
            "SELECT * FROM a; SELECT * FROM b",
            "SELECT id AS name FROM a",
            "SELECT record.* FROM a",
            "SELECT record.id FROM a",
            "SELECT * FROM a UNION SELECT * FROM b",
        ] {
            assert!(
                edit_target(DbType::Postgresql, sql, "c", &scope, "").is_none(),
                "{sql}"
            );
        }
    }
}

/// tokenizer 位置按顺序转换为字节偏移，每个字符最多扫描一次。
fn offset(sql: &str, location: Location, cursor: &mut (usize, u64, u64)) -> usize {
    while cursor.0 < sql.len() && (cursor.1, cursor.2) < (location.line, location.column) {
        let Some(ch) = sql.get(cursor.0..).and_then(|rest| rest.chars().next()) else {
            return sql.len();
        };
        cursor.0 += ch.len_utf8();
        if ch == '\n' {
            cursor.1 += 1;
            cursor.2 = 1;
        } else {
            cursor.2 += 1;
        }
    }
    cursor.0
}

/// 分割只返回原文；tokenizer 失败时拒绝，不尝试破坏性简化后继续执行。
pub(crate) fn split(kind: DbType, sql: &str) -> Result<Vec<String>, String> {
    if sql.len() > 1024 * 1024 {
        return Err("SQL 超过 1 MiB，请拆分脚本后执行".into());
    }
    let dialect = parser_dialect(kind);
    let tokens = Tokenizer::new(dialect.as_ref(), sql)
        .tokenize_with_location()
        .map_err(|e| format!("SQL 分析失败: {e}"))?;
    if matches!(kind, DbType::Mysql | DbType::Polardb) && tokens.iter().any(|token| matches!(&token.token, Token::Whitespace(Whitespace::MultiLineComment(text)) if text.starts_with('!') || text.starts_with("M!"))) {
        return Err("请展开 MySQL 可执行版本注释后执行，避免隐藏写入绕过预检".into());
    }
    let words: Vec<String> = tokens
        .iter()
        .filter_map(|t| match &t.token {
            Token::Word(w) => Some(w.value.to_uppercase()),
            _ => None,
        })
        .take(5)
        .collect();
    let block = kind == DbType::Oracle
        && words.first().is_some_and(|w| {
            w == "DECLARE"
                || w == "BEGIN"
                || (w == "CREATE"
                    && words.iter().any(|w| {
                        ["PROCEDURE", "FUNCTION", "TRIGGER", "PACKAGE"].contains(&w.as_str())
                    }))
        });
    if block {
        return Ok(vec![sql.trim().trim_end_matches('/').trim().to_string()]);
    }
    if words
        .first()
        .is_some_and(|w| ["DELIMITER", "SPOOL"].contains(&w.as_str()))
    {
        return Err("暂不执行客户端指令 DELIMITER/SPOOL；请直接执行完整 SQL 块".into());
    }
    // SQLite trigger 内 BEGIN…END 的分号属于触发器体，使用 token 而非字符串扫描。
    let mut trigger = false;
    let mut leading_words = Vec::new();
    let mut cursor = (0, 1, 1);
    let mut depth = 0usize;
    let mut start = 0;
    let mut has_content = false;
    let mut output = Vec::new();
    for token in tokens {
        if matches!(token.token, Token::Whitespace(_)) {
            continue;
        }
        if let Token::Word(word) = &token.token {
            if leading_words.len() < 5 {
                leading_words.push(word.value.to_uppercase());
            }
            trigger |= kind == DbType::Sqlite
                && leading_words.first().is_some_and(|word| word == "CREATE")
                && leading_words.iter().any(|word| word == "TRIGGER");
        }
        if trigger {
            if let Token::Word(w) = &token.token {
                if w.value.eq_ignore_ascii_case("BEGIN") || w.value.eq_ignore_ascii_case("CASE") {
                    depth += 1;
                }
                if w.value.eq_ignore_ascii_case("END") {
                    depth = depth.saturating_sub(1);
                }
            }
        }
        if token.token == Token::SemiColon && depth == 0 {
            let end = offset(sql, token.span.start, &mut cursor);
            if has_content {
                output.push(sql[start..end].trim().to_string());
            }
            start = offset(sql, token.span.end, &mut cursor);
            has_content = false;
            trigger = false;
            leading_words.clear();
        } else {
            has_content = true;
        }
    }
    if has_content {
        output.push(sql[start..].trim().to_string());
    }
    if output.len() > 500 {
        return Err("单次脚本最多 500 条语句，请分批执行".into());
    }
    Ok(output)
}

fn query_readonly(query: &Query) -> bool {
    struct ReadOnlyQueries;
    impl Visitor for ReadOnlyQueries {
        type Break = ();
        fn pre_visit_query(&mut self, query: &Query) -> std::ops::ControlFlow<()> {
            if set_readonly(&query.body) {
                std::ops::ControlFlow::Continue(())
            } else {
                std::ops::ControlFlow::Break(())
            }
        }
    }
    query.visit(&mut ReadOnlyQueries).is_continue()
}
fn set_readonly(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Select(select) => select.into.is_none(),
        SetExpr::Query(query) => set_readonly(&query.body),
        SetExpr::SetOperation { left, right, .. } => set_readonly(left) && set_readonly(right),
        SetExpr::Values(_) | SetExpr::Table(_) => true,
        _ => false,
    }
}

/// 只读分析采用允许列表；未知/厂商专有语法不自动视为安全。
pub(crate) fn readonly(statement: &Statement) -> bool {
    match statement {
        Statement::Query(query) => query_readonly(query),
        Statement::Explain { analyze: false, .. }
        | Statement::ExplainTable { .. }
        | Statement::ShowTables { .. }
        | Statement::ShowColumns { .. }
        | Statement::ShowDatabases { .. }
        | Statement::ShowSchemas { .. }
        | Statement::ShowCreate { .. }
        | Statement::ShowVariables { .. }
        | Statement::ShowStatus { .. }
        | Statement::ShowVariable { .. }
        | Statement::ShowProcessList { .. } => true,
        _ => false,
    }
}

struct ScopeFunction;
impl Visitor for ScopeFunction {
    type Break = ();
    fn pre_visit_expr(&mut self, expr: &Expr) -> std::ops::ControlFlow<()> {
        if let Expr::Function(function) = expr {
            if ["set_config", "pg_catalog.set_config"]
                .iter()
                .any(|name| function.name.to_string().eq_ignore_ascii_case(name))
            {
                return std::ops::ControlFlow::Break(());
            }
        }
        std::ops::ControlFlow::Continue(())
    }
}

/// 返回是否有写入与是否需确认；字符串/注释中的关键词不产生误报。
pub(crate) fn assess(kind: DbType, sql: &str) -> Result<(bool, bool), String> {
    if kind == DbType::Redis {
        let args = super::drivers::redis::split_redis_command(sql)?;
        let first = args
            .first()
            .map(|s| s.to_ascii_uppercase())
            .unwrap_or_default();
        if ["SELECT", "AUTH", "HELLO"].contains(&first.as_str()) {
            return Err("请通过连接设置和数据库选择切换 Redis 目标或认证".into());
        }
        let read = [
            "PING", "GET", "MGET", "TYPE", "TTL", "PTTL", "SCAN", "HSCAN", "SSCAN", "ZSCAN",
            "HGET", "HMGET", "HLEN", "STRLEN", "LLEN", "LRANGE", "SCARD", "ZSCORE", "ZCARD",
            "ZRANGE", "XRANGE", "XLEN", "EXISTS", "DBSIZE", "INFO",
        ]
        .contains(&first.as_str());
        let dangerous = !read
            && ![
                "SET", "SETEX", "PSETEX", "HSET", "LPUSH", "RPUSH", "SADD", "ZADD", "INCR", "DECR",
                "EXPIRE", "PEXPIRE",
            ]
            .contains(&first.as_str());
        return Ok((!read, dangerous));
    }
    let dialect = parser_dialect(kind);
    let mut write = false;
    let mut danger = false;
    for part in split(kind, sql)? {
        match Parser::parse_sql(dialect.as_ref(), &part) {
            Ok(statements) => {
                for statement in statements {
                    let normalized = statement.to_string().to_uppercase();
                    if matches!(statement, Statement::Use(_))
                        || [
                            "SET SEARCH_PATH",
                            "SET SCHEMA",
                            "SET ROLE",
                            "SET SESSION AUTHORIZATION",
                            "SET AUTOCOMMIT",
                            "ALTER SESSION SET CURRENT_SCHEMA",
                        ]
                        .iter()
                        .any(|prefix| normalized.starts_with(prefix))
                        || statement.visit(&mut ScopeFunction).is_break()
                    {
                        return Err("请通过工作页的连接、数据库或 schema 选择器切换目标；不执行会使目标显示失真的会话设置".into());
                    }
                    let transaction = matches!(
                        statement,
                        Statement::StartTransaction { .. }
                            | Statement::Commit { .. }
                            | Statement::Rollback { .. }
                    );
                    let reads = readonly(&statement);
                    write |= !reads && !transaction;
                    danger |= !reads
                        && !transaction
                        && !matches!(
                            statement,
                            Statement::Insert(_)
                                | Statement::Update(sqlparser::ast::Update {
                                    selection: Some(_),
                                    ..
                                })
                                | Statement::Delete(sqlparser::ast::Delete {
                                    selection: Some(_),
                                    ..
                                })
                        );
                }
            }
            Err(_) => {
                write = true;
                danger = true;
            }
        }
    }
    Ok((write, danger))
}

/// 明确事务命令更新页签状态；状态未知时保持事务标记，禁止静默切目标。
pub(crate) fn transaction_after(kind: DbType, sql: &str, mut active: bool) -> bool {
    if let Ok(statements) = Parser::parse_sql(parser_dialect(kind).as_ref(), sql) {
        for statement in statements {
            match statement {
                Statement::StartTransaction { .. } => active = true,
                Statement::Commit { .. }
                | Statement::Rollback {
                    savepoint: None, ..
                } => active = false,
                _ => {
                    if matches!(kind, DbType::Mysql | DbType::Polardb | DbType::Oracle) {
                        let normalized = statement.to_string().to_uppercase();
                        if [
                            "CREATE ",
                            "ALTER ",
                            "DROP ",
                            "TRUNCATE ",
                            "RENAME ",
                            "GRANT ",
                            "REVOKE ",
                        ]
                        .iter()
                        .any(|prefix| normalized.starts_with(prefix))
                            && !normalized.starts_with("CREATE TEMPORARY TABLE")
                            && !normalized.starts_with("DROP TEMPORARY TABLE")
                        {
                            active = false;
                        }
                    }
                }
            }
        }
    }
    active
}
