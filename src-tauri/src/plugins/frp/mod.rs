//! frp 插件 · 门面（命令薄层 + 装配）
//!
//! 能力分域：`models.rs`（数据结构）/ `binary.rs`（frpc 定位与下载）/
//! `profile.rs`（档案文件读写与创建模板）/ `verify.rs`（`frpc verify` 调用与解析）/
//! `runtime.rs`（进程启停与状态机）。本文件只做参数适配、设置读取与元数据落库。
//!
//! 元数据约定：工具侧备注存 `<存储根>/data/frp.db`（`profile_meta` 表），
//! **绝不写进用户的 frpc.toml**，保证与手写配置双向互通。

pub(crate) mod binary;
mod clients;
mod models;
pub(crate) mod profile;
pub(crate) mod runtime;
mod transfer;
pub(crate) mod verify;

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::framework::store::PluginDb;
use crate::plugins::frp::models::{
    FrpBinaryInfo, FrpOpResult, FrpProfileContent, FrpProfileList, FrpProfileSummary,
    FrpReleaseInfo, FrpRuntimeState, FrpVerifyResult,
};
use crate::plugins::frp::models::{FrpClient, FrpClientList};
use crate::plugins::frp::runtime::FrpState;

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

/// 工具设置读取（settings.json 的 `app.tools.frp.<key>`）
fn tool_setting(app: &AppHandle, key: &str) -> Option<String> {
    // 经框架读路径（合并设备层与空间层）：插件不得直读设置文件
    crate::framework::settings::tool_setting(app, TOOL_ID, key)
}

/// 配置目录：工具设置 `profileDir` 优先，否则 `<存储根>/data/frp/profiles`
pub(crate) fn profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let configured = tool_setting(app, "profileDir").unwrap_or_default();
    if !configured.trim().is_empty() {
        return Ok(PathBuf::from(configured.trim()));
    }
    let dir = crate::framework::paths::data_dir(app)?
        .join(TOOL_ID)
        .join("profiles");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建配置目录失败（{}）：{e}", dir.display()))?;
    Ok(dir)
}

/// 读取全部备注（无库或无记录时返回空表，不影响列表展示）
fn read_remarks(app: &AppHandle) -> HashMap<String, String> {
    let Ok(db) = PluginDb::open(app, TOOL_ID, MIGRATIONS) else {
        return HashMap::new();
    };
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT file_name, remark FROM profile_meta")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        for row in rows.flatten() {
            map.insert(row.0, row.1);
        }
        Ok(map)
    })
    .unwrap_or_default()
}

/// 写入备注（upsert；空串即清除备注内容）
fn write_remark(app: &AppHandle, file_name: &str, remark: &str) -> Result<(), String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO profile_meta (file_name, remark, last_used_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(file_name) DO UPDATE SET remark = excluded.remark, last_used_at = excluded.last_used_at",
            rusqlite::params![file_name, remark, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 由落盘路径组装操作结果（各写操作共用）
fn op_result(path: &std::path::Path) -> FrpOpResult {
    FrpOpResult {
        ok: true,
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from),
        error: None,
    }
}

/// 记录「最近使用」时间（启动时调用；失败只记日志，不打断启动流程）
fn touch_used(app: &AppHandle, file_name: &str) {
    if let Ok(db) = PluginDb::open(app, TOOL_ID, MIGRATIONS) {
        let _ = db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO profile_meta (file_name, remark, last_used_at) VALUES (?1, '', ?2)
                 ON CONFLICT(file_name) DO UPDATE SET last_used_at = excluded.last_used_at",
                rusqlite::params![file_name, now_ms()],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        });
    }
}

// ────────────────────────────── 档案命令 ──────────────────────────────

/// 列出配置目录下的全部档案（含运行状态、备注与基础元信息）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profiles_list(
    app: AppHandle,
    state: State<'_, FrpState>,
) -> Result<FrpProfileList, String> {
    let dir = profile_dir(&app)?;
    let mut files = match profile::list_profile_files(&dir).await {
        Ok(files) => files,
        Err(message) => {
            return Ok(FrpProfileList {
                ok: false,
                dir: dir.display().to_string(),
                profiles: Vec::new(),
                error: Some(message),
            });
        }
    };
    let managed = transfer::managed_dir(&app)?;
    if managed.is_dir() && managed != dir {
        let imported = profile::list_profile_files(&managed).await?;
        for path in imported {
            files.retain(|existing| existing.file_name() != path.file_name());
            files.push(path);
        }
    }
    let remarks = read_remarks(&app);
    let mut profiles = Vec::with_capacity(files.len());
    for path in files {
        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from)
        else {
            continue;
        };
        let display_name = transfer::remember(&app, &file_name)?;
        // 读文件失败（权限/编码）不阻断列表：按空内容展示，用户仍能看到这一条
        let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
        let meta = models::parse_meta(&text);
        let live = runtime::state_of(&state, &file_name).await;
        let mtime = tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|info| info.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(0))
            .unwrap_or(0);
        let remark = remarks.get(&file_name).cloned().unwrap_or_default();
        profiles.push(FrpProfileSummary {
            file_name: file_name.clone(),
            display_name: display_name.trim_end_matches(".toml").to_string(),
            remark,
            server_addr: meta.server_addr,
            server_port: meta.server_port,
            proxy_count: meta.proxy_count,
            enabled_proxy_count: meta.enabled_proxy_count,
            mtime: i64::try_from(mtime).unwrap_or(i64::MAX),
            client_id: clients::binding(&app, &file_name),
            state: live.state,
            pid: live.pid,
            last_error: live.last_error,
        });
    }
    Ok(FrpProfileList {
        ok: true,
        dir: dir.display().to_string(),
        profiles,
        error: None,
    })
}

