//! 数据库方言抽象（驱动与 SQL 语义的分界）
//! 每个方言实现连接 URL/标识符引用/元数据 catalog SQL/分页包装等纯函数，
//! 全部可单测（不依赖真实数据库）。agent 类型（oracle/vastbase/kingbase）
//! 的元数据由 agent 进程提供，不实现本 trait。

use crate::plugins::database::dialect::{
    mysql::MySqlDialect, postgres::PostgresDialect, sqlite::SqliteDialect,
};
use crate::plugins::database::models::DbType;

pub mod mysql;
pub mod postgres;
pub mod sqlite;

/// 方言能力接口：native 数据库（mysql/polardb/postgresql/sqlite）各自实现
pub trait DbDialect: Send + Sync {
    /// 标识符引用（防注入：表名/库名/schema 一律引用后拼接）
    fn quote_ident(&self, name: &str) -> String;

    /// 库列表 SQL（None = 单库类型，由调用方返回默认库名）
    fn databases_sql(&self) -> Option<&'static str>;

    /// schema 列表 SQL（None = 无 schema 层，如 mysql）
    fn schemas_sql(&self) -> Option<&'static str>;

    /// 对象（表/视图/函数/序列等）列表 SQL；返回 (kind, name) 两列
    fn objects_sql(&self) -> &'static str;

    /// 表结构列信息 SQL（按 schema + 表名过滤，参数化用 `?` 或 `$1` 由驱动侧处理）
    /// 返回列顺序：name, data_type, nullable, default_value, comment（key 单独探测）
    fn columns_sql(&self) -> &'static str;

    /// 表行数 SQL（参数：schema、表名）
    fn row_count_sql(&self) -> &'static str;

    /// 版本探测 SQL
    fn version_sql(&self) -> &'static str;

    /// 分页包装：`SELECT ...` → `SELECT ... LIMIT n OFFSET m`
    fn paginate(&self, sql: &str, limit: u64, offset: u64) -> String;

    /// 是否为查询语句（SELECT/WITH/SHOW/EXPLAIN/PRAGMA/DESC 前缀）
    fn is_query_sql(&self, sql: &str) -> bool;

    /// 多语句拆分（分号分隔，跳过引号/注释内的分号；空语句丢弃）
    fn split_statements(&self, sql: &str) -> Vec<String>;

    // ── 管理操作（v2：建库/授权/DDL/索引/表维护；默认不支持，方言按需覆盖）──

    /// 字符串字面量引用（单引号转义，用于无参数绑定的 SHOW/PRAGMA 类语句）
    fn quote_literal(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    /// schema 限定名（schema 为空时只引用对象名）
    fn qualified(&self, schema: &str, name: &str) -> String {
        if schema.is_empty() {
            self.quote_ident(name)
        } else {
            format!("{}.{}", self.quote_ident(schema), self.quote_ident(name))
        }
    }

    /// 建库 SQL（charset/collation 仅 mysql 系用；None = 该类型不支持在线建库）
    fn create_database_sql(
        &self,
        _name: &str,
        _charset: Option<&str>,
        _collation: Option<&str>,
    ) -> Option<String> {
        None
    }

    /// 字符集清单 SQL（返回单列字符集名；None = 不支持）
    fn charsets_sql(&self) -> Option<&'static str> {
        None
    }

