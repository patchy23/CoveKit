//! 插件数据管理 · 统一规则（docs/03-plugin-development.md §3）
//! - 数据文件约定：%APPDATA%/com.patchy23.patchybox/<plugin-id>.db（按平台 app_data_dir）
//! - 迁移规则：PRAGMA user_version 版本号 + 顺序迁移数组（只追加不改写）

use std::path::PathBuf;
use tauri::Manager;

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
        conn.execute_batch(sql)
            .map_err(|e| format!("迁移 v{} 失败: {e}", i + 1))?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))
            .map_err(|e| format!("迁移版本号更新失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 迁移按版本顺序执行且只追加() {
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
