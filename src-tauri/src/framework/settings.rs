//! 设置模块：settings_get / settings_set（tauri-plugin-store 持久化）
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。
//! globalHotkey 联动：保存时动态注销旧快捷键并注册新值（被占用降级告警）。

use serde_json::Value;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_store::StoreExt;

/// 当前已注册的全局快捷键（供改键时注销旧值）
pub struct HotkeyState(pub Mutex<Option<Shortcut>>);

/// 注册/切换全局快捷键：注销旧的 → 注册新的 → 更新状态
pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let state = app.state::<HotkeyState>();
    if let Some(old) = state.0.lock().map_err(|e| e.to_string())?.take() {
        let _ = app.global_shortcut().unregister(old);
    }
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|e| format!("快捷键格式无效（{e}），示例：Ctrl+Shift+Space / Alt+Space"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("注册失败（可能被占用）: {e}"))?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(shortcut);
    Ok(())
}

/// 读取设置：key 为空返回整个 app 设置对象（前端与默认值合并），否则返回指定字段
#[tauri::command]
pub fn settings_get(app: AppHandle, key: Option<String>) -> Result<Value, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    match key {
        Some(k) => Ok(store.get(&k).unwrap_or(Value::Null)),
        None => Ok(store.get("app").unwrap_or_else(|| serde_json::json!({}))),
    }
}

/// 更新设置对象单个字段并保存；launchAtStartup 联动系统自启，globalHotkey 动态改键
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

    // 全局快捷键改键：动态切换，失败降级告警（不阻断保存）
    if key == "globalHotkey" {
        if let Some(hotkey) = value.as_str() {
            if let Err(e) = register_hotkey(&app, hotkey) {
                eprintln!("[shortcut] 改键失败: {e}");
            }
        }
    }
    Ok(())
}

/// 框架装配：只注册 State（命令入库与分派 handler 由 framework/mod.rs 的静态清单生成）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.manage(HotkeyState(std::sync::Mutex::new(None)))
}

/// 插件启动初始化：按设置注册全局快捷键（被占用时降级告警，不阻断启动）
pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let hotkey = match tauri_plugin_store::StoreExt::store(app, "settings.json") {
        Ok(s) => s
            .get("app")
            .and_then(|v| {
                v.get("globalHotkey")
                    .and_then(|h| h.as_str().map(String::from))
            })
            .unwrap_or_else(|| "Ctrl+Shift+Space".to_string()),
        Err(_) => "Ctrl+Shift+Space".to_string(),
    };
    if let Err(e) = register_hotkey(app.handle(), &hotkey) {
        eprintln!("[shortcut] 全局快捷键注册失败（可能被占用）: {e}");
    }
    Ok(())
}
