//! 框架层：窗口控制命令 + 外链打开（托盘/前端共用）
//! 设置存储（framework/settings.rs）
//! 统一存储路径（framework/paths.rs）：四分区布局 + 老布局迁移，禁止插件手拼路径
//! 存储位置管理（framework/storage/）：信息查询与迁移（只复制不删源，重启生效）
//! IPC 接口入库（ipc_registry）与插件数据管理（store）
//! 静态模块清单（module_manifest）：命令/入库元数据/route 三处一份声明，禁止手写第二份
//! 数据上下文（context）：存储位置/空间代际/启动 epoch 与维护互斥的唯一来源
//! 关闭协调（lifecycle）：唯一关闭入口（prepare/dispose + 总超时），清理由模块提供
//! 长任务登记（tasks）：框架长任务的状态与进度，前端订阅 `framework://task` 获知变化
//! 关闭协商命令（exit）：页签关闭与退出共用同一入口（先裁决后提交），业务拒绝时可见并可由用户强退
//! 本地凭证管理（credentials）：插件按命名空间+键存取，不关心存储实现
//! 本地数据导出导入（data_transfer）：`.pbdata` 包容器与清单 DTO（选定导出 / 隔离导入）
//! Vault 凭证管理（vault）：统一凭证库（keyring 主密钥 + AES-256-GCM + Argon2id 备份）
//! 框架能力不属于业务插件（插件 = 工具，框架 = 基建）。

pub(crate) mod brand_compat;
pub mod context;
pub mod credential_refs;
pub mod credentials;
pub mod data_transfer;
pub mod exit;
pub mod ipc_registry;
pub mod lifecycle;
pub mod module_manifest;
pub mod paths;
pub mod preferences;
pub mod secure_store;
pub mod settings;
pub mod space;
pub mod storage;
pub mod store;
pub mod tasks;
pub mod updater;
pub mod vault;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow};

// 框架命令静态清单（AR07）：命令名与 handler 取自同一函数路径标识符，
// 入库元数据（中文说明）与分派 handler 由本清单一次生成。
// owner "framework" 不参与插件路由：应用级 handler 对其余命令直接落到本模块。
//
// 分派入口 `invoke_handler` 由宏生成；应用级 Builder 只能安装一个 invoke_handler。
/// 框架装配：命令入库（清单生成）。
/// 业务插件的装配顺序在 plugins/mod.rs 的路由清单里，框架命令不参与插件路由。
pub(crate) fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    // 框架自有数据集适配登记（凭证 / 收藏 / 最近使用）：导入导出目录要先知道框架自己有什么
    data_transfer::datasets::register_all();
    register_ipc_or_fail();
    crate::framework::settings::register(builder)
}

crate::covekit_module! {
    owner: "framework",
    feature: "core",
    commands: {
        window_toggle => "切换主窗口显示/隐藏（返回切换后可见性）",
        window_hide => "隐藏主窗口（最小化到托盘）",
        exit::app_request_close => "请求关闭（页签或退出）：只裁决，业务可拒绝，原因交回前端展示",
        exit::app_commit_close => "提交关闭：裁决通过后执行页签级清理或发起退出",
        tasks::framework_tasks => "查询框架长任务清单（活跃与最近结束，供界面与诊断使用）",
        exit::app_force_exit => "用户强制退出（跳过业务拦截，清理仍受总超时约束）",
        open_external => "打开外部链接（tauri-plugin-opener，安全替代 shell 插件）",
        ipc_registry::framework_commands => "查询全量已入库 IPC 命令（名称 + 说明）",
        settings::settings_get => "读取应用设置（可指定 key）",
        settings::settings_set => "写入应用设置（launchAtStartup 有联动副作用）",
        settings::settings_patch => "批量写入应用设置（带版本号校验，拒绝陈旧覆盖）",
        settings::settings_set_tool => "按工具与键写入工具级设置",
        settings::settings_revision => "读取设置版本号（保存时回传防覆盖）",
        settings::preferences_get => "读取空间级用户数据（收藏、最近使用）",
        settings::preferences_set => "写入空间级用户数据（收藏、最近使用）",
    updater::update_availability => "读取更新可用性（占位公钥等无效配置按不可用上报）",
        storage::storage_info => "读取存储位置信息（四分区路径与占用、待执行计划、恢复状态）",
        storage::storage_schedule_migration => "安排存储目录迁移（只登记计划，重启后复制并校验）",
        storage::storage_cancel_migration => "取消待执行的存储目录迁移计划（不修改业务文件）",
        storage::storage_recovery_status => "读取存储恢复状态（配置盘不可用或迁移失败）",
        storage::storage_recovery_action => "执行存储恢复动作（retry/use-default/choose）",
        vault::vault_list => "凭证列表（脱敏摘要：id/name/kind/掩码/时间，无明文）",
        vault::vault_save => "新增/更新凭证（payload 打包，id 可选 upsert）",
        vault::vault_delete => "删除凭证（返回被引用计数供前端提示）",
        vault::vault_credential_references => "查询凭证引用概况（按插件自报能力批量扫描）",
        vault::vault_reveal => "读取单条凭证明文（仅用户点显示/复制时调用）",
        vault::vault_protection_status => "凭证保护状态（主密钥实际来源与可用性，设置页展示）",
        vault::vault_export => "密码加密导出 .pbvault 备份（Argon2id 派生密钥）",
        vault::vault_import => "解密导入 .pbvault 备份（合并/覆盖由 UI 选择）",
        data_transfer::commands::spaces::data_spaces_list => "列出本机空间（名称、来源留档、当前活动标记）",
        data_transfer::commands::spaces::data_space_switch => "切换活动空间（写设备级指针，重启后生效）",
        data_transfer::commands::export::data_export_catalog => "当前空间可导出集合摘要与依赖关系",
        data_transfer::commands::export::data_export_start => "生成数据包 .pbdata（密码加密，写入用户选定路径）",
        data_transfer::commands::import::data_import_inspect => "校验数据包并返回预览（新增/待补全/被排除/重复包）",
        data_transfer::commands::import::data_import_plan => "规划导入：确定目标空间与要写入的记录（不写业务数据）",
        data_transfer::commands::import::data_import_commit => "提交导入：建新空间落数据或合并/覆盖进当前空间（失败回滚）",
        data_transfer::commands::spaces::data_backup_list => "列出当前空间的导入前快照（设置页还原入口）",
        data_transfer::commands::spaces::data_backup_restore => "还原到导入前：快照写回当前空间并广播刷新",
        data_transfer::commands::data_transfer_cancel => "取消正在进行的导出/导入（清理半成品）",
    },
}

/// 窗口可见性状态（window_toggle 的返回）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    /// 切换后窗口是否可见
    visible: bool,
}

/// 获取主窗口引用（托盘/命令共用）
fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// 显示并聚焦主窗口（托盘左键 / 二次唤起复用）
pub fn show_main(app: &AppHandle) {
    if let Some(win) = main_window(app) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// 切换主窗口显示/隐藏（托盘左键呼出）
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
