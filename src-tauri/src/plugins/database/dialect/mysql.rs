//! MySQL 方言（含 PolarDB MySQL 兼容模式）
//! 元数据走 information_schema；分页用 LIMIT/OFFSET；标识符用反引号。

use crate::plugins::database::dialect::DbDialect;

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

    fn version_sql(&self) -> &'static str {
        "SELECT VERSION()"
    }

    // ── 管理操作 ──

    fn create_database_sql(
        &self,
        name: &str,
        charset: Option<&str>,
        collation: Option<&str>,
    ) -> Option<String> {
        // 字符集/排序规则只允许字母数字下划线（防注入；合法值本就满足）
        fn safe(v: &str) -> bool {
            !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        let mut sql = format!("CREATE DATABASE {}", self.quote_ident(name));
        if let Some(cs) = charset.filter(|v| safe(v)) {
            sql.push_str(&format!(" DEFAULT CHARACTER SET {cs}"));
        }
        if let Some(co) = collation.filter(|v| safe(v)) {
            sql.push_str(&format!(" DEFAULT COLLATE {co}"));
        }
        Some(sql)
    }

    fn charsets_sql(&self) -> Option<&'static str> {
        Some("SELECT CHARSET_NAME FROM information_schema.CHARACTER_SETS ORDER BY CHARSET_NAME")
    }

    fn collations_sql(&self) -> Option<&'static str> {
        Some(
            "SELECT CHARACTER_SET_NAME, COLLATION_NAME FROM information_schema.COLLATIONS \
             ORDER BY CHARACTER_SET_NAME, COLLATION_NAME",
        )
    }

    fn users_sql(&self) -> Option<&'static str> {
        Some("SELECT USER, HOST FROM mysql.user ORDER BY USER, HOST")
    }

    fn grant_sql(&self, database: &str, user: &str, host: &str, privilege: &str) -> Option<String> {
        // 权限走白名单（grant 子句无法参数化，必须防注入）
        let priv_sql = match privilege {
            "all" => "ALL PRIVILEGES",
            "readwrite" => "SELECT, INSERT, UPDATE, DELETE",
            "readonly" => "SELECT",
            _ => return None,
        };
        Some(format!(
            "GRANT {priv_sql} ON {}.* TO {}@{}",
            self.quote_ident(database),
            self.quote_literal(user),
            self.quote_literal(host)
        ))
    }

    fn table_ddl_sql(&self, schema: &str, table: &str) -> Option<String> {
        // SHOW CREATE TABLE 不支持参数绑定，用限定名直拼
        Some(format!(
            "SHOW CREATE TABLE {}",
            self.qualified(schema, table)
        ))
    }

    fn indexes_sql(&self, schema: &str, table: &str) -> Option<String> {
        // SHOW INDEX 不支持参数绑定；返回列序 Key_name/Column_name/Non_unique/Index_type
        Some(format!(
            "SELECT Key_name, Column_name, Non_unique, Index_type FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = {} AND TABLE_NAME = {} ORDER BY Key_name, SEQ_IN_INDEX",
            self.quote_literal(schema),
            self.quote_literal(table)
        ))
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
    fn create_database_with_charset_and_collation() {
        let sql = MySqlDialect
            .create_database_sql("shop", Some("utf8mb4"), Some("utf8mb4_unicode_ci"))
            .unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE `shop` DEFAULT CHARACTER SET utf8mb4 DEFAULT COLLATE utf8mb4_unicode_ci"
        );
        // 不传字符集时只建库
        assert_eq!(
            MySqlDialect
                .create_database_sql("shop", None, None)
                .unwrap(),
            "CREATE DATABASE `shop`"
        );
        // 非法字符集值被丢弃（防注入）
        let sql = MySqlDialect
            .create_database_sql("shop", Some("utf8mb4; DROP TABLE x"), None)
            .unwrap();
        assert_eq!(sql, "CREATE DATABASE `shop`");
    }

    #[test]
    fn grant_sql_whitelist() {
        assert_eq!(
            MySqlDialect.grant_sql("shop", "dev", "%", "all").unwrap(),
            "GRANT ALL PRIVILEGES ON `shop`.* TO 'dev'@'%'"
        );
        assert_eq!(
            MySqlDialect
                .grant_sql("shop", "dev", "localhost", "readwrite")
                .unwrap(),
            "GRANT SELECT, INSERT, UPDATE, DELETE ON `shop`.* TO 'dev'@'localhost'"
        );
        assert!(MySqlDialect
            .grant_sql("shop", "dev", "%", "SUPER")
            .is_none());
        // 用户名引号转义
        assert_eq!(
            MySqlDialect
                .grant_sql("shop", "o'k", "%", "readonly")
                .unwrap(),
            "GRANT SELECT ON `shop`.* TO 'o''k'@'%'"
        );
    }

    #[test]
    fn ddl_and_indexes_sql() {
        assert_eq!(
            MySqlDialect.table_ddl_sql("shop", "orders").unwrap(),
            "SHOW CREATE TABLE `shop`.`orders`"
        );
        let idx = MySqlDialect.indexes_sql("shop", "orders").unwrap();
        assert!(idx.contains("information_schema.STATISTICS"));
        assert!(idx.contains("TABLE_SCHEMA = 'shop'"));
        assert!(idx.contains("TABLE_NAME = 'orders'"));
    }

    #[test]
    fn table_admin_sql() {
        assert_eq!(
            MySqlDialect.rename_table_sql("shop", "a", "b").unwrap(),
            "ALTER TABLE `shop`.`a` RENAME TO `b`"
        );
        assert_eq!(
            MySqlDialect.truncate_table_sql("shop", "a").unwrap(),
            "TRUNCATE TABLE `shop`.`a`"
        );
        assert_eq!(
            MySqlDialect.drop_object_sql("shop", "table", "a").unwrap(),
            "DROP TABLE IF EXISTS `shop`.`a`"
        );
        assert_eq!(
            MySqlDialect.drop_database_sql("shop").unwrap(),
            "DROP DATABASE IF EXISTS `shop`"
        );
        assert!(MySqlDialect
            .drop_object_sql("shop", "function", "f")
            .is_none());
    }
}
