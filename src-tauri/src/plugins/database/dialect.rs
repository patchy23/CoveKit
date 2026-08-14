//! 数据库方言抽象（驱动与 SQL 语义的分界）
//! 每个方言实现连接 URL/标识符引用/元数据 catalog SQL/分页包装等纯函数，
//! 全部可单测（不依赖真实数据库）。agent 类型（oracle/vastbase/kingbase）
//! 的元数据由 agent 进程提供，不实现本 trait。

use crate::plugins::database::models::DbType;
use crate::plugins::database::{
    mysql::MySqlDialect, postgres::PostgresDialect, sqlite::SqliteDialect,
};

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

    /// 执行计划 SQL（explain 前缀；返回带列的结果）
    fn explain_sql(&self, sql: &str) -> String;

    /// 是否为查询语句（SELECT/WITH/SHOW/EXPLAIN/PRAGMA/DESC 前缀）
    fn is_query_sql(&self, sql: &str) -> bool;

    /// 多语句拆分（分号分隔，跳过引号/注释内的分号；空语句丢弃）
    fn split_statements(&self, sql: &str) -> Vec<String>;
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
    fn 拆分多语句并跳过引号与注释() {
        let sql =
            "SELECT ';' AS a; -- 注释;注释\nUPDATE t SET v = 'x;y'; /* 块;注释 */ DELETE FROM t;";
        let parts = split_sql_statements(sql);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "SELECT ';' AS a");
        assert_eq!(parts[1], "UPDATE t SET v = 'x;y'");
        assert_eq!(parts[2], "DELETE FROM t");
    }

    #[test]
    fn 空语句与尾部分号被忽略() {
        let parts = split_sql_statements(";;; SELECT 1 ;;;");
        assert_eq!(parts, vec!["SELECT 1"]);
    }

    #[test]
    fn 转义引号不截断字符串() {
        let parts = split_sql_statements("SELECT 'it''s; ok'");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], "SELECT 'it''s; ok'");
    }

    #[test]
    fn 方言分发只覆盖native类型() {
        assert!(dialect_for(DbType::Mysql).is_some());
        assert!(dialect_for(DbType::Polardb).is_some());
        assert!(dialect_for(DbType::Postgresql).is_some());
        assert!(dialect_for(DbType::Sqlite).is_some());
        assert!(dialect_for(DbType::Redis).is_none());
        assert!(dialect_for(DbType::Oracle).is_none());
        assert!(dialect_for(DbType::Dameng).is_none());
    }
}
