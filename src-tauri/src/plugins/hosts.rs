//! hosts 修改模块（第二批）：读取 + 备份 + 提权写入
//! 平台策略（应用本体不常驻管理员，仅保存一步提权）：
//! - Windows：写临时文件 + ps1，Start-Process -Verb RunAs 提权执行「备份 → 覆盖」（UAC 弹窗）
//! - macOS：写临时文件，osascript `do shell script ... with administrator privileges`
//!   一次授权执行「备份 → 覆盖」（系统授权框，输密码）
//! - Linux：直接写入（应用需以 root 运行；失败返回明确错误）
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use serde::Serialize;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
pub fn hosts_path() -> &'static str {
    "C:\\Windows\\System32\\drivers\\etc\\hosts"
}

#[cfg(not(target_os = "windows"))]
pub fn hosts_path() -> &'static str {
    "/etc/hosts"
}

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
    fs::read_to_string(hosts_path()).map_err(|e| format!("读取失败: {e}"))
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

/// 备份 + 写入 hosts（平台提权；取消/失败返回 ok=false）
#[tauri::command]
pub async fn hosts_save(content: String) -> Result<HostsResult, String> {
    tauri::async_runtime::spawn_blocking(move || save_hosts_blocking(&content))
        .await
        .map_err(|e| format!("提权任务失败: {e}"))?
}

fn save_hosts_blocking(content: &str) -> Result<HostsResult, String> {
    #[cfg(target_os = "windows")]
    {
        save_windows(content)
    }
    #[cfg(target_os = "macos")]
    {
        save_macos(content)
    }
    #[cfg(target_os = "linux")]
    {
        save_linux(content)
    }
}

#[cfg(target_os = "windows")]
fn save_windows(content: &str) -> Result<HostsResult, String> {
    let tmp_dir = std::env::temp_dir();
    let tmp_hosts = tmp_dir.join("patchybox-hosts.tmp");
    let tmp_ps1 = tmp_dir.join("patchybox-hosts.ps1");
    let backup = format!("{}.bak-{}", hosts_path(), timestamp());

    fs::write(&tmp_hosts, content).map_err(|e| format!("写临时文件失败: {e}"))?;

    let script = format!(
        "Copy-Item -Force '{hosts}' '{backup}'\nMove-Item -Force '{tmp}' '{hosts}'\nRemove-Item -Force '{tmp}'",
        hosts = hosts_path(),
        backup = backup,
        tmp = tmp_hosts.display(),
    );
    fs::write(&tmp_ps1, script).map_err(|e| format!("写提权脚本失败: {e}"))?;

    let cmd = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        tmp_ps1.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .status()
        .map_err(|e| format!("启动提权失败: {e}"))?;

    let _ = fs::remove_file(&tmp_ps1);
    let _ = fs::remove_file(&tmp_hosts);

    if !status.success() {
        return Ok(HostsResult {
            ok: false,
            content: String::new(),
            error: Some("提权被取消或执行失败".into()),
        });
    }
    finish_save()
}

#[cfg(target_os = "macos")]
fn save_macos(content: &str) -> Result<HostsResult, String> {
    let tmp_hosts = std::env::temp_dir().join("patchybox-hosts.tmp");
    let backup = format!("{}.bak-{}", hosts_path(), timestamp());

    fs::write(&tmp_hosts, content).map_err(|e| format!("写临时文件失败: {e}"))?;

    // 一次授权执行「备份 → 覆盖 → 清理」（弹系统授权框，需输入管理员密码）
    let script = format!(
        "do shell script \"cp '{}' '{}' && cp '{}' '{}' && rm -f '{}'\" with administrator privileges",
        hosts_path(),
        backup,
        tmp_hosts.display(),
        hosts_path(),
        tmp_hosts.display(),
    );
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| format!("启动授权失败: {e}"))?;

    let _ = fs::remove_file(&tmp_hosts);

    if !status.success() {
        return Ok(HostsResult {
            ok: false,
            content: String::new(),
            error: Some("授权被取消或执行失败".into()),
        });
    }
    finish_save()
}

#[cfg(target_os = "linux")]
fn save_linux(content: &str) -> Result<HostsResult, String> {
    let backup = format!("{}.bak-{}", hosts_path(), timestamp());
    // 先备份（失败说明无 /etc 写权限）
    fs::copy(hosts_path(), &backup).map_err(|e| format!("备份失败（应用需要 root 权限）: {e}"))?;
    fs::write(hosts_path(), content).map_err(|e| format!("写入失败（应用需要 root 权限）: {e}"))?;
    finish_save()
}

/// 保存后回读确认
fn finish_save() -> Result<HostsResult, String> {
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

/// 插件注册：命令
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        ("hosts_read", "读取 hosts 文件"),
        ("hosts_save", "备份并写入 hosts（平台提权）"),
    ])
    .expect("IPC 命令重复注册");
    builder.invoke_handler(tauri::generate_handler![hosts_read, hosts_save])
}
