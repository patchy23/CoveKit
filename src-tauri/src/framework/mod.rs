//! 框架层：窗口控制命令 + 外链打开（托盘/快捷键/前端共用）
//! 设置存储与全局快捷键（framework/settings.rs）
//! IPC 接口入库（ipc_registry）与插件数据管理（store）
//! 框架能力不属于业务插件（插件 = 工具，框架 = 基建）。

pub mod ipc_registry;
pub mod settings;
pub mod store;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    visible: bool,
}

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// 显示并聚焦主窗口（托盘左键 / 二次唤起 / 全局快捷键复用）
pub fn show_main(app: &AppHandle) {
    if let Some(win) = main_window(app) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[tauri::command]
pub fn window_toggle(app: AppHandle) -> Result<WindowState, String> {
    let Some(win) = main_window(&app) else {
        return Err("找不到主窗口".into());
    };
    let visible = win.is_visible().map_err(|e| e.to_string())?;
    if visible {
        win.hide().map_err(|e| e.to_string())?;
    } else {
        show_main(&app);
    }
    Ok(WindowState { visible: !visible })
}

#[tauri::command]
pub fn window_hide(app: AppHandle) -> Result<(), String> {
    let Some(win) = main_window(&app) else {
        return Err("找不到主窗口".into());
    };
    win.hide().map_err(|e| e.to_string())
}

/// 打开外链（tauri-plugin-opener，安全替代 shell 插件）
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}
