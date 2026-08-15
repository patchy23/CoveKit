//! 数据库插件本地存储（PluginDb 骨架 + 业务 SQL）
//! 数据文件：app_data_dir/database.db；表：connections（连接配置，密码不入表）、
//! history（查询历史）、saved_sql（收藏 SQL）。迁移数组只追加。

use std::sync::Mutex;

use crate::framework::store::PluginDb;
use crate::plugins::database::models::{ConnConfig, DbType, HistoryEntry, SavedEntry};

/// 本地库迁移（v1：三张表；只允许追加新迁移）
const MIGRATIONS: &[&str] = &[
    // v1：连接配置（密码存 stronghold，不在此表）
    "CREATE TABLE IF NOT EXISTS connections (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        db_type TEXT NOT NULL,
        host TEXT NOT NULL DEFAULT '',
        port INTEGER NOT NULL DEFAULT 0,
        username TEXT NOT NULL DEFAULT '',
        database TEXT NOT NULL DEFAULT '',
        env TEXT NOT NULL DEFAULT '开发',
        readonly INTEGER NOT NULL DEFAULT 0,
        ssl INTEGER NOT NULL DEFAULT 0,
        connect_timeout_ms INTEGER NOT NULL DEFAULT 10000,
        sort_order INTEGER NOT NULL DEFAULT 0,
        updated_at TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE IF NOT EXISTS history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conn_id TEXT NOT NULL DEFAULT '',
        sql TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'success',
        duration_ms INTEGER NOT NULL DEFAULT 0,
        at TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE IF NOT EXISTS saved_sql (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        sql TEXT NOT NULL,
        at TEXT NOT NULL DEFAULT ''
    );",
];

/// 本地库 State（惰性打开；锁内同步访问）
pub struct StoreState(pub Mutex<Option<std::sync::Arc<PluginDb>>>);

/// 取（或首次打开）本地库
fn db(app: &tauri::AppHandle, state: &StoreState) -> Result<std::sync::Arc<PluginDb>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(std::sync::Arc::new(PluginDb::open(
            app, "database", MIGRATIONS,
        )?));
    }
    Ok(guard.clone().expect("已初始化"))
}

