//! FRP 配置目录与工具元数据。备注只写 frp.db，不写入用户的 frpc.toml。

use super::now_ms;
use super::MIGRATIONS;
use super::TOOL_ID;
use crate::framework::store::PluginDb;
use crate::plugins::frp::models::FrpOpResult;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::AppHandle;

/// 工具设置读取（settings.json 的 `app.tools.frp.<key>`）
fn tool_setting(app: &AppHandle, key: &str) -> Option<String> {
    // 经框架读路径（合并设备层与空间层）：插件不得直读设置文件
    crate::framework::settings::tool_setting(app, TOOL_ID, key)
}

/// 配置目录：工具设置 `profileDir` 优先，否则 `<存储根>/data/frp/profiles`
pub(crate) fn profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let configured = tool_setting(app, "profileDir").unwrap_or_default();
    if !configured.trim().is_empty() {
        return Ok(PathBuf::from(configured.trim()));
    }
    let dir = crate::framework::paths::data_dir(app)?
        .join(TOOL_ID)
        .join("profiles");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建配置目录失败（{}）：{e}", dir.display()))?;
    Ok(dir)
}

/// 读取全部备注（无库或无记录时返回空表，不影响列表展示）
pub(super) fn read_remarks(app: &AppHandle) -> HashMap<String, String> {
    let Ok(db) = PluginDb::open(app, TOOL_ID, MIGRATIONS) else {
        return HashMap::new();
    };
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT file_name, remark FROM profile_meta")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        for row in rows.flatten() {
            map.insert(row.0, row.1);
        }
        Ok(map)
    })
    .unwrap_or_default()
}

/// 写入备注（upsert；空串即清除备注内容）
pub(super) fn write_remark(app: &AppHandle, file_name: &str, remark: &str) -> Result<(), String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO profile_meta (file_name, remark, last_used_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(file_name) DO UPDATE SET remark = excluded.remark, last_used_at = excluded.last_used_at",
            rusqlite::params![file_name, remark, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 由落盘路径组装操作结果（各写操作共用）
pub(super) fn op_result(path: &std::path::Path) -> FrpOpResult {
    FrpOpResult {
        ok: true,
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from),
        error: None,
    }
}

/// 记录「最近使用」时间（启动时调用；失败只记日志，不打断启动流程）
pub(super) fn touch_used(app: &AppHandle, file_name: &str) {
    if let Ok(db) = PluginDb::open(app, TOOL_ID, MIGRATIONS) {
        let _ = db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO profile_meta (file_name, remark, last_used_at) VALUES (?1, '', ?2)
                 ON CONFLICT(file_name) DO UPDATE SET last_used_at = excluded.last_used_at",
                rusqlite::params![file_name, now_ms()],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        });
    }
}
