//! 框架层：窗口控制命令 + 外链打开（托盘/快捷键/前端共用）
//! 设置存储与全局快捷键（framework/settings.rs）
//! IPC 接口入库（ipc_registry）与插件数据管理（store）
//! 本地凭证管理（credentials）：插件按命名空间+键存取，不关心存储实现
//! Vault 凭证管理（vault）：统一凭证库（keyring 主密钥 + AES-256-GCM + Argon2id 备份）
//! 框架能力不属于业务插件（插件 = 工具，框架 = 基建）。

pub mod credentials;
pub mod ipc_registry;
pub mod settings;
pub mod store;
pub mod vault;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow};

/// 分派全部框架命令；应用级 Builder 只能安装一个 invoke_handler。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        window_toggle,
        window_hide,
        open_external,
        ipc_registry::framework_commands,
        settings::settings_get,
        settings::settings_set,
        vault::vault_list,
        vault::vault_save,
        vault::vault_delete,
        vault::vault_reference_count,
        vault::vault_reveal,
        vault::vault_export,
        vault::vault_import,
    ];
    handler(invoke)
}

/// 窗口可见性状态（window_toggle 的返回）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    /// 切换后窗口是否可见
    visible: bool,
}

/// 获取主窗口引用（托盘/快捷键/命令共用）
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

/// 切换主窗口显示/隐藏（托盘左键与快捷键呼出）
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

/// 隐藏主窗口（最小化到托盘）
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
