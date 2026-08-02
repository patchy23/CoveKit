//! 接口管理模块：HTTP/WebSocket 接口列表持久化（SQLite，rusqlite bundled）
//! 库文件：%APPDATA%/com.patchy23.patchybox/api.db
//! 表：api_list（type/name/method/url/params/headers/body_mode/body/updated_at）
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tauri::State;

pub struct ApiState(pub Mutex<Option<rusqlite::Connection>>);

fn db_path() -> PathBuf {
    let dir = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("com.patchy23.patchybox");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("api.db")
}

fn open_conn() -> Result<rusqlite::Connection, String> {
    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS api_list (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL DEFAULT 'http',
            name TEXT NOT NULL,
            method TEXT NOT NULL,
            url TEXT NOT NULL,
            params TEXT NOT NULL DEFAULT '[]',
            headers TEXT NOT NULL DEFAULT '[]',
            body_mode TEXT NOT NULL DEFAULT 'none',
            body TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;
    // 旧表（无 type 列）迁移
    let _ = conn.execute_batch(
        "ALTER TABLE api_list ADD COLUMN type TEXT NOT NULL DEFAULT 'http';",
    );
    Ok(conn)
}

fn conn<'a>(
    state: &'a State<'_, ApiState>,
) -> Result<MutexGuard<'a, Option<rusqlite::Connection>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(open_conn()?);
    }
    Ok(guard)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRecord {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub params: String,
    pub headers: String,
    pub body_mode: String,
    pub body: String,
    pub updated_at: String,
}

/// 保存接口：id 为 0 时新增，否则更新；返回记录 id
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri 命令按字段平铺入参（与前端契约一一对应）
pub fn api_save(
    state: State<'_, ApiState>,
    id: Option<i64>,
    #[allow(unused)] kind: String,
    name: String,
    method: String,
    url: String,
    params: String,
    headers: String,
    body_mode: String,
    body: String,
) -> Result<i64, String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    let id = match id {
        Some(0) | None => {
            c.execute(
                "INSERT INTO api_list (type, name, method, url, params, headers, body_mode, body, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now', 'localtime'))",
                rusqlite::params![kind, name, method, url, params, headers, body_mode, body],
            )
            .map_err(|e| e.to_string())?;
            c.last_insert_rowid()
        }
        Some(existing) => {
            c.execute(
                "UPDATE api_list SET type=?1, name=?2, method=?3, url=?4, params=?5, headers=?6,
                 body_mode=?7, body=?8, updated_at=datetime('now', 'localtime') WHERE id=?9",
                rusqlite::params![kind, name, method, url, params, headers, body_mode, body, existing],
            )
            .map_err(|e| e.to_string())?;
            existing
        }
    };
    Ok(id)
}

#[tauri::command]
pub fn api_list(state: State<'_, ApiState>) -> Result<Vec<ApiRecord>, String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    let mut stmt = c
        .prepare(
            "SELECT id, type, name, method, url, params, headers, body_mode, body, updated_at
             FROM api_list ORDER BY updated_at DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ApiRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                method: row.get(3)?,
                url: row.get(4)?,
                params: row.get(5)?,
                headers: row.get(6)?,
                body_mode: row.get(7)?,
                body: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn api_delete(state: State<'_, ApiState>, id: i64) -> Result<(), String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    c.execute("DELETE FROM api_list WHERE id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn api_clear(state: State<'_, ApiState>) -> Result<(), String> {
    let mut guard = conn(&state)?;
    let c = guard.as_mut().unwrap();
    c.execute("DELETE FROM api_list", []).map_err(|e| e.to_string())?;
    Ok(())
}
