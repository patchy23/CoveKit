//! FRP 命令装配、持久化迁移与进程清理入口。

mod auth;
pub(crate) mod binary;
mod clients;
mod models;
pub(crate) mod profile;
pub(crate) mod runtime;
mod transfer;
pub(crate) mod trash;
pub(crate) mod verify;

pub(crate) mod client_commands;
pub(crate) mod metadata;
pub(crate) mod profile_commands;
pub(crate) mod runtime_commands;
use crate::plugins::frp::runtime::FrpState;
use tauri::AppHandle;

/// 插件 id（设置键、数据文件名统一用它）
pub(crate) const TOOL_ID: &str = "frp";

/// 元数据库迁移（只追加；v1 = 备注表，v2 = 客户端清单与档案绑定）
///
/// v2 说明：客户端只登记路径、不复制文件，因此 `path` 可能是任意位置的绝对路径；
/// `profile_client` 存档案与客户端的绑定，无记录即表示跟随默认客户端。
pub(crate) const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS profile_meta (
        file_name TEXT PRIMARY KEY,
        remark TEXT NOT NULL DEFAULT '',
        last_used_at INTEGER NOT NULL DEFAULT 0
     );",
    "CREATE TABLE IF NOT EXISTS clients (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL DEFAULT '',
        path TEXT NOT NULL,
        version TEXT NOT NULL DEFAULT '',
        source TEXT NOT NULL DEFAULT 'external',
        is_default INTEGER NOT NULL DEFAULT 0,
        last_seen INTEGER NOT NULL DEFAULT 0
     );",
    "CREATE TABLE IF NOT EXISTS profile_client (
        file_name TEXT PRIMARY KEY,
        client_id TEXT NOT NULL
     );",
    "ALTER TABLE profile_meta ADD COLUMN uid TEXT NOT NULL DEFAULT '';
     ALTER TABLE profile_meta ADD COLUMN source_name TEXT NOT NULL DEFAULT '';
     UPDATE profile_meta SET uid=lower(hex(randomblob(16))) WHERE uid='';
     CREATE UNIQUE INDEX profile_meta_uid ON profile_meta(uid);
     CREATE TRIGGER profile_meta_assign_uid AFTER INSERT ON profile_meta WHEN NEW.uid=''
     BEGIN UPDATE profile_meta SET uid=lower(hex(randomblob(16))) WHERE file_name=NEW.file_name; END;",
];

/// 当前毫秒时间戳（失败回 0：只影响排序，不影响功能）
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

/// 清理（生命周期钩子，页签关闭与退出都会调用）：结束全部由本应用拉起的 frpc 进程，
/// 避免关掉界面后残留后台进程；清理由本模块自己提供，应用只协调与超时。
///
/// 页签作用域的理由：FRP 明确「不做后台常驻」，关掉工具页签后没有任何界面能停它，
/// 留一个看不见的转发进程比停掉更难排查，所以关页签即结束进程。
fn on_dispose(
    app: Option<&AppHandle>,
    _reason: crate::framework::lifecycle::CloseReason,
) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    tauri::async_runtime::block_on(runtime::shutdown_all(app));
    Vec::new()
}

/// 注册插件命令与状态（入 ipc_registry；命令体挂全局 handler；退出清理登记到统一关闭入口）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    transfer::register();
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::for_tool(IPC_OWNER, "frp")
            .with_tab_scope()
            .with_dispose(on_dispose),
    );
    builder.manage(FrpState::default())
}

pub(crate) use metadata::profile_dir;

crate::covekit_module! {
    owner: "frp",
    feature: "frp",
    commands: {
        profile_commands::frp_profile_read => "读取档案原文与 TOML 解析结果",
        profile_commands::frp_profile_save_text => "按原文保存档案（保存前自动备份）",
        profile_commands::frp_profile_duplicate => "复制档案",
        profile_commands::frp_profile_rename => "重命名档案（同步迁移备注）",
        profile_commands::frp_profile_delete => "删除档案（移入 .trash/ 软删）",
        trash::frp_profiles_deleted => "列出已删除的配置档案",
        trash::frp_profile_restore => "恢复已删除的配置档案",
        profile_commands::frp_profile_remark => "写入档案备注（只落 frp.db）",
        runtime_commands::frp_verify => "用 frpc verify 校验档案并解析错误行列",
        runtime_commands::frp_start => "启动档案对应的 frpc 进程",
        runtime_commands::frp_stop => "停止档案对应的 frpc 进程",
        runtime_commands::frp_restart => "重启档案对应的 frpc 进程",
        runtime_commands::frp_status => "查询全部档案的当前运行状态",
        client_commands::frp_binary_versions => "查询上游 frp 可用版本列表",
        client_commands::frp_binary_download => "下载并安装 frpc（含 SHA256 校验）",
        client_commands::frp_client_remove => "移除客户端登记（不删除文件）",
        client_commands::frp_client_set_default => "设为默认客户端",
        profile_commands::frp_profile_client_set => "设置档案绑定的客户端",
        profile_commands::frp_profiles_list => "档案列表（含运行状态、备注与元信息）",
        profile_commands::frp_profile_save_form => "表单模式保存档案（重建 TOML 并保留未知字段）",
        profile_commands::frp_profile_create => "新建档案（内置模板）",
        client_commands::frp_binary_detect => "探测 frpc（可传候选路径，否则走设置项与自动查找）",
        client_commands::frp_client_list => "已登记客户端列表（含默认项与文件是否还在）",
        client_commands::frp_client_add => "登记外部 frpc 可执行文件（只引用路径，不复制）",
    },
}
