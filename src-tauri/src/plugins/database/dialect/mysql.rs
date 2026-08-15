//! MySQL 方言（含 PolarDB MySQL 兼容模式）
//! 元数据走 information_schema；分页用 LIMIT/OFFSET；标识符用反引号。

use crate::plugins::database::dialect::{split_sql_statements, DbDialect};

/// MySQL / PolarDB(MySQL 兼容) 方言
pub struct MySqlDialect;

impl DbDialect for MySqlDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("`{}`", name.replace('`', "``"))
    }

    fn databases_sql(&self) -> Option<&'static str> {
        Some("SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME")
    }

    fn schemas_sql(&self) -> Option<&'static str> {
        // MySQL 的 schema 即数据库，树层级上不单独展示 schema 层
        None
    }

    fn objects_sql(&self) -> &'static str {
        // 表（BASE TABLE）与视图（VIEW）一次取回，kind 由 TABLE_TYPE 归一化
        "SELECT TABLE_NAME, \
                CASE TABLE_TYPE WHEN 'VIEW' THEN 'view' ELSE 'table' END \
         FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME"
    }

    fn columns_sql(&self) -> &'static str {
        "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_COMMENT, COLUMN_KEY \
         FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION"
    }

    fn row_count_sql(&self) -> &'static str {
        "SELECT TABLE_ROWS FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?"
    }

    fn version_sql(&self) -> &'static str {
        "SELECT VERSION()"
    }

    fn paginate(&self, sql: &str, limit: u64, offset: u64) -> String {
        let trimmed = sql.trim_end_matches(';').trim_end();
        format!("{trimmed} LIMIT {limit} OFFSET {offset}")
    }

    fn is_query_sql(&self, sql: &str) -> bool {
        let upper = sql.trim_start().to_uppercase();
        [
            "SELECT", "WITH", "SHOW", "EXPLAIN", "DESC", "DESCRIBE", "PRAGMA",
        ]
        .iter()
        .any(|k| upper.starts_with(k))
    }

    fn split_statements(&self, sql: &str) -> Vec<String> {
        split_sql_statements(sql)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backtick_quoted_identifiers() {
        assert_eq!(MySqlDialect.quote_ident("users"), "`users`");
        assert_eq!(MySqlDialect.quote_ident("we`ird"), "`we``ird`");
    }

    #[test]
    fn pagination_appends_limit_and_offset() {
        let sql = MySqlDialect.paginate("SELECT * FROM users ORDER BY id", 20, 40);
        assert_eq!(sql, "SELECT * FROM users ORDER BY id LIMIT 20 OFFSET 40");
    }

    #[test]
    fn trailing_semicolon_cleaned() {
        let sql = MySqlDialect.paginate("SELECT * FROM t;", 10, 0);
        assert_eq!(sql, "SELECT * FROM t LIMIT 10 OFFSET 0");
    }

    #[test]
    fn query_statement_detection() {
        assert!(MySqlDialect.is_query_sql("SELECT 1"));
        assert!(MySqlDialect.is_query_sql("  with t as (select 1) select * from t"));
        assert!(MySqlDialect.is_query_sql("SHOW TABLES"));
        assert!(MySqlDialect.is_query_sql("EXPLAIN SELECT 1"));
        assert!(!MySqlDialect.is_query_sql("UPDATE t SET a = 1"));
        assert!(!MySqlDialect.is_query_sql("INSERT INTO t VALUES (1)"));
        assert!(!MySqlDialect.is_query_sql("DELETE FROM t WHERE a = 'SELECT'"));
    }
}
