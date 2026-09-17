//! 插件数据管理 · 统一规则（docs/standards/03-模块开发规则.md §3）
//! - 数据文件约定：经 `framework::paths` 解析到数据分区（`<storageRoot>/data/<plugin-id>.db`），
//!   禁止插件手拼路径；`storageRoot` 缺省为平台 app_data_dir
//! - 迁移规则：PRAGMA user_version 版本号 + 顺序迁移数组（只追加不改写），
//!   **每条迁移的 SQL 与版本号更新在同一事务内提交**（见 `migrate`）
//! - 本地库统一骨架 PluginDb：连接生命周期 + 锁语义 + 迁移 + 一致性快照，插件只管业务 SQL

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::framework::paths;

/// 迁移/写入的锁等待上限：多连接（跨进程探针、诊断工具）同时打开同一文件时，
/// 等锁而不是立刻返回 SQLITE_BUSY；超过上限即明确报错，不无限等待。
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// 插件本地数据库（rusqlite 场景的统一骨架）
/// 用途：本地键值/记录型插件（接口列表、设置等）直接使用；
/// 连接型/方言型（MySQL/PG 调试）走 sqlx，不套用本结构。
pub struct PluginDb {
    /// 连接（锁内同步执行；rusqlite 无跨 await 需求，禁止持锁跨 await）
    conn: Mutex<rusqlite::Connection>,
}

impl PluginDb {
    /// 打开插件数据文件并执行迁移（路径统一约定，文件不存在自动创建）
    pub fn open(app: &tauri::AppHandle, plugin: &str, migrations: &[&str]) -> Result<Self, String> {
        let path = plugin_db_path(app, plugin)?;
        Self::open_at(&path, migrations)
    }

    /// 按明确路径打开（插件命令走 `open`；测试与诊断工具用本入口）
    pub fn open_at(path: &Path, migrations: &[&str]) -> Result<Self, String> {
        let mut conn =
            rusqlite::Connection::open(path).map_err(|e| format!("打开数据文件失败: {e}"))?;
        apply_pragmas(&conn)?;
        migrate(&mut conn, migrations)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 锁内访问原始连接（保持 rusqlite 全 API 自由度，业务 SQL 由插件书写）
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self.conn.lock().map_err(|e| e.to_string())?;
        f(&guard)
    }

    /// 锁内事务写入：单事务内执行全部写入，任一失败即整体回滚（合并导入提交用）。
    ///
    /// rusqlite 的 `Transaction` 解引用即 `Connection`，插件的 `&Connection` 层函数直接复用；
    /// `IMMEDIATE` 保证事务期间无其他写者（并发用户写入由 busy_timeout 排队到事务后执行）。
    pub fn with_transaction<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = guard
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| format!("开启事务失败: {e}"))?;
        let value = f(&tx)?;
        tx.commit().map_err(|e| format!("事务提交失败: {e}"))?;
        Ok(value)
    }

    /// 便捷执行：无参写入语句（返回受影响行数）
    pub fn execute(&self, sql: &str) -> Result<usize, String> {
        self.with_conn(|c| c.execute(sql, []).map_err(|e| e.to_string()))
    }
}

/// 插件数据文件路径（统一约定，禁止插件手拼路径）
/// 经 `framework::paths` 解析到数据分区，并对老布局（根下 `<plugin>.db`）自动回落。
pub fn plugin_db_path(app: &tauri::AppHandle, plugin: &str) -> Result<PathBuf, String> {
    paths::data_path(app, &format!("{plugin}.db"))
}

/// 连接级 PRAGMA（集中设置，插件不再各自处理）
///
/// 只设置锁等待上限。**不统一开启 foreign_keys / WAL**：
/// - 既有库的外键行为依赖历史写入顺序，统一开启会让老库出现新的约束失败；
/// - WAL 会新增 `-wal` / `-shm` 伴生文件，改变存储迁移需要复制的文件集合，
///   属于「为现代化付出兼容代价」，需要单独的兼容性评估，不在本次迁移改造内夹带。
fn apply_pragmas(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| format!("设置锁等待上限失败: {e}"))
}

