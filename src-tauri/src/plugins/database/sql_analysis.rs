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