/// 读取单个档案（原文 + 解析结果 + 是否含注释）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_read(
    app: AppHandle,
    file_name: String,
) -> Result<FrpProfileContent, String> {
    let dir = transfer::directory_for(&app, &file_name)?;
    let content = profile::read_profile_text(&dir, &file_name).await?;
    let parsed = models::toml_text_to_value(&content)?;
    Ok(FrpProfileContent {
        ok: true,
        file_name,
        has_comments: models::contains_comments(&content),
        content,
        parsed,
        error: None,
    })
}

/// 源码模式保存（写原文，改前自动备份）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_save_text(
    app: AppHandle,
    file_name: String,
    content: String,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    let dir = transfer::directory_for(&app, &file_name)?;
    let path = profile::write_profile_text(&dir, &file_name, &content).await?;
    Ok(op_result(&path))
}

/// 表单模式保存（传入解析后的 JSON，由 profile.rs 重建 TOML 并保留未知字段）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_save_form(
    app: AppHandle,
    file_name: String,
    parsed: serde_json::Value,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    let dir = transfer::directory_for(&app, &file_name)?;
    let text = models::parsed_to_toml_text(&parsed)?;
    let path = profile::write_profile_text(&dir, &file_name, &text).await?;
    Ok(op_result(&path))
}

/// 新建档案（内置模板）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_create(
    app: AppHandle,
    file_name: String,
    template: String,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    transfer::ensure_name_available(&app, &file_name)?;
    let dir = profile_dir(&app)?;
    let path = profile::create_profile(&dir, &file_name, &template).await?;
    Ok(op_result(&path))
}

/// 复制档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_duplicate(
    app: AppHandle,
    file_name: String,
    new_name: String,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    transfer::ensure_name_available(&app, &new_name)?;
    let dir = transfer::directory_for(&app, &file_name)?;
    let path = profile::duplicate_profile(&dir, &file_name, &new_name).await?;
    Ok(op_result(&path))
}

/// 重命名档案（同步迁移备注）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_rename(
    app: AppHandle,
    file_name: String,
    new_name: String,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    if new_name != file_name {
        transfer::ensure_name_available(&app, &new_name)?;
    }
    let dir = transfer::directory_for(&app, &file_name)?;
    transfer::remember(&app, &file_name)?;
    let path = profile::rename_profile(&dir, &file_name, &new_name).await?;
    PluginDb::open(&app, TOOL_ID, MIGRATIONS)?.with_conn(|conn| {
        conn.execute(
            "UPDATE profile_meta SET file_name=?1,source_name='' WHERE file_name=?2",
            rusqlite::params![new_name, file_name],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })?;
    let result = op_result(&path);
    // 备注跟着档案名迁移，避免重命名后备注「丢失」
    if let Some(target) = result.file_name.clone() {
        if let Some(remark) = read_remarks(&app).get(&file_name).cloned() {
            let _ = write_remark(&app, &target, &remark);
        }
    }
    Ok(result)
}

/// 删除档案（移入同目录 `.trash/`，不物理抹除）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_delete(app: AppHandle, file_name: String) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    let dir = transfer::directory_for(&app, &file_name)?;
    let path = profile::delete_profile(&dir, &file_name).await?;
    Ok(op_result(&path))
}

/// 写入档案备注（只落 frp.db，不动用户 TOML）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_remark(
    app: AppHandle,
    file_name: String,
    remark: String,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    let dir = transfer::directory_for(&app, &file_name)?;
    // 先确认档案存在，避免给不存在的文件留下孤儿备注
    profile::resolve_profile_path(&dir, &file_name).await?;
    write_remark(&app, &file_name, &remark)?;
    Ok(FrpOpResult {
        ok: true,
        file_name: Some(file_name),
        error: None,
    })
}

/// 校验档案（`frpc verify -c <path>`，错误行与列按解析结果返回）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_verify(app: AppHandle, file_name: String) -> Result<FrpVerifyResult, String> {
    let dir = transfer::directory_for(&app, &file_name)?;
    let path = profile::resolve_profile_path(&dir, &file_name).await?;
    // 校验必须用档案实际绑定的客户端：否则会出现「校验通过但启动失败」
    // （不同 frpc 版本对配置字段的支持不同，服务端有版本限制时尤其明显）
    let exe = clients::resolve(&app, &file_name).await?;
    let (raw, exit_ok) = verify::run_verify(&exe, &path).await?;
    Ok(verify::build_verify_result(&file_name, &raw, exit_ok))
}

