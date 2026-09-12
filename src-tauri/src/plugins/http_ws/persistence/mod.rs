//! HTTP/WS 插件 · 接口库持久化（AR07 §10.1：原 plugins/api 模块归入 http_ws 门面）
//! 数据层走框架统一骨架 PluginDb（framework::store）：路径约定 + 迁移 + 锁内访问，
//! 本模块只写业务 SQL（规则见 docs/03-plugin-development.md §3）。
//! 命令（api_save/api_list/api_delete/api_clear）与分派 handler 由 http_ws 的静态模块清单声明，
//! 本文件不再自建 handler 与 IPC 登记表。

mod models;

use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, State};

use crate::framework::store::PluginDb;
use models::ApiRecord;

/// 接口库状态（惰性初始化：首次命令访问时打开并迁移）
pub struct ApiState(pub Mutex<Option<PluginDb>>);

/// 接口库存储键（历史名）：数据文件固定为 `<storageRoot>/data/api.db`。
/// 模块归属 2026-09-12 由 api 并入 http_ws，但存储键与库内结构保持不变（零数据迁移）。
pub(crate) const STORAGE_KEY: &str = "api";

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

/// 取库实例（锁内借用；首次访问时经 PluginDb::open 惰性打开）
fn db<'a>(
    app: &AppHandle,
    state: &'a State<'_, ApiState>,
) -> Result<MutexGuard<'a, Option<PluginDb>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(PluginDb::open(app, STORAGE_KEY, MIGRATIONS)?);
    }
    Ok(guard)
}

/// 保存接口：id 为 0/None 时新增，否则更新；返回记录 id
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
    let guard = db(&app, &state)?;
    let d = guard.as_ref().ok_or("本地库未初始化")?;
    d.with_conn(|c| {
        let id = match id {
            // 新增：INSERT 后取自增主键
            Some(0) | None => {
                c.execute(
                    "INSERT INTO api_list (type, name, method, url, params, headers, body_mode, body, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now', 'localtime'))",
                    rusqlite::params![kind, name, method, url, params, headers, body_mode, body],
                )
                .map_err(|e| e.to_string())?;
                c.last_insert_rowid()
            }
            // 更新：按 id 全字段覆盖
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
    })
}

/// 接口列表（按更新时间倒序，前端侧栏直接渲染）
#[tauri::command]
pub fn api_list(app: AppHandle, state: State<'_, ApiState>) -> Result<Vec<ApiRecord>, String> {
    let guard = db(&app, &state)?;
    let d = guard.as_ref().ok_or("本地库未初始化")?;
    d.with_conn(|c| {
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
    })
}

/// 删除单个接口
#[tauri::command]
pub fn api_delete(app: AppHandle, state: State<'_, ApiState>, id: i64) -> Result<(), String> {
    let guard = db(&app, &state)?;
    let d = guard.as_ref().ok_or("本地库未初始化")?;
    d.with_conn(|c| {
        c.execute("DELETE FROM api_list WHERE id=?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 清空全部接口
#[tauri::command]
pub fn api_clear(app: AppHandle, state: State<'_, ApiState>) -> Result<(), String> {
    let guard = db(&app, &state)?;
    let d = guard.as_ref().ok_or("本地库未初始化")?;
    d.execute("DELETE FROM api_list").map(|_| ())
}

/// 装配接口库 State（命令登记与分派 handler 由 http_ws 模块清单统一生成）
pub fn register_state(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.manage(ApiState(std::sync::Mutex::new(None)))
}