/// 顺序迁移：按 user_version 依次执行未应用的迁移（只追加，禁止修改已发布迁移）
///
/// 契约：
/// 1. 每条迁移的 SQL 与 `user_version` 更新在**同一事务**内提交：SQL 失败或版本号写入失败都整条回滚，
///    不会留下「改了表但版本没推进」或「版本推进了但表只改了一半」的状态。
/// 2. 版本号大于本版本支持的迁移条数（用新版创建、被旧版打开）或为负数时**拒绝写入**并保留文件。
/// 3. 历史半执行库的修复按**版本定向的 schema 校验**进行：只有该迁移声明的列/表/索引全部已存在时，
///    才判定「这条迁移的效果已生效」并推进版本；否则原样报错，不做「遇到 duplicate column 就当整条成功」的兜底。
#[allow(clippy::needless_pass_by_value)]
pub fn migrate(conn: &mut rusqlite::Connection, migrations: &[&str]) -> Result<(), String> {
    apply_pragmas(conn)?;
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| format!("读取数据库版本失败: {e}"))?;
    if current < 0 {
        return Err(format!(
            "数据库版本号非法（user_version = {current}），已保留文件不做任何写入"
        ));
    }
    let supported = migrations.len() as i64;
    if current > supported {
        return Err(format!(
            "数据库由更新版本的应用创建（user_version = {current} > 当前支持的 {supported}），请升级应用后再打开；文件未被修改"
        ));
    }

    for (index, sql) in migrations.iter().enumerate().skip(current as usize) {
        let version = index as i64 + 1;
        let tx = conn
            .transaction()
            .map_err(|e| format!("迁移 v{version} 开启事务失败: {e}"))?;
        if let Err(error) = tx.execute_batch(sql) {
            let message = error.to_string();
            let repairable =
                message.contains("duplicate column name") || message.contains("already exists");
            if repairable && schema_verified_repair(&tx, sql)? {
                eprintln!("[store] 迁移 v{version} 由 schema 校验确认已生效，跳过该版本（历史半执行修复）");
            } else {
                return Err(format!("迁移 v{version} 失败（已回滚）: {error}"));
            }
        }
        tx.pragma_update(None, "user_version", version)
            .map_err(|e| format!("迁移 v{version} 版本号写入失败（已回滚）: {e}"))?;
        tx.commit()
            .map_err(|e| format!("迁移 v{version} 提交失败: {e}"))?;
    }
    Ok(())
}