// ────────────────────────────── 运行命令 ──────────────────────────────

/// 启动档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_start(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    touch_used(&app, &file_name);
    runtime::start(&app, &state, &file_name).await
}

/// 停止档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_stop(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    runtime::stop(&app, &state, &file_name).await
}

/// 重启档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_restart(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    runtime::restart(&app, &state, &file_name).await
}

/// 全部档案的当前状态（事件为主，前端 5 秒轮询兜底）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_status(
    app: AppHandle,
    state: State<'_, FrpState>,
) -> Result<Vec<FrpRuntimeState>, String> {
    Ok(runtime::status_all(&app, &state).await)
}

// ────────────────────────────── frpc 命令 ──────────────────────────────

/// 探测 frpc（可传入候选路径；未传则走设置项与自动查找）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_binary_detect(
    app: AppHandle,
    path: Option<String>,
) -> Result<FrpBinaryInfo, String> {
    Ok(binary::detect_with(&app, path.as_deref()).await)
}

/// 上游可用版本列表
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_binary_versions(
    _app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<FrpReleaseInfo>, String> {
    binary::versions(limit.unwrap_or(10)).await
}

/// 下载并安装 frpc（进度走 `frp://download` 事件；失败不改动已配置路径）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_binary_download(app: AppHandle, version: String) -> Result<FrpBinaryInfo, String> {
    binary::download(&app, &version).await
}

// ──────────────────────── 客户端管理命令 ────────────────────────

/// 列出已登记的客户端（含默认项与「文件是否还在」）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_client_list(app: AppHandle) -> Result<FrpClientList, String> {
    Ok(clients::list(&app).await)
}

/// 登记一个外部 frpc 可执行文件（只引用路径，不复制文件）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_client_add(app: AppHandle, path: String) -> Result<FrpClient, String> {
    clients::add_external(&app, &path).await
}

/// 移除客户端登记（只删记录不删文件；同时解绑引用它的档案）
#[tauri::command(rename_all = "camelCase")]
pub fn frp_client_remove(app: AppHandle, id: String) -> Result<FrpOpResult, String> {
    clients::remove(&app, &id)?;
    Ok(FrpOpResult {
        ok: true,
        file_name: None,
        error: None,
    })
}

/// 设为默认客户端（全局唯一）
#[tauri::command(rename_all = "camelCase")]
pub fn frp_client_set_default(app: AppHandle, id: String) -> Result<FrpOpResult, String> {
    clients::set_default(&app, &id)?;
    Ok(FrpOpResult {
        ok: true,
        file_name: None,
        error: None,
    })
}

/// 设置档案绑定的客户端（clientId 为空则解除绑定、回到跟随默认）
#[tauri::command(rename_all = "camelCase")]
pub fn frp_profile_client_set(
    app: AppHandle,
    file_name: String,
    client_id: Option<String>,
) -> Result<FrpOpResult, String> {
    let normalized = client_id.filter(|value| !value.trim().is_empty());
    clients::bind(&app, &file_name, normalized.as_deref())?;
    Ok(FrpOpResult {
        ok: true,
        file_name: Some(file_name),
        error: None,
    })
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

// 模块静态清单：命令名、入库元数据与分派 handler 同源生成（AR07 §10.2）
crate::patchybox_module! {
    owner: "frp",
    feature: "frp",
    commands: {
        frp_profile_read => "读取档案原文与 TOML 解析结果",
        frp_profile_save_text => "按原文保存档案（保存前自动备份）",
        frp_profile_duplicate => "复制档案",
        frp_profile_rename => "重命名档案（同步迁移备注）",
        frp_profile_delete => "删除档案（移入 .trash/ 软删）",
        frp_profile_remark => "写入档案备注（只落 frp.db）",
        frp_verify => "用 frpc verify 校验档案并解析错误行列",
        frp_start => "启动档案对应的 frpc 进程",
        frp_stop => "停止档案对应的 frpc 进程",
        frp_restart => "重启档案对应的 frpc 进程",
        frp_status => "查询全部档案的当前运行状态",
        frp_binary_versions => "查询上游 frp 可用版本列表",
        frp_binary_download => "下载并安装 frpc（含 SHA256 校验）",
        frp_client_remove => "移除客户端登记（不删除文件）",
        frp_client_set_default => "设为默认客户端",
        frp_profile_client_set => "设置档案绑定的客户端",
        frp_profiles_list => "档案列表（含运行状态、备注与元信息）",
        frp_profile_save_form => "表单模式保存档案（重建 TOML 并保留未知字段）",
        frp_profile_create => "新建档案（内置模板）",
        frp_binary_detect => "探测 frpc（可传候选路径，否则走设置项与自动查找）",
        frp_client_list => "已登记客户端列表（含默认项与文件是否还在）",
        frp_client_add => "登记外部 frpc 可执行文件（只引用路径，不复制）",
    },
}