/// 连接配置 CRUD ─────────────────────────────────────────────────────────
/// 全部连接配置（按 sort_order 排序）
pub fn list_connections(
    app: &tauri::AppHandle,
    state: &StoreState,
) -> Result<Vec<ConnConfig>, String> {
    db(app, state)?.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, label, db_type, host, port, username, database, env, readonly, ssl, connect_timeout_ms \
                 FROM connections ORDER BY sort_order, updated_at",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ConnConfig {
                    id: row.get::<_, String>(0)?,
                    label: row.get::<_, String>(1)?,
                    db_type: DbType::parse(&row.get::<_, String>(2)?)
                        .unwrap_or(DbType::Mysql),
                    host: row.get::<_, String>(3)?,
                    port: row.get::<_, u16>(4)?,
                    username: row.get::<_, String>(5)?,
                    database: row.get::<_, String>(6)?,
                    env: row.get::<_, String>(7)?,
                    readonly: row.get::<_, i64>(8)? != 0,
                    ssl: row.get::<_, i64>(9)? != 0,
                    connect_timeout_ms: row.get::<_, u64>(10)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// 保存连接配置（id 存在则更新）
pub fn save_connection(
    app: &tauri::AppHandle,
    state: &StoreState,
    config: &ConnConfig,
) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute(
            "INSERT INTO connections (id, label, db_type, host, port, username, database, env, readonly, ssl, connect_timeout_ms, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
             ON CONFLICT(id) DO UPDATE SET \
               label = excluded.label, db_type = excluded.db_type, host = excluded.host, \
               port = excluded.port, username = excluded.username, database = excluded.database, \
               env = excluded.env, readonly = excluded.readonly, ssl = excluded.ssl, \
               connect_timeout_ms = excluded.connect_timeout_ms, updated_at = excluded.updated_at",
            rusqlite::params![
                config.id,
                config.label,
                config.db_type.to_string(),
                config.host,
                config.port,
                config.username,
                config.database,
                config.env,
                config.readonly as i64,
                config.ssl as i64,
                config.connect_timeout_ms,
                now_text(),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 删除连接配置（会话由命令层先断开）
pub fn delete_connection(
    app: &tauri::AppHandle,
    state: &StoreState,
    id: &str,
) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute("DELETE FROM connections WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 查询历史 ───────────────────────────────────────────────────────────────
/// 历史列表（时间倒序，上限 100 条）
pub fn list_history(
    app: &tauri::AppHandle,
    state: &StoreState,
) -> Result<Vec<HistoryEntry>, String> {
    db(app, state)?.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, conn_id, sql, status, duration_ms, at FROM history ORDER BY id DESC LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(HistoryEntry {
                    id: row.get::<_, i64>(0)?,
                    conn_id: row.get::<_, String>(1)?,
                    sql: row.get::<_, String>(2)?,
                    status: row.get::<_, String>(3)?,
                    duration_ms: row.get::<_, u64>(4)?,
                    at: row.get::<_, String>(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// 追加一条历史（自动裁剪到 300 条）
pub fn add_history(
    app: &tauri::AppHandle,
    state: &StoreState,
    conn_id: &str,
    sql: &str,
    status: &str,
    duration_ms: u64,
) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute(
            "INSERT INTO history (conn_id, sql, status, duration_ms, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![conn_id, sql, status, duration_ms as i64, now_text()],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT 300)",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 清空历史
pub fn clear_history(app: &tauri::AppHandle, state: &StoreState) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute("DELETE FROM history", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 收藏 SQL ───────────────────────────────────────────────────────────────
/// 收藏列表（时间倒序）
pub fn list_saved(app: &tauri::AppHandle, state: &StoreState) -> Result<Vec<SavedEntry>, String> {
    db(app, state)?.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, title, sql, at FROM saved_sql ORDER BY id DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SavedEntry {
                    id: row.get::<_, i64>(0)?,
                    title: row.get::<_, String>(1)?,
                    sql: row.get::<_, String>(2)?,
                    at: row.get::<_, String>(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    })
}

/// 添加收藏；返回新记录 id（SQL 编辑器首次保存后用于二次保存定位）
pub fn add_saved(
    app: &tauri::AppHandle,
    state: &StoreState,
    title: &str,
    sql: &str,
) -> Result<i64, String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute(
            "INSERT INTO saved_sql (title, sql, at) VALUES (?1, ?2, ?3)",
            rusqlite::params![title, sql, now_text()],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

/// 更新收藏（SQL 编辑器二次保存：仅更新内容与时间，别名以用户重命名为准）
pub fn update_saved(
    app: &tauri::AppHandle,
    state: &StoreState,
    id: i64,
    title: &str,
    sql: &str,
) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        let affected = conn
            .execute(
                "UPDATE saved_sql SET title = ?1, sql = ?2, at = ?3 WHERE id = ?4",
                rusqlite::params![title, sql, now_text(), id],
            )
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err(format!("收藏不存在或已被删除（id={id}）"));
        }
        Ok(())
    })
}

/// 删除收藏
pub fn delete_saved(app: &tauri::AppHandle, state: &StoreState, id: i64) -> Result<(), String> {
    db(app, state)?.with_conn(|conn| {
        conn.execute("DELETE FROM saved_sql WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 当前时间文本（YYYY-MM-DD HH:MM:SS，本地时区）
fn now_text() -> String {
    use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    // Rfc3339 再截取日期时间部分，避免引入额外格式化描述符
    now.format(&Rfc3339)
        .unwrap_or_default()
        .replace('T', " ")
        .chars()
        .take(19)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 时间文本格式为19位() {
        let text = now_text();
        assert_eq!(text.len(), 19);
        assert_eq!(&text[4..5], "-");
        assert_eq!(&text[10..11], " ");
    }
}
