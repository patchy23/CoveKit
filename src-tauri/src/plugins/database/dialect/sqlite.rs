//! SQLite 方言
//! 元数据走 sqlite_master + PRAGMA；schema 固定 main；标识符双引号（SQLite 标准引用）。

use crate::plugins::database::dialect::DbDialect;

/// SQLite 方言（单文件库，无库/schema 列表语义）
pub struct SqliteDialect;

impl DbDialect for SqliteDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn databases_sql(&self) -> Option<&'static str> {
        // 单库：固定返回 main
        None
    }

    fn schemas_sql(&self) -> Option<&'static str> {
        None
    }

    fn objects_sql(&self) -> &'static str {
        // sqlite_master 的 type 即对象类型（table/view/index/trigger）
        "SELECT name, type FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name"
    }

    fn columns_sql(&self) -> &'static str {
        // PRAGMA table_info 无参数占位，调用方直接拼表名（已引用转义）
        ""
    }

    fn version_sql(&self) -> &'static str {
        "SELECT sqlite_version()"
    }

    // ── 管理操作（sqlite 差异点）──

    /// sqlite 无 TRUNCATE，用 DELETE FROM 兜底
    fn truncate_table_sql(&self, schema: &str, table: &str) -> Option<String> {
        Some(format!("DELETE FROM {}", self.qualified(schema, table)))
    }

    /// sqlite 是单文件库，无 DROP DATABASE
    fn drop_database_sql(&self, _name: &str) -> Option<String> {
        None
    }

    /// DDL 直接取 sqlite_master.sql（建表/建视图原文）
    fn table_ddl_sql(&self, _schema: &str, table: &str) -> Option<String> {
        Some(format!(
            "SELECT sql FROM sqlite_master WHERE name = {} AND sql IS NOT NULL",
            self.quote_literal(table)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_quote_quoted_identifiers() {
        assert_eq!(SqliteDialect.quote_ident("users"), "\"users\"");
        assert_eq!(SqliteDialect.quote_ident("a\"b"), "\"a\"\"b\"");
    }
}
