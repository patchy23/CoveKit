//! FRP 模块 · runtime_commands

use super::metadata::touch_used;
use crate::plugins::frp::clients;
use crate::plugins::frp::models::FrpRuntimeState;
use crate::plugins::frp::models::FrpVerifyResult;
use crate::plugins::frp::profile;
use crate::plugins::frp::runtime;
use crate::plugins::frp::runtime::FrpState;
use crate::plugins::frp::transfer;
use crate::plugins::frp::verify;
use tauri::AppHandle;
use tauri::State;

/// 校验档案（`frpc verify -c <path>`，错误行与列按解析结果返回）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_verify(app: AppHandle, file_name: String) -> Result<FrpVerifyResult, String> {
    let dir = transfer::directory_for(&app, &file_name)?;
    let path = profile::resolve_profile_path(&dir, &file_name).await?;
    // 校验必须用档案实际绑定的客户端：否则会出现「校验通过但启动失败」
    // （不同 frpc 版本对配置字段的支持不同，服务端有版本限制时尤其明显）
    let exe = clients::resolve(&app, &file_name).await?;
    let config = super::auth::prepare(&app, &path).await?;
    let (raw, exit_ok) = verify::run_verify(&exe, &config).await?;
    let mut result = verify::build_verify_result(&file_name, &raw, exit_ok);
    if config.has_generated_file() {
        for error in &mut result.errors {
            error.line = None;
            error.column = None;
            error.message = format!("凭证运行配置校验：{}", error.message);
        }
    }
    Ok(result)
}

// ────────────────────────────── 运行命令 ──────────────────────────────

/// 启动档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_start(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    eprintln!("[frp] INFO [{file_name}] 请求启动");
    touch_used(&app, &file_name);
    runtime::start(&app, &state, &file_name)
        .await
        .inspect_err(|error| {
            eprintln!(
                "[frp] ERROR [{file_name}] 启动失败：{}",
                verify::clean_line(error)
            );
        })
}

/// 停止档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_stop(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    eprintln!("[frp] INFO [{file_name}] 请求停止");
    runtime::stop(&app, &state, &file_name)
        .await
        .inspect_err(|error| {
            eprintln!(
                "[frp] ERROR [{file_name}] 停止失败：{}",
                verify::clean_line(error)
            );
        })
}

/// 重启档案
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_restart(
    app: AppHandle,
    state: State<'_, FrpState>,
    file_name: String,
) -> Result<FrpRuntimeState, String> {
    eprintln!("[frp] INFO [{file_name}] 请求重启");
    runtime::restart(&app, &state, &file_name)
        .await
        .inspect_err(|error| {
            eprintln!(
                "[frp] ERROR [{file_name}] 重启失败：{}",
                verify::clean_line(error)
            );
        })
}

/// 全部档案的当前状态（事件为主，前端 5 秒轮询兜底）
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_status(
    app: AppHandle,
    state: State<'_, FrpState>,
) -> Result<Vec<FrpRuntimeState>, String> {
    Ok(runtime::status_all(&app, &state).await)
}
