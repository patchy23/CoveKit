//! FRP 模块 · profile_commands

use super::metadata::op_result;
use super::metadata::profile_dir;
use super::metadata::read_remarks;
use super::metadata::write_remark;
use super::MIGRATIONS;
use super::TOOL_ID;
use crate::framework::store::PluginDb;
use crate::plugins::frp::clients;
use crate::plugins::frp::models;
use crate::plugins::frp::models::FrpOpResult;
use crate::plugins::frp::models::FrpProfileContent;
use crate::plugins::frp::models::FrpProfileList;
use crate::plugins::frp::models::FrpProfileSummary;
use crate::plugins::frp::profile;
use crate::plugins::frp::runtime;
use crate::plugins::frp::runtime::FrpState;
use crate::plugins::frp::transfer;
use tauri::AppHandle;
use tauri::State;

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
            proxy_types: meta.proxy_types,
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
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        let dir = transfer::directory_for(&app, &file_name)?;
        let path = profile::write_profile_text(&dir, &file_name, &content).await?;
        Ok(op_result(&path))
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_save_text elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_save_text elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_save_text elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 表单模式保存（传入解析后的 JSON，由 profile.rs 重建 TOML 并保留未知字段）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_save_form(
    app: AppHandle,
    file_name: String,
    parsed: serde_json::Value,
) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        let dir = transfer::directory_for(&app, &file_name)?;
        let text = models::parsed_to_toml_text(&parsed)?;
        let path = profile::write_profile_text(&dir, &file_name, &text).await?;
        Ok(op_result(&path))
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_save_form elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_save_form elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_save_form elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 新建档案（内置模板）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_create(
    app: AppHandle,
    file_name: String,
    template: String,
) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        transfer::ensure_name_available(&app, &file_name)?;
        let dir = profile_dir(&app)?;
        let path = profile::create_profile(&dir, &file_name, &template).await?;
        Ok(op_result(&path))
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_create elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_create elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_create elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 复制档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_duplicate(
    app: AppHandle,
    file_name: String,
    new_name: String,
) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        transfer::ensure_name_available(&app, &new_name)?;
        let dir = transfer::directory_for(&app, &file_name)?;
        let path = profile::duplicate_profile(&dir, &file_name, &new_name).await?;
        Ok(op_result(&path))
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_duplicate elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_duplicate elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_duplicate elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 重命名档案（同步迁移备注）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_rename(
    app: AppHandle,
    file_name: String,
    new_name: String,
) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
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
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_rename elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_rename elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_rename elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 删除档案（移入同目录 `.trash/`，不物理抹除）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_delete(app: AppHandle, file_name: String) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = async {
        let _maintenance = crate::framework::context::maintenance_guard().await;
        let dir = transfer::directory_for(&app, &file_name)?;
        let path = profile::delete_profile(&dir, &file_name).await?;
        Ok(op_result(&path))
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_profile_delete elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_profile_delete elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_profile_delete elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
