//! patchyBox 桌面工具箱 · Rust 侧框架装配入口
//! 插件模式：业务插件位于 plugins/（命令清单 + State 由 register 自注册，启动初始化走 init）；
//! 新增插件 = plugins/<id>/ 目录 + plugins/mod.rs 路由清单一行，本文件无需改动。
//! 框架级命令（窗口/外链）与本文件同目录，清单见 framework/mod.rs。

mod framework;
mod plugins;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_single_instance::init as single_instance_init;

/// 应用入口：装配框架与全部插件后启动（tauri 主循环）
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
        .invoke_handler(|invoke| {
            if plugins::is_command(invoke.message.command()) {
                plugins::invoke_handler(invoke)
            } else {
                framework::invoke_handler(invoke)
            }
        });

    // ── 框架装配（命令入库与分派 handler 见 framework/mod.rs 的静态清单）──
    let builder = framework::register(builder);

    // ── 业务插件装配：顺序由 plugins/mod.rs 的路由清单决定，新增插件不改动本文件 ──
    let builder = plugins::register_all(builder);

    // 启动校验：清单与登记表 owner 均有路由分支（登记了命令却没有分支 = 启动即炸，不等运行期静默 404）
    plugins::validate_routing();

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
            // ── 框架启动初始化 ──
            // 数据上下文最先固定：存储位置、空间代际与启动 epoch 由它唯一给出，
            // 之后 paths / PluginDb / 插件一律取该实例，不再各自现读配置
            framework::context::init_from_app(app.handle())
                .map_err(|e| format!("数据上下文初始化失败: {e}"))?;
            // 存储布局迁移必须最先执行：早于任何插件打开数据库、凭证与已知主机文件
            framework::paths::migrate_layout(app.handle())
                .map_err(|e| format!("存储布局迁移失败: {e}"))?;
            framework::settings::init(app)?;

            // 屏蔽 WebView2 原生右键菜单（不再干扰程序内自绘右键菜单；仅 Windows 生效）
            disable_native_context_menu(app);

            // 托盘：左键显示主窗；菜单含 显示/退出
            let show_item = MenuItem::with_id(app, "show", "显示 CoveKit", true, None::<&str>)?;
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            // prepare 阶段允许业务拒绝（未保存内容、任务进行中）；清理由各模块 dispose 钩子提供
            tauri::RunEvent::ExitRequested { api, .. } => {
                let outcome = framework::lifecycle::prepare_close(
                    app_handle,
                    framework::lifecycle::CloseReason::Exit,
                );
                if !outcome.proceed {
                    for blocker in &outcome.blockers {
                        eprintln!("[lifecycle] 退出被拒绝: {blocker}");
                    }
                    api.prevent_exit();
                }
            }
            // 真正退出：统一清理（各模块钩子 + 总超时），失败只做诊断，不再阻断退出
            tauri::RunEvent::Exit => {
                let reason = framework::lifecycle::CloseReason::Exit;
                let outcome = framework::lifecycle::dispose(app_handle, reason);
                for failure in &outcome.failures {
                    eprintln!("[lifecycle] 退出清理失败: {failure}");
                }
                eprintln!(
                    "[lifecycle] 关闭完成(原因={}, 模块={}, 超时={}, epoch={}, 丢弃晚到事件={})",
                    reason.code(),
                    outcome.ran,
                    outcome.timed_out,
                    framework::context::current()
                        .map(|c| c.epoch())
                        .unwrap_or(0),
                    framework::context::stale_dropped(),
                );
            }
            _ => {}
        });
}

/// 屏蔽 WebView2 原生右键菜单（复制/粘贴/返回/刷新/检查元素等），避免与程序内自绘右键菜单叠加干扰。
///
/// 为什么用 COM 而不是前端 `contextmenu` + `preventDefault`：前端方案只在主文档生效，
/// iframe 等子文档里的右键事件冒泡不到父文档，菜单照样弹出；而 `AreDefaultContextMenusEnabled=false`
/// 是 host 级总开关（微软文档写明此时连 ContextMenuRequested 事件都不再触发），
/// 主文档、子文档与内建菜单项一次性封禁，故宁可为此付出一次 unsafe（安全性论证见下方 SAFETY 注释）。
///
/// 能力归属：该开关本是 wry 的能力（`WebViewBuilderExtWindows::with_default_context_menus`，
/// 需 WebView2 Runtime ≥ 92.0.902.0），但 Tauri 2.11.5 未透出到 builder 与 tauri.conf.json，
/// 只能经 with_webview 拿到 ICoreWebView2Controller 走 COM；待 Tauri 透出后，本段（连同其 SAFETY 注释）可整体替换。
#[cfg(windows)]
fn disable_native_context_menu(app: &tauri::App) {
    for (_, window) in app.webview_windows() {
        let _ = window.with_webview(|webview| {
            let controller = webview.controller();
            // SAFETY: controller 由 Tauri 的 with_webview 回调提供，生命周期覆盖本次闭包调用；
            // COM 接口（CoreWebView2 / Settings）在同一闭包内即时取用，不跨线程、不跨 await，
            // 不保存任何裸指针；两个调用均用 if let 处理失败分支，不做未经验证的解引用，
            // 因此不存在 UB 路径。
            unsafe {
                if let Ok(core) = controller.CoreWebView2() {
                    if let Ok(settings) = core.Settings() {
                        let _ = settings.SetAreDefaultContextMenusEnabled(false);
                    }
                }
            }
        });
    }
}