    /// 排序规则清单 SQL（返回 (charset, collation) 两列；None = 不支持）
    fn collations_sql(&self) -> Option<&'static str> {
        None
    }

    /// 数据库用户清单 SQL（返回 (user, host) 两列；None = 不支持）
    fn users_sql(&self) -> Option<&'static str> {
        None
    }

    /// 库级授权 SQL（对 'user'@'host' 授予指定权限；None = 不支持）
    fn grant_sql(
        &self,
        _database: &str,
        _user: &str,
        _host: &str,
        _privilege: &str,
    ) -> Option<String> {
        None
    }

    /// 表 DDL 查询 SQL（None = 该类型暂不支持查看 DDL）
    fn table_ddl_sql(&self, _schema: &str, _table: &str) -> Option<String> {
        None
    }

    /// 索引清单 SQL（返回 (name, columns, non_unique, definition) 四列；None = 不支持）
    fn indexes_sql(&self, _schema: &str, _table: &str) -> Option<String> {
        None
    }

    /// 重命名表 SQL（三方言同为 ALTER TABLE ... RENAME TO ...）
    fn rename_table_sql(&self, schema: &str, old: &str, new: &str) -> Option<String> {
        Some(format!(
            "ALTER TABLE {} RENAME TO {}",
            self.qualified(schema, old),
            self.quote_ident(new)
        ))
    }

    /// 清空表 SQL（默认 TRUNCATE；sqlite 无 TRUNCATE 覆盖为 DELETE）
    fn truncate_table_sql(&self, schema: &str, table: &str) -> Option<String> {
        Some(format!("TRUNCATE TABLE {}", self.qualified(schema, table)))
    }

    /// 删除对象 SQL（kind = table/view）
    fn drop_object_sql(&self, schema: &str, kind: &str, name: &str) -> Option<String> {
        let kw = match kind {
            "table" => "TABLE",
            "view" | "materialized_view" => "VIEW",
            _ => return None,
        };
        Some(format!(
            "DROP {kw} IF EXISTS {}",
            self.qualified(schema, name)
        ))
    }

    /// 删除库 SQL（sqlite 无库概念返回 None）
    fn drop_database_sql(&self, name: &str) -> Option<String> {
        Some(format!(
            "DROP DATABASE IF EXISTS {}",
            self.quote_ident(name)
        ))
    }
}

/// 按类型取方言实例（agent/redis 返回 None）
pub fn dialect_for(db_type: DbType) -> Option<Box<dyn DbDialect>> {
    match db_type {
        DbType::Mysql | DbType::Polardb => Some(Box::new(MySqlDialect)),
        DbType::Postgresql => Some(Box::new(PostgresDialect)),
        DbType::Sqlite => Some(Box::new(SqliteDialect)),
        _ => None,
    }
}

/// 通用多语句拆分：按分号切分，跳过单引号/双引号/反引号字符串与行注释/块注释。
/// 返回去掉首尾空白后的语句列表（空语句丢弃）。
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut quote: Option<char> = None;
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some(c) = chars.next() {
        if line_comment {
            // 行注释：跳过（语句内注释对执行无意义；注释仅作分隔/说明）
            if c == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                // 处理转义：'' 或 \'
                if chars.peek() == Some(&q) {
                    current.push(q);
                    chars.next();
                } else {
                    quote = None;
                }
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                current.push(c);
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                line_comment = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                block_comment = true;
            }
            ';' => {
                let stmt = current.trim();
                if !stmt.is_empty() {
                    statements.push(stmt.to_string());
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        statements.push(tail.to_string());
    }
    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_statements_skips_quotes_and_comments() {
        let sql =
            "SELECT ';' AS a; -- 注释;注释\nUPDATE t SET v = 'x;y'; /* 块;注释 */ DELETE FROM t;";
        let parts = split_sql_statements(sql);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "SELECT ';' AS a");
        assert_eq!(parts[1], "UPDATE t SET v = 'x;y'");
        assert_eq!(parts[2], "DELETE FROM t");
    }

    #[test]
    fn empty_statements_and_trailing_semicolons_ignored() {
        let parts = split_sql_statements(";;; SELECT 1 ;;;");
        assert_eq!(parts, vec!["SELECT 1"]);
    }

    #[test]
    fn escaped_quotes_do_not_break_strings() {
        let parts = split_sql_statements("SELECT 'it''s; ok'");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], "SELECT 'it''s; ok'");
    }

    #[test]
    fn dialect_dispatch_covers_native_types_only() {
        assert!(dialect_for(DbType::Mysql).is_some());
        assert!(dialect_for(DbType::Polardb).is_some());
        assert!(dialect_for(DbType::Postgresql).is_some());
        assert!(dialect_for(DbType::Sqlite).is_some());
        assert!(dialect_for(DbType::Redis).is_none());
        assert!(dialect_for(DbType::Oracle).is_none());
        assert!(dialect_for(DbType::Dameng).is_none());
    }
}
