//! 接口管理插件：HTTP/WebSocket 接口列表持久化（SQLite，rusqlite bundled）
//! 数据文件与迁移走框架约定（framework::store，见 docs/03-plugin-development.md §3）
//! 表：api_list（type/name/method/url/params/headers/body_mode/body/updated_at）

mod models;

use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, State};

pub struct ApiState(pub Mutex<Option<rusqlite::Connection>>);

/// 建表迁移（v1：初始结构；v2：type 列——旧库迁移）
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE api_list (
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
    "ALTER TABLE api_list ADD COLUMN type TEXT NOT NULL DEFAULT 'http';",
];

fn open_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let path = crate::framework::store::plugin_db_path(app, "api")?;
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    crate::framework::store::migrate(&conn, MIGRATIONS)?;
    Ok(conn)
}

fn conn<'a>(
    app: &AppHandle,
    state: &'a State<'_, ApiState>,
) -> Result<MutexGuard<'a, Option<rusqlite::Connection>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(open_conn(app)?);
    }
    Ok(guard)
}

use crate::plugins::api::models::ApiRecord;

/// 保存接口：id 为 0 时新增，否则更新；返回记录 id
#[tauri::command(rename_all = "camelCase")]
#[allow(clippy::too_many_arguments)] // Tauri 命令按字段平铺入参（与前端契约一一对应）
pub fn api_save(
    app: AppHandle,
    state: State<'_, ApiState>,
    id: Option<i64>,
    kind: String,
    name: String,
    method: String,
    url: String,
    params: String,
    headers: String,
    body_mode: String,
    body: String,
) -> Result<i64, String> {
    let mut guard = conn(&app, &state)?;
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
                rusqlite::params![
                    kind, name, method, url, params, headers, body_mode, body, existing
                ],
            )
            .map_err(|e| e.to_string())?;
            existing
        }
    };
    Ok(id)
}

#[tauri::command]
pub fn api_list(app: AppHandle, state: State<'_, ApiState>) -> Result<Vec<ApiRecord>, String> {
    let mut guard = conn(&app, &state)?;
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
pub fn api_delete(app: AppHandle, state: State<'_, ApiState>, id: i64) -> Result<(), String> {
    let mut guard = conn(&app, &state)?;
    let c = guard.as_mut().unwrap();
    c.execute("DELETE FROM api_list WHERE id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn api_clear(app: AppHandle, state: State<'_, ApiState>) -> Result<(), String> {
    let mut guard = conn(&app, &state)?;
    let c = guard.as_mut().unwrap();
    c.execute("DELETE FROM api_list", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 插件注册：命令 + 接口库 State（惰性初始化）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        ("api_save", "保存/更新接口（id=0 新增）"),
        ("api_list", "接口列表"),
        ("api_delete", "删除接口"),
        ("api_clear", "清空全部接口"),
    ])
    .expect("IPC 命令重复注册");
    builder
        .invoke_handler(tauri::generate_handler![
            api_save, api_list, api_delete, api_clear
        ])
        .manage(ApiState(std::sync::Mutex::new(None)))
}
