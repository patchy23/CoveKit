//! FRP 模块 · client_commands

use crate::plugins::frp::binary;
use crate::plugins::frp::clients;
use crate::plugins::frp::models::FrpBinaryInfo;
use crate::plugins::frp::models::FrpClient;
use crate::plugins::frp::models::FrpClientList;
use crate::plugins::frp::models::FrpOpResult;
use crate::plugins::frp::models::FrpReleaseInfo;
use tauri::AppHandle;

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
    let log_started = std::time::Instant::now();
    let result: Result<FrpBinaryInfo, String> =
        async { binary::download(&app, &version).await }.await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=frp_binary_download elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_binary_download elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
    let log_started = std::time::Instant::now();
    let result: Result<FrpClient, String> =
        async { clients::add_external(&app, &path).await }.await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=frp_client_add elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_client_add elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 移除客户端登记（只删记录不删文件；同时解绑引用它的档案）
#[tauri::command(rename_all = "camelCase")]
pub fn frp_client_remove(app: AppHandle, id: String) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = (|| {
        clients::remove(&app, &id)?;
        Ok(FrpOpResult {
            ok: true,
            file_name: None,
            error: None,
        })
    })();
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_client_remove elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_client_remove elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_client_remove elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 设为默认客户端（全局唯一）
#[tauri::command(rename_all = "camelCase")]
pub fn frp_client_set_default(app: AppHandle, id: String) -> Result<FrpOpResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FrpOpResult, String> = (|| {
        clients::set_default(&app, &id)?;
        Ok(FrpOpResult {
            ok: true,
            file_name: None,
            error: None,
        })
    })();
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=frp_client_set_default elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=frp_client_set_default elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=frp_client_set_default elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
