//! hosts 修改模块（第二批）：读取 + 备份 + 提权写入（UAC 最小授权）
//! 方案：应用本体不常驻管理员；保存时写临时文件 + ps1 脚本（普通权限），
//! 再经 Start-Process -Verb RunAs 提权执行"备份 → 覆盖"（仅此一步需要管理员）。
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use serde::Serialize;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub const HOSTS_PATH: &str = "C:\\Windows\\System32\\drivers\\etc\\hosts";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsResult {
    ok: bool,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

fn read_hosts() -> Result<String, String> {
    fs::read_to_string(HOSTS_PATH).map_err(|e| format!("读取失败: {e}"))
}

/// 读取 hosts 文件（普通权限可读）
#[tauri::command]
pub fn hosts_read() -> Result<HostsResult, String> {
    match read_hosts() {
        Ok(c) => Ok(HostsResult {
            ok: true,
            content: c,
            error: None,
        }),
        Err(e) => Ok(HostsResult {
            ok: false,
            content: String::new(),
            error: Some(e),
        }),
    }
}

/// 备份 + 写入 hosts（UAC 提权；取消/失败返回 ok=false）
#[tauri::command]
pub async fn hosts_save(content: String) -> Result<HostsResult, String> {
    tauri::async_runtime::spawn_blocking(move || save_hosts_blocking(&content))
        .await
        .map_err(|e| format!("提权任务失败: {e}"))?
}

fn save_hosts_blocking(content: &str) -> Result<HostsResult, String> {
    let tmp_dir = std::env::temp_dir();
    let tmp_hosts = tmp_dir.join("patchybox-hosts.tmp");
    let tmp_ps1 = tmp_dir.join("patchybox-hosts.ps1");
    let backup = format!("{HOSTS_PATH}.bak-{}", timestamp());

    // 1. 写内容到临时文件（普通权限）
    fs::write(&tmp_hosts, content).map_err(|e| format!("写临时文件失败: {e}"))?;

    // 2. 写提权脚本：备份 → 覆盖 → 清理临时文件
    let script = format!(
        "Copy-Item -Force '{hosts}' '{backup}'\nMove-Item -Force '{tmp}' '{hosts}'\nRemove-Item -Force '{tmp}'",
        hosts = HOSTS_PATH,
        backup = backup,
        tmp = tmp_hosts.display(),
    );
    fs::write(&tmp_ps1, script).map_err(|e| format!("写提权脚本失败: {e}"))?;

    // 3. UAC 提权执行（Start-Process -Verb RunAs -Wait；用户取消则命令失败）
    let cmd = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        tmp_ps1.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .status()
        .map_err(|e| format!("启动提权失败: {e}"))?;

    // 4. 清理
    let _ = fs::remove_file(&tmp_ps1);
    let _ = fs::remove_file(&tmp_hosts);

    if !status.success() {
        return Ok(HostsResult {
            ok: false,
            content: String::new(),
            error: Some("提权被取消或执行失败".into()),
        });
    }

    // 5. 回读确认
    match read_hosts() {
        Ok(c) => Ok(HostsResult {
            ok: true,
            content: c,
            error: None,
        }),
        Err(e) => Ok(HostsResult {
            ok: false,
            content: String::new(),
            error: Some(e),
        }),
    }
}
