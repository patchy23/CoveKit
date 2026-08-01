//! 设置模块：settings_get / settings_set（tauri-plugin-store 持久化）
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

/// 读取设置：key 为空返回整个 app 设置对象（前端与默认值合并），否则返回指定字段
#[tauri::command]
pub fn settings_get(app: AppHandle, key: Option<String>) -> Result<Value, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    match key {
        Some(k) => Ok(store.get(&k).unwrap_or(Value::Null)),
        None => Ok(store.get("app").unwrap_or_else(|| serde_json::json!({}))),
    }
}

/// 更新设置对象单个字段并保存；launchAtStartup 联动系统自启
#[tauri::command]
pub fn settings_set(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut current = store.get("app").unwrap_or_else(|| serde_json::json!({}));
    if let Value::Object(map) = &mut current {
        map.insert(key.clone(), value.clone());
    }
    store.set("app", current);
    store.save().map_err(|e| e.to_string())?;

    // 开机自启联动（autostart 插件；失败仅告警不阻断）
    if key == "launchAtStartup" {
        let result = match value.as_bool() {
            Some(true) => app.autolaunch().enable(),
            Some(false) => app.autolaunch().disable(),
            None => Ok(()),
        };
        if let Err(e) = result {
            eprintln!("[autostart] 设置失败: {e}");
        }
    }
    Ok(())
}
