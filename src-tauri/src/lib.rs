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
    // 启动校验：登记的命令与模块清单逐条对应（少入库 / 清单漏写都在启动期暴露，而不是运行期 404）
    framework::module_manifest::validate_command_declarations();

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
            // ── 维护阶段（必须最早执行）：待执行的存储根迁移 ──
            // 复制与校验在任何业务资源初始化之前完成；校验通过才提交新根配置，
            // 失败则保留源、计划与错误详情并登记可见恢复状态（不阻塞启动，用户能看到提示）。
            let configured_before =
                framework::paths::configured_root(app.handle()).unwrap_or_else(|| {
                    framework::paths::default_root(app.handle())
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                });
            let migration = framework::storage::run_pending(app.handle(), &configured_before);
            eprintln!("[storage] {}", migration.summary());

            // 存储布局迁移必须最先执行：早于任何插件打开数据库、凭证与已知主机文件。
            // 恢复状态（配置盘不可用/迁移失败）下跳过：此时目录不可写，强行迁移只会失败，
            // 而且它属于「必须用户处理的故障」，不该让启动整体失败而看不到恢复提示。
            if framework::storage::recovery::current().is_some() {
                eprintln!("[storage] 存在未处理的存储故障，跳过布局迁移，等待用户在恢复页处理");
            } else {
                // 失败不再让启动整体失败：单项失败会保留原位置，用户需要看到恢复提示而不是
                // 一个打不开的应用。
                match framework::storage::layout::migrate_layout(app.handle()) {
                    Ok(report) if report.has_failures() => {
                        framework::storage::recovery::set(
                            framework::storage::recovery::StorageRecovery::migration_failed(
                                "",
                                &framework::paths::storage_root(app.handle())?
                                    .display()
                                    .to_string(),
                                None,
                                format!(
                                    "旧布局迁移有 {} 项失败（数据保留在原位置）",
                                    report.failures().len()
                                ),
                            ),
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        framework::storage::recovery::set(
                            framework::storage::recovery::StorageRecovery::migration_failed(
                                "",
                                &framework::paths::storage_root(app.handle())?
                                    .display()
                                    .to_string(),
                                None,
                                error,
                            ),
                        );
                    }
                }
            }

            // ── 空间化迁移（uid 化 + 旧扁平布局入位 + 代际目录平铺；一次性升级路径）──
            // 必须在数据上下文固定之前执行：上下文要读到迁移后的空间标识。
            // fail-fast：失败即登记恢复状态（保留现场，重试 = 重启后重跑，迁移幂等）。
            if framework::storage::recovery::current().is_some() {
                eprintln!("[space] 存在未处理的存储故障，跳过空间化迁移");
            } else {
                let root = framework::paths::storage_root(app.handle())?;
                match framework::space::migration::migrate_to_spaces(app.handle(), &root) {
                    Ok(report) => eprintln!("[space] {}", report.summary()),
                    Err(error) => {
                        eprintln!("[space] 空间化迁移失败: {error}");
                        framework::storage::recovery::set(
                            framework::storage::recovery::StorageRecovery::migration_failed(
                                "",
                                &root.display().to_string(),
                                None,
                                format!("空间化迁移失败：{error}"),
                            ),
                        );
                    }
                }
            }

            // ── 框架启动初始化 ──
            // 数据上下文在全部迁移之后固定：存储位置、空间标识与启动 epoch 由它唯一给出，
            // 之后 paths / PluginDb / 插件一律取该实例，不再各自现读配置。
            // 初始化失败（空间自举损坏等）不终止启动：登记恢复状态，数据读写会得到
            // 明确错误，用户能看到恢复页而不是一个打不开的应用。
            if let Err(error) = framework::context::init_from_app(app.handle()) {
                eprintln!("[space] 数据上下文初始化失败: {error}");
                framework::storage::recovery::set(
                    framework::storage::recovery::StorageRecovery::migration_failed(
                        "",
                        &configured_before,
                        None,
                        format!("空间初始化失败：{error}"),
                    ),
                );
            }
            framework::settings::init(app)?;

            // 资源协议（asset://）范围跟随本次生效根：只授权可播放的 cache/tts
            framework::paths::grant_asset_scope(app.handle());

            // 屏蔽 WebView2 原生右键菜单（不再干扰程序内自绘右键菜单）
            // Windows 专属：其他平台没有 WebView2 原生菜单，明确不调用而不是让非 Windows 构建失败
            #[cfg(windows)]
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
                            // 退出不直接 app.exit：与页面/系统发起同一条裁决，业务可拒绝
                            "quit" => framework::exit::quit_from_tray(app),
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
            // prepare 阶段允许业务拒绝（未保存内容、任务进行中）；清理由各模块 dispose 钩子提供。
            // 用户显式强退（app_force_exit）时跳过拦截：拒绝原因仍会写日志，但不再阻止进程退出。
            tauri::RunEvent::ExitRequested { api, .. } => {
                // OS/托盘发起的退出没有同步询问页面的通道，只按后端 blockers 裁决：
                // 页面内「未保存内容」在这条路径上不参与（边界见 AR06 实现方案 §9）。
                // 裁决与托盘菜单退出共用一处（framework::exit::decide_exit），避免两条退出路径语义分叉。
                if !framework::exit::decide_exit(app_handle).proceed {
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
                    "[lifecycle] 关闭完成(原因={}, 模块={}[{}], 超时={}, epoch={}, 丢弃晚到事件={})",
                    reason.code(),
                    outcome.owners.len(),
                    outcome.owners.join(","),
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
