//! HTTP/WS 插件 · 接口库持久化（AR07 §10.1：原 plugins/api 模块归入 http_ws 门面）
//! 数据层走框架统一骨架 PluginDb（framework::store）：路径约定 + 迁移 + 锁内访问，
//! 本模块只写业务 SQL（规则见 docs/standards/03-模块开发规则.md §3）。
//! 命令（api_save/api_list/api_delete/api_clear）与分派 handler 由 http_ws 的静态模块清单声明，
//! 本文件不再自建 handler 与 IPC 登记表。

mod models;
mod transfer;

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
    "ALTER TABLE api_list ADD COLUMN uid TEXT NOT NULL DEFAULT '';
     UPDATE api_list SET uid=lower(hex(randomblob(16))) WHERE uid='';
     CREATE UNIQUE INDEX api_list_uid ON api_list(uid);
     CREATE TRIGGER api_list_assign_uid AFTER INSERT ON api_list WHEN NEW.uid=''
     BEGIN UPDATE api_list SET uid=lower(hex(randomblob(16))) WHERE id=NEW.id; END;",
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
    transfer::register();
    builder.manage(ApiState(std::sync::Mutex::new(None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::store::migrate;
    use std::path::{Path, PathBuf};

    /// 建立本测试专用临时目录（用例之间互不干扰，重复运行时先清掉上次残留）
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("patchybox-api-legacy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 造一个「历史版本」的 api.db：表结构**没有 type 列**（AR07 之前只有 HTTP 接口），
    /// `user_version = 1`，预置两条老数据。返回老数据的 (名称, URL) 供比对。
    fn create_legacy_db(path: &Path) -> Vec<(String, String)> {
        let conn = rusqlite::Connection::open(path).expect("建旧库");
        conn.execute_batch(
            "CREATE TABLE api_list (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                method TEXT NOT NULL,
                url TEXT NOT NULL,
                params TEXT NOT NULL DEFAULT '[]',
                headers TEXT NOT NULL DEFAULT '[]',
                body_mode TEXT NOT NULL DEFAULT 'none',
                body TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            );
            PRAGMA user_version = 1;",
        )
        .expect("旧版建表");
        let legacy = vec![
            (
                "旧接口一".to_string(),
                "https://old.example.com/a".to_string(),
            ),
            (
                "旧接口二".to_string(),
                "https://old.example.com/b".to_string(),
            ),
        ];
        for (name, url) in &legacy {
            conn.execute(
                "INSERT INTO api_list (name, method, url, updated_at)
                 VALUES (?1, 'GET', ?2, '2025-01-01 00:00:00')",
                rusqlite::params![name, url],
            )
            .expect("写旧数据");
        }
        legacy
    }

    /// 读出全部 (名称, 类型, URL)，按 id 升序便于逐条比对
    fn read_all(conn: &rusqlite::Connection) -> Vec<(String, String, String)> {
        let mut stmt = conn
            .prepare("SELECT name, type, url FROM api_list ORDER BY id")
            .expect("准备查询");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("执行查询")
            .collect::<Result<Vec<_>, _>>()
            .expect("收集行")
    }

    /// AR07 ①（旧 schema 升级）：v1 结构（无 type 列）的库经版本迁移后原数据逐条完整，
    /// 且升级后可写、重开后保留（版本号推进到迁移表长度）
    #[test]
    fn old_schema_db_survives_version_migration() {
        let root = temp_dir("inplace");
        let legacy_path = root.join("api.db");
        let expected = create_legacy_db(&legacy_path);

        let mut conn = rusqlite::Connection::open(&legacy_path).expect("打开旧库");
        migrate(&mut conn, MIGRATIONS).expect("版本迁移");
        let rows = read_all(&conn);
        assert_eq!(rows.len(), expected.len(), "老数据条数不变");
        assert_eq!(rows[0].0, expected[0].0, "老数据内容不变");

        // 升级后的表必须可写：新增（含新类型）与更新都要成功
        conn.execute(
            "INSERT INTO api_list (type, name, method, url, updated_at)
             VALUES ('ws', '新接口', 'GET', 'wss://new.example.com', '2026-01-01 00:00:00')",
            [],
        )
        .expect("新增记录");
        conn.execute("UPDATE api_list SET name = '旧接口一改名' WHERE id = 1", [])
            .expect("更新记录");
        drop(conn);

        // 重开同一文件：新增与修改都还在（无丢失）
        let reopened = rusqlite::Connection::open(&legacy_path).expect("重开库");
        let rows = read_all(&reopened);
        assert_eq!(rows.len(), 3, "重开后条数不变");
        assert_eq!(rows[0].0, "旧接口一改名", "重开后修改保留");
        assert_eq!(rows[2].1, "ws", "重开后新增记录的类型保留");
        let version: i64 = reopened
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("读版本号");
        assert_eq!(version, MIGRATIONS.len() as i64, "版本号推进到迁移表长度");
        drop(reopened);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// AR07 ①（幂等）：新库的 v1 建表已含 type 列，v2 的 ADD COLUMN 必然报
    /// duplicate column，迁移必须按幂等成功处理；重复打开不报错、不动数据。
    #[test]
    fn fresh_db_migration_is_idempotent_across_reopen() {
        let root = temp_dir("fresh");
        let path = root.join("api.db");
        {
            let mut conn = rusqlite::Connection::open(&path).expect("建新库");
            migrate(&mut conn, MIGRATIONS).expect("首轮迁移");
            conn.execute(
                "INSERT INTO api_list (type, name, method, url, updated_at)
                 VALUES ('http', '接口', 'GET', 'https://a.example.com', '2026-01-01 00:00:00')",
                [],
            )
            .expect("写入");
        }
        // 第二次打开：版本号已到位，迁移整体跳过；即使重放也要幂等
        let mut conn = rusqlite::Connection::open(&path).expect("重开新库");
        migrate(&mut conn, MIGRATIONS).expect("重复迁移不报错");
        let rows = read_all(&conn);
        assert_eq!(rows.len(), 1, "重复迁移不动数据");
        assert_eq!(rows[0].1, "http");
        drop(conn);

        let _ = std::fs::remove_dir_all(&root);
    }
}
