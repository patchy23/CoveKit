//! PostgreSQL 方言
//! 元数据走 pg_catalog；schema 层为库内 schema；分页用 LIMIT/OFFSET；标识符双引号。

use crate::plugins::database::dialect::{split_sql_statements, DbDialect};

/// PostgreSQL 方言
pub struct PostgresDialect;

impl DbDialect for PostgresDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn databases_sql(&self) -> Option<&'static str> {
        Some(
            "SELECT datname FROM pg_catalog.pg_database \
             WHERE datistemplate = false ORDER BY datname",
        )
    }

    fn schemas_sql(&self) -> Option<&'static str> {
        Some(
            "SELECT nspname FROM pg_catalog.pg_namespace \
             WHERE nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema' \
             ORDER BY nspname",
        )
    }

    fn objects_sql(&self) -> &'static str {
        // 表（r/p 分区表）与视图（v/m 物化视图）一次取回，kind 由 relkind 归一化
        "SELECT c.relname, \
                CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'materialized_view' ELSE 'table' END \
         FROM pg_catalog.pg_class c \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = ? AND c.relkind IN ('r', 'p', 'v', 'm') \
         ORDER BY c.relname"
    }

    fn columns_sql(&self) -> &'static str {
        // 列 + 默认值 + 注释；键标记由 pg_index 单独探测（见 catalog 层）
        "SELECT a.attname, \
                pg_catalog.format_type(a.atttypid, a.atttypmod), \
                CASE WHEN a.attnotnull THEN '否' ELSE '是' END, \
                COALESCE(pg_catalog.pg_get_expr(ad.adbin, ad.adrelid), ''), \
                COALESCE(col_description(c.oid, a.attnum), '') \
         FROM pg_catalog.pg_attribute a \
         JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         LEFT JOIN pg_catalog.pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum \
         WHERE n.nspname = ? AND c.relname = ? AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY a.attnum"
    }

    fn row_count_sql(&self) -> &'static str {
        // reltuples 为估算值，快速且不锁表；精确值走 COUNT(*)（catalog 层可选）
        "SELECT c.reltuples::bigint FROM pg_catalog.pg_class c \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = ? AND c.relname = ?"
    }

    fn version_sql(&self) -> &'static str {
        "SELECT version()"
    }

    fn paginate(&self, sql: &str, limit: u64, offset: u64) -> String {
        let trimmed = sql.trim_end_matches(';').trim_end();
        format!("{trimmed} LIMIT {limit} OFFSET {offset}")
    }

    fn is_query_sql(&self, sql: &str) -> bool {
        let upper = sql.trim_start().to_uppercase();
        ["SELECT", "WITH", "SHOW", "EXPLAIN", "TABLE", "VALUES"]
            .iter()
            .any(|k| upper.starts_with(k))
    }

    fn split_statements(&self, sql: &str) -> Vec<String> {
        split_sql_statements(sql)
    }

    // ── 管理操作（pg 差异点）──

    /// PG 建库支持 ENCODING（charset 入参映射为 encoding 字面量，白名单字符防注入）
    fn create_database_sql(
        &self,
        name: &str,
        charset: Option<&str>,
        _collation: Option<&str>,
    ) -> Option<String> {
        let mut sql = format!("CREATE DATABASE {}", self.quote_ident(name));
        if let Some(enc) = charset
            .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            sql.push_str(&format!(" ENCODING {}", self.quote_literal(enc)));
        }
        Some(sql)
    }

    /// 索引清单走 pg_indexes（无参数绑定，schema/表名用字面量引用）
    fn indexes_sql(&self, schema: &str, table: &str) -> Option<String> {
        Some(format!(
            "SELECT indexname, '', 0, indexdef FROM pg_indexes \
             WHERE schemaname = {} AND tablename = {} ORDER BY indexname",
            self.quote_literal(schema),
            self.quote_literal(table)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_quote_quoted_identifiers() {
        assert_eq!(PostgresDialect.quote_ident("users"), "\"users\"");
        assert_eq!(PostgresDialect.quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn pagination_appends_limit_and_offset() {
        let sql = PostgresDialect.paginate("SELECT * FROM users", 50, 100);
        assert_eq!(sql, "SELECT * FROM users LIMIT 50 OFFSET 100");
    }

    #[test]
    fn query_statement_detection() {
        assert!(PostgresDialect.is_query_sql("SELECT 1"));
        assert!(PostgresDialect.is_query_sql("WITH x AS (SELECT 1) SELECT * FROM x"));
        assert!(!PostgresDialect.is_query_sql("UPDATE t SET a = 1"));
        assert!(!PostgresDialect.is_query_sql("BEGIN"));
    }

    #[test]
    fn object_sql_has_schema_placeholder() {
        let sql = PostgresDialect.objects_sql();
        assert!(sql.contains("n.nspname = ?"));
    }
}
