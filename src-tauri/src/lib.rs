// patchyBox 桌面工具箱 · Rust 侧框架装配入口
// 插件模式：业务模块各自 register(builder)（命令 + State 自注册），
// 新增插件 = modules/ 下新模块 + 下方 register 链一行，框架与既有插件零改动。
// 框架级命令（窗口/外链）与启动初始化（剪贴板库/快捷键）留在本文件。

mod framework;
mod modules;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_single_instance::init as single_instance_init;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // 单实例：防多开，二次唤起聚焦主窗
        .plugin(single_instance_init(|app, _args, _cwd| {
            framework::show_main(app);
        }))
        // 全局快捷键：Ctrl+Shift+Space 呼出/隐藏主窗（注册失败降级告警）
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let Some(win) = app.get_webview_window("main") else {
                            return;
                        };
                        if win.is_visible().unwrap_or(false) {
                            let _ = win.hide();
                        } else {
                            framework::show_main(app);
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            framework::window_toggle,
            framework::window_hide,
            framework::open_external,
        ]);

    // ── 业务插件装配（每个插件一行，互不影响）──
    let builder = modules::settings::register(builder);
    let builder = modules::clipboard::register(builder);
    let builder = modules::color::register(builder);
    let builder = modules::http_ws::register(builder);
    let builder = modules::api::register(builder);
    let builder = modules::db::register(builder);
    let builder = modules::hosts::register(builder);

    builder
        // 关窗行为：最小化到托盘（开放问题默认值）
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // 剪贴板历史库（app_data_dir/clipboard.db）
            let db = modules::clipboard::init_db(app)?;
            app.manage(db);
            modules::clipboard::start_watcher(app.handle().clone());

            // 全局快捷键：读取设置 settings.globalHotkey 注册（占用时降级，不阻断启动）
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
            if let Err(e) = modules::settings::register_hotkey(app.handle(), &hotkey) {
                eprintln!("[shortcut] 全局快捷键注册失败（可能被占用）: {e}");
            }

            // 托盘：左键显示主窗；菜单含 显示/退出
            let show_item = MenuItem::with_id(app, "show", "显示 patchyBox", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("缺少默认窗口图标")?;
            let _tray =
                TrayIconBuilder::with_id("main")
                    .icon(icon)
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                        match event.id().as_ref() {
                            "show" => framework::show_main(app),
                            "quit" => app.exit(0),
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(
                        |tray: &tauri::tray::TrayIcon, event: tauri::tray::TrayIconEvent| {
                            if let TrayIconEvent::Click {
                                button: MouseButton::Left,
                                ..
                            } = event
                            {
                                framework::show_main(tray.app_handle());
                            }
                        },
                    )
                    .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
