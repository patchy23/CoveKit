//! 框架层：窗口控制命令 + 外链打开（托盘/快捷键/前端共用）
//! 设置存储与全局快捷键（framework/settings.rs）
//! 统一存储路径（framework/paths.rs）：四分区布局 + 老布局迁移，禁止插件手拼路径
//! 存储位置管理（framework/storage/）：信息查询与迁移（只复制不删源，重启生效）
//! IPC 接口入库（ipc_registry）与插件数据管理（store）
//! 静态模块清单（module_manifest）：命令/入库元数据/route 三处一份声明，禁止手写第二份
//! 数据上下文（context）：存储位置/空间代际/启动 epoch 与维护互斥的唯一来源
//! 关闭协调（lifecycle）：唯一关闭入口（prepare/dispose + 总超时），清理由模块提供
//! 本地凭证管理（credentials）：插件按命名空间+键存取，不关心存储实现
//! Vault 凭证管理（vault）：统一凭证库（keyring 主密钥 + AES-256-GCM + Argon2id 备份）
//! 框架能力不属于业务插件（插件 = 工具，框架 = 基建）。

pub mod context;
pub mod credentials;
pub mod ipc_registry;
pub mod lifecycle;
pub mod module_manifest;
pub mod paths;
pub mod settings;
pub mod storage;
pub mod store;
pub mod vault;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow};

// 框架命令静态清单（AR07）：命令名与 handler 取自同一函数路径标识符，
// 入库元数据（中文说明）与分派 handler 由本清单一次生成。
// owner "framework" 不参与插件路由：应用级 handler 对其余命令直接落到本模块。
//
// 分派入口 `invoke_handler` 由宏生成；应用级 Builder 只能安装一个 invoke_handler。
/// 框架装配：命令入库（清单生成）+ 设置/快捷键等框架 State。
/// 业务插件的装配顺序在 plugins/mod.rs 的路由清单里，框架命令不参与插件路由。
pub(crate) fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    crate::framework::settings::register(builder)
}

crate::patchybox_module! {
    owner: "framework",
    feature: "core",
    commands: {
        window_toggle => "切换主窗口显示/隐藏（返回切换后可见性）",
        window_hide => "隐藏主窗口（最小化到托盘）",
        open_external => "打开外部链接（tauri-plugin-opener，安全替代 shell 插件）",
        ipc_registry::framework_commands => "查询全量已入库 IPC 命令（名称 + 说明）",
        settings::settings_get => "读取应用设置（可指定 key）",
        settings::settings_set => "写入应用设置（launchAtStartup/globalHotkey 有联动副作用）",
        storage::storage_info => "读取存储位置信息（四分区路径与占用）",
        storage::storage_migrate => "迁移存储目录到新根目录（重启生效）",
        vault::vault_list => "凭证列表（脱敏摘要：id/name/kind/掩码/时间，无明文）",
        vault::vault_save => "新增/更新凭证（payload 打包，id 可选 upsert）",
        vault::vault_delete => "删除凭证（返回被引用计数供前端提示）",
        vault::vault_reference_count => "删除前查询后端插件凭证引用数",
        vault::vault_reveal => "读取单条凭证明文（仅用户点显示/复制时调用）",
        vault::vault_export => "密码加密导出 .pbvault 备份（Argon2id 派生密钥）",
        vault::vault_import => "解密导入 .pbvault 备份（合并/覆盖由 UI 选择）",
    },
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

mod manifest_contract_tests;
