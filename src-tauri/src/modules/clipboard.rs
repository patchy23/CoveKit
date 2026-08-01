//! 剪贴板模块：clipboard_list / delete / clear / toggle_pin
//! 轮询服务（500ms + hash 去重）落 SQLite；历史上限清理由 M2 接入 settings.clipboard.historyLimit。

use rusqlite::{params, Connection, Row};
use serde::Serialize;
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::{App, AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

const CREATE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL DEFAULT 'text',
    content TEXT NOT NULL,
    preview TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    content_hash INTEGER NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
"#;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardRecord {
    id: String,
    kind: String,
    content: String,
    preview: String,
    pinned: bool,
    created_at: i64,
}

/// 剪贴板历史数据库（SQLite，存 app_data_dir/clipboard.db）
pub struct ClipboardDb(pub Mutex<Connection>);

pub fn init_db(app: &App) -> Result<ClipboardDb, String> {
    let dir: PathBuf = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let conn = Connection::open(dir.join("clipboard.db")).map_err(|e| e.to_string())?;
    conn.execute_batch(CREATE_SQL).map_err(|e| e.to_string())?;
    Ok(ClipboardDb(Mutex::new(conn)))
}

fn hash_text(s: &str) -> i64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish() as i64
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn row_to_record(row: &Row) -> rusqlite::Result<ClipboardRecord> {
    Ok(ClipboardRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        content: row.get(2)?,
        preview: row.get(3)?,
        pinned: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
    })
}

/// 剪贴板轮询服务：500ms 轮询 + 内容 hash 去重 + 落库（Windows 无系统级变更通知）
pub fn start_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(500));
        // read_text 为同步 API（内部 arboard）
        let Ok(text) = app.clipboard().read_text() else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let hash = hash_text(trimmed);
        let db = app.state::<ClipboardDb>();
        let Ok(conn) = db.0.lock() else {
            continue;
        };
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clipboard_history WHERE content_hash = ?1)",
                [hash],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            continue;
        }
        let preview: String = trimmed.chars().take(80).collect();
        let _ = conn.execute(
            "INSERT INTO clipboard_history (content, preview, content_hash, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![trimmed, preview, hash, now_millis()],
        );
    });
}

#[tauri::command]
pub fn clipboard_list(
    state: State<ClipboardDb>,
    limit: Option<usize>,
    pinned_only: Option<bool>,
) -> Result<Vec<ClipboardRecord>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let limit = (limit.unwrap_or(100)).min(500) as i64;
    let sql = if pinned_only.unwrap_or(false) {
        "SELECT id, kind, content, preview, pinned, created_at FROM clipboard_history
         WHERE pinned = 1 ORDER BY created_at DESC LIMIT ?1"
    } else {
        "SELECT id, kind, content, preview, pinned, created_at FROM clipboard_history
         ORDER BY created_at DESC LIMIT ?1"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], row_to_record)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clipboard_delete(state: State<ClipboardDb>, id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM clipboard_history WHERE id = ?1", [&id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clipboard_clear(state: State<ClipboardDb>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM clipboard_history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clipboard_toggle_pin(state: State<ClipboardDb>, id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE clipboard_history SET pinned = 1 - pinned WHERE id = ?1",
        [&id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