/// 历史半执行修复判定：该迁移声明的 schema 期望是否**全部**已经存在
///
/// 只做静态提取 + schema 查询（不按分号切 SQL、不执行任何语句）：
/// - 所有 `ALTER TABLE t ADD COLUMN c` 的 (t, c) 都必须已存在；
/// - 所有 `CREATE TABLE/INDEX [IF NOT EXISTS] name` 的 name 都必须已存在；
/// - 一条期望都没提取到（无法判定）时返回 false，交给正常错误路径。
fn schema_verified_repair(tx: &rusqlite::Transaction<'_>, sql: &str) -> Result<bool, String> {
    let columns = extract_add_columns(sql);
    let created = extract_created_names(sql);
    if columns.is_empty() && created.is_empty() {
        return Ok(false);
    }
    for (table, column) in &columns {
        if !column_exists(tx, table, column)? {
            return Ok(false);
        }
    }
    for (kind, name) in &created {
        if !object_exists(tx, kind, name)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 提取 `ALTER TABLE <t> ADD COLUMN <c>` 的表名与列名（大小写不敏感的字面匹配）
fn extract_add_columns(sql: &str) -> Vec<(String, String)> {
    let upper = sql.to_ascii_uppercase();
    let mut found = Vec::new();
    let mut cursor = 0usize;
    while let Some(position) = upper[cursor..].find("ALTER TABLE ") {
        let table_start = cursor + position + "ALTER TABLE ".len();
        let table: String = sql[table_start..]
            .chars()
            .take_while(|c| is_ident_char(*c))
            .collect();
        if let Some(offset) = upper[table_start..].find("ADD COLUMN ") {
            let column_start = table_start + offset + "ADD COLUMN ".len();
            let column: String = sql[column_start..]
                .chars()
                .take_while(|c| is_ident_char(*c))
                .collect();
            if !table.is_empty() && !column.is_empty() {
                found.push((table, column));
            }
        }
        cursor = table_start;
    }
    found
}

/// 提取 `CREATE TABLE/INDEX [IF NOT EXISTS] <name>` 的 (类型, 名字)
///
/// 长前缀优先匹配，命中区域在扫描副本里涂掉，避免 `CREATE TABLE ` 又把
/// `CREATE TABLE IF NOT EXISTS a` 里的 `IF` 当成表名。
fn extract_created_names(sql: &str) -> Vec<(&'static str, String)> {
    let mut masked: Vec<u8> = sql.to_ascii_uppercase().into_bytes();
    let mut found = Vec::new();
    for (prefix, kind) in [
        ("CREATE TABLE IF NOT EXISTS ", "table"),
        ("CREATE UNIQUE INDEX IF NOT EXISTS ", "index"),
        ("CREATE TABLE ", "table"),
        ("CREATE UNIQUE INDEX ", "index"),
        ("CREATE INDEX IF NOT EXISTS ", "index"),
        ("CREATE INDEX ", "index"),
    ] {
        let mut cursor = 0usize;
        loop {
            let haystack = String::from_utf8_lossy(&masked).to_string();
            let Some(position) = haystack[cursor..].find(prefix) else {
                break;
            };
            let name_start = cursor + position + prefix.len();
            let name: String = sql[name_start..]
                .chars()
                .take_while(|c| is_ident_char(*c))
                .collect();
            if !name.is_empty() {
                found.push((kind, name));
            }
            for byte in masked.iter_mut().skip(cursor + position).take(prefix.len()) {
                *byte = b' ';
            }
            cursor = cursor + position + prefix.len();
        }
    }
    found
}

/// 标识符字符（迁移里的表名/列名都是普通标识符；带引号或表达式一律不认，交回正常错误路径）
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// 列是否存在
fn column_exists(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
) -> Result<bool, String> {
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM pragma_table_info(?1) WHERE lower(name) = lower(?2)",
            [table, column],
            |r| r.get(0),
        )
        .map_err(|e| format!("列存在性检查失败（{table}.{column}）: {e}"))?;
    Ok(count > 0)
}

/// 表/索引是否存在
fn object_exists(tx: &rusqlite::Transaction<'_>, kind: &str, name: &str) -> Result<bool, String> {
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = ?1 AND lower(name) = lower(?2)",
            [kind, name],
            |r| r.get(0),
        )
        .map_err(|e| format!("对象存在性检查失败（{kind} {name}）: {e}"))?;
    Ok(count > 0)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 内存库（每个用例独立）
    fn memory() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    /// 读取 user_version
    fn version(conn: &rusqlite::Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    /// 表是否存在
    fn has_table(conn: &rusqlite::Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    /// 往返迁移：只追加、幂等、版本号等于已应用条数
    #[test]
    fn migration_runs_in_version_order_and_only_appends() {
        let mut conn = memory();
        migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap();
        migrate(
            &mut conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "CREATE TABLE t2 (id INTEGER);",
            ],
        )
        .unwrap();
        assert_eq!(version(&conn), 2);
        assert!(has_table(&conn, "t2"));

        // 重复打开同一版本数组：幂等，不再执行
        migrate(
            &mut conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "CREATE TABLE t2 (id INTEGER);",
            ],
        )
        .unwrap();
        assert_eq!(version(&conn), 2);
    }

    /// 同一版本内第二条语句失败：第一条修改必须一起回滚，版本号不推进
    #[test]
    fn failure_inside_version_rolls_back_whole_version() {
        let mut conn = memory();
        migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap();
        let error = migrate(
            &mut conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "CREATE TABLE t2 (id INTEGER);\nTHIS IS NOT SQL;",
            ],
        )
        .unwrap_err();
        assert!(error.contains("迁移 v2 失败"), "{error}");
        assert_eq!(version(&conn), 1, "版本号不得推进");
        assert!(!has_table(&conn, "t2"), "失败版本内的建表必须回滚");
    }

    /// 历史半执行库：该版本声明的列/表都已存在时按 schema 校验跳过并推进版本
    #[test]
    fn half_applied_history_is_repaired_by_schema_check() {
        let mut conn = memory();
        migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap();
        // 模拟旧版本「已执行但版本号没写」：列与表都在，版本号仍是 1
        conn.execute_batch("ALTER TABLE t1 ADD COLUMN name TEXT; CREATE TABLE t2 (id INTEGER);")
            .unwrap();
        migrate(
            &mut conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "ALTER TABLE t1 ADD COLUMN name TEXT;\nCREATE TABLE t2 (id INTEGER);",
            ],
        )
        .unwrap();
        assert_eq!(version(&conn), 2, "确认已生效后必须推进版本");
    }

    /// 历史半执行但后续表缺失：不得当成「整条成功」，必须报错（提示需要修复）
    #[test]
    fn half_applied_with_missing_object_is_rejected() {
        let mut conn = memory();
        migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap();
        conn.execute_batch("ALTER TABLE t1 ADD COLUMN name TEXT;")
            .unwrap();
        let error = migrate(
            &mut conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "ALTER TABLE t1 ADD COLUMN name TEXT;\nCREATE TABLE t3 (id INTEGER);",
            ],
        )
        .unwrap_err();
        assert!(error.contains("迁移 v2 失败"), "{error}");
        assert_eq!(version(&conn), 1);
    }

    /// 更新版本的库（user_version > 支持条数）：拒绝写入并保留文件
    #[test]
    fn newer_version_db_is_refused_without_touching_it() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA user_version = 5; CREATE TABLE future (id INTEGER);")
            .unwrap();
        let error = migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap_err();
        assert!(error.contains("更新版本"), "{error}");
        assert_eq!(version(&conn), 5, "版本号不得被改写");
        assert!(has_table(&conn, "future"));
        assert!(!has_table(&conn, "t1"), "拒绝时不得执行任何迁移");
    }

    /// 非法负版本号：显式拒绝
    #[test]
    fn negative_version_is_refused() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA user_version = -1;").unwrap();
        let error = migrate(&mut conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap_err();
        assert!(error.contains("版本号非法"), "{error}");
        assert_eq!(version(&conn), -1);
    }

    /// 迁移声明的解析：只认普通标识符，带引号/表达式不参与判定
    #[test]
    fn extraction_only_accepts_plain_identifiers() {
        assert_eq!(
            extract_add_columns("ALTER TABLE ssh_profiles ADD COLUMN note TEXT;"),
            vec![("ssh_profiles".to_string(), "note".to_string())]
        );
        assert!(extract_add_columns("SELECT 1").is_empty());
        assert_eq!(
            extract_created_names("CREATE TABLE IF NOT EXISTS a (id INTEGER);"),
            vec![("table", "a".to_string())]
        );
        assert_eq!(
            extract_created_names("CREATE INDEX idx_a ON a (id);"),
            vec![("index", "idx_a".to_string())]
        );
    }
}
