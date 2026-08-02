//! 历史记录模块：HTTP 请求历史持久化（SQLite，rusqlite bundled）
//! 库文件：%APPDATA%/com.patchy23.patchybox/history.db
//! 表：http_history（method/url/headers/body/status/duration/body_size/created_at）
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tauri::State;

pub struct HistoryState(pub Mutex<Option<rusqlite::Connection>>);

fn db_path() -> PathBuf {
    let dir = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("com.patchy23.patchybox");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("history.db")
}

fn open_conn() -> Result<rusqlite::Connection, String> {
    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS http_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            method TEXT NOT NULL,
            url TEXT NOT NULL,
            headers TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            status INTEGER,
            duration_ms INTEGER,
            body_size INTEGER,
            created_at TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 取连接（惰性初始化）；返回锁内借用，命令在锁内同步执行
fn conn<'a>(
    state: &'a State<'_, HistoryState>,
) -> Result<MutexGuard<'a, Option<rusqlite::Connection>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(open_conn()?);
    }
    Ok(guard)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub headers: String,
    pub body: String,
    pub status: Option<i64>,
    pub duration_ms: Option<i64>,
    pub body_size: Option<i64>,
    pub created_at: String,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri 命令按字段平铺入参（与前端契约一一对应）
pub fn history_add(
    state: State<'_, HistoryState>,
    method: String,
    url: String,
    headers: String,
    body: String,
    status: Option<i64>,
    duration_ms: Option<i64>,
    body_size: Option<i64>,
) -> Result<(), String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    c.execute(
        "INSERT INTO http_history (method, url, headers, body, status, duration_ms, body_size, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now', 'localtime'))",
        rusqlite::params![method, url, headers, body, status, duration_ms, body_size],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn history_list(
    state: State<'_, HistoryState>,
    limit: u32,
) -> Result<Vec<HistoryRecord>, String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    let mut stmt = c
        .prepare(
            "SELECT id, method, url, headers, body, status, duration_ms, body_size, created_at
             FROM http_history ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                method: row.get(1)?,
                url: row.get(2)?,
                headers: row.get(3)?,
                body: row.get(4)?,
                status: row.get(5)?,
                duration_ms: row.get(6)?,
                body_size: row.get(7)?,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn history_clear(state: State<'_, HistoryState>) -> Result<(), String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    c.execute("DELETE FROM http_history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
