//! 插件数据管理 · 统一规则（docs/03-plugin-development.md §3）
//! - 数据文件约定：%APPDATA%/com.patchy23.patchybox/<plugin-id>.db（按平台 app_data_dir）
//! - 迁移规则：PRAGMA user_version 版本号 + 顺序迁移数组（只追加不改写）
//! - 本地库统一骨架 PluginDb：连接生命周期 + 锁语义 + 迁移，插件只管业务 SQL

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

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
        let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
        migrate(&conn, migrations)?;
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

    /// 便捷执行：无参写入语句（返回受影响行数）
    pub fn execute(&self, sql: &str) -> Result<usize, String> {
        self.with_conn(|c| c.execute(sql, []).map_err(|e| e.to_string()))
    }
}

/// 插件数据文件路径（统一约定，禁止插件手拼路径）
pub fn plugin_db_path(app: &tauri::AppHandle, plugin: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    Ok(dir.join(format!("{plugin}.db")))
}

/// 顺序迁移：按 user_version 依次执行未应用的迁移（只追加，禁止修改已发布迁移）
pub fn migrate(conn: &rusqlite::Connection, migrations: &[&str]) -> Result<(), String> {
    let cur: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    for (i, sql) in migrations.iter().enumerate().skip(cur as usize) {
        if let Err(e) = conn.execute_batch(sql) {
            // ALTER TABLE ADD COLUMN 无 IF NOT EXISTS：进程在版本号写入前中断会导致重放报
            // duplicate column——该错误视为幂等成功（列已存在即等价于迁移完成），其余错误照旧失败
            if e.to_string().contains("duplicate column name") {
                eprintln!("[store] 迁移 v{} 幂等跳过（列已存在）", i + 1);
            } else {
                return Err(format!("迁移 v{} 失败: {e}", i + 1));
            }
        }
        conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))
            .map_err(|e| format!("迁移版本号更新失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_runs_in_version_order_and_only_appends() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn, &["CREATE TABLE t1 (id INTEGER);"]).unwrap();
        // 已应用的不再执行
        migrate(
            &conn,
            &[
                "CREATE TABLE t1 (id INTEGER);",
                "CREATE TABLE t2 (id INTEGER);",
            ],
        )
        .unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
        // 新表确实建了，旧迁移未重复执行（无报错即通过）
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='t2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
