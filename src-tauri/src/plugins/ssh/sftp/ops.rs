//! SSH 插件 · 远程文件操作（删除/重命名/mkdir/chmod/新建 + 递归展开 + 传输取消注册表）
//! 浏览与操作走 conn::get_sftp_session 长驻会话（不逐操作新建通道）；
//! 递归展开的服务端目录项一律过 util::check_entry_name（防恶意服务端路径穿越）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

use crate::plugins::ssh::conn::{get_sftp_session, SshState};
use crate::plugins::ssh::models::SshActionResult;
use crate::plugins::ssh::sftp::util::{
    check_chmod_allowed, check_delete_allowed, check_entry_name, is_dir_mode, LocalUploadEntry,
};

/// 删除远程文件/目录（目录需 recursive 或仅空目录）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_delete(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    recursive: Option<bool>,
) -> Result<SshActionResult, String> {
    // 系统路径删除拦截（后端最后防线；本体及子树一律禁止，根下自定义目录放行）
    if let Err(msg) = check_delete_allowed(&remote_path) {
        return Ok(SshActionResult {
            ok: false,
            error: Some(msg),
        });
    }
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    let meta = sftp
        .metadata(&remote_path)
        .await
        .map_err(|e| e.to_string())?;
    let is_dir = meta.permissions.map(is_dir_mode).unwrap_or(false);
    let result = if is_dir {
        if recursive.unwrap_or(false) {
            remove_dir_recursive(&sftp, &remote_path).await
        } else {
            sftp.remove_dir(&remote_path)
                .await
                .map_err(|e| e.to_string())
        }
    } else {
        sftp.remove_file(&remote_path)
            .await
            .map_err(|e| e.to_string())
    };
    match result {
        Ok(_) => Ok(SshActionResult {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(SshActionResult {
            ok: false,
            error: Some(format!("删除失败: {e}")),
        }),
    }
}

/// 递归删除目录（SFTP 无递归 API，遍历子项后自底向上删）
async fn remove_dir_recursive(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> Result<(), String> {
    let entries = sftp.read_dir(path).await.map_err(|e| e.to_string())?;
    for entry in entries {
        check_entry_name(&entry.file_name())?;
        let child = if path.ends_with('/') {
            format!("{path}{}", entry.file_name())
        } else {
            format!("{path}/{}", entry.file_name())
        };
        let meta = entry.metadata();
        if meta.permissions.map(is_dir_mode).unwrap_or(false) {
            // 递归删除子目录（async 递归需显式 box，见 E0733）
            Box::pin(remove_dir_recursive(sftp, &child)).await?;
        } else {
            sftp.remove_file(&child).await.map_err(|e| e.to_string())?;
        }
    }
    sftp.remove_dir(path).await.map_err(|e| e.to_string())
}

/// 重命名远程文件/目录
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_rename(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    old_path: String,
    new_path: String,
) -> Result<SshActionResult, String> {
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    match sftp.rename(&old_path, &new_path).await {
        Ok(_) => Ok(SshActionResult {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(SshActionResult {
            ok: false,
            error: Some(format!("重命名失败: {e}")),
        }),
    }
}

/* ── 传输取消注册表 ── */

/// 传输任务取消标志：transferId → 取消位（协作式；循环内检查并清理临时文件）
pub struct TransferState(
    pub Arc<std::sync::Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>>,
);

/// 传输任务的取消位句柄：任务持有共享位，取消命令置 true
pub(crate) struct CancelFlag(Arc<std::sync::atomic::AtomicBool>);

impl CancelFlag {
    /// 是否已被请求取消
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// 注册取消位并返回句柄
pub(crate) fn register_cancel(state: &TransferState, transfer_id: &str) -> CancelFlag {
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut map) = state.0.lock() {
        map.insert(transfer_id.to_string(), flag.clone());
    }
    CancelFlag(flag)
}

/// 移除取消位（任务结束时调用；接收可克隆的注册表句柄）
pub(crate) fn unregister_cancel(
    registry: Arc<std::sync::Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>>,
    transfer_id: &str,
) {
    if let Ok(mut map) = registry.lock() {
        map.remove(transfer_id);
    }
}

/// 取消传输任务（命令）：置位即可；传输循环负责清理临时文件并结束
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_transfer_cancel(
    state: State<'_, TransferState>,
    transfer_id: String,
) -> Result<SshActionResult, String> {
    let found = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        map.get(&transfer_id).map(|f| {
            f.store(true, std::sync::atomic::Ordering::Relaxed);
        })
    };
    Ok(SshActionResult {
        ok: found.is_some(),
        error: found
            .is_none()
            .then(|| "传输任务不存在或已结束".to_string()),
    })
}

/* ── 本地目录列表（双栏文件管理的本地侧） ── */

/// 新建远程目录（仅单级；多级由前端逐级调用或先建父目录）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_mkdir(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    path: String,
) -> Result<SshActionResult, String> {
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    match sftp.create_dir(&path).await {
        Ok(_) => Ok(SshActionResult {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(SshActionResult {
            ok: false,
            error: Some(format!("创建目录失败: {e}")),
        }),
    }
}

/* ── 递归下载 ── */

/// 递归展开远程目录（父目录优先，返回扁平清单）
pub(crate) async fn collect_remote_entries(
    sftp: &russh_sftp::client::SftpSession,
    remote_path: &str,
    local_path: &Path,
) -> Result<Vec<LocalUploadEntry>, String> {
    let meta = sftp
        .metadata(remote_path)
        .await
        .map_err(|e| format!("读取远程元数据失败: {e}"))?;
    if !meta.permissions.map(is_dir_mode).unwrap_or(false) {
        return Ok(vec![LocalUploadEntry {
            local_path: local_path.to_path_buf(),
            remote_path: remote_path.to_string(),
            is_dir: false,
            size: meta.size.unwrap_or(0),
        }]);
    }
    let mut entries = vec![LocalUploadEntry {
        local_path: local_path.to_path_buf(),
        remote_path: remote_path.to_string(),
        is_dir: true,
        size: 0,
    }];
    let mut pending = vec![remote_path.to_string()];
    while let Some(directory) = pending.pop() {
        let remote_entries = sftp.read_dir(&directory).await.map_err(|e| e.to_string())?;
        for entry in remote_entries {
            let name = entry.file_name();
            check_entry_name(&name)?;
            let child_remote = if directory.ends_with('/') {
                format!("{directory}{name}")
            } else {
                format!("{directory}/{name}")
            };
            let relative = Path::new(&child_remote)
                .strip_prefix(remote_path)
                .map_err(|e| format!("计算相对路径失败: {e}"))?;
            let child_local = local_path.join(relative);
            let meta = entry.metadata();
            if meta.permissions.map(is_dir_mode).unwrap_or(false) {
                entries.push(LocalUploadEntry {
                    local_path: child_local.clone(),
                    remote_path: child_remote.clone(),
                    is_dir: true,
                    size: 0,
                });
                pending.push(child_remote);
            } else {
                entries.push(LocalUploadEntry {
                    local_path: child_local,
                    remote_path: child_remote,
                    is_dir: false,
                    size: meta.size.unwrap_or(0),
                });
            }
        }
    }
    Ok(entries)
}

/// 新建远程空文件（已存在则报错，防覆盖）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_create(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
) -> Result<SshActionResult, String> {
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    // CREATE|EXCLUDE = 原子 create_new 语义：已存在由服务端直接报错，
    // 替代「先 stat 探测再 create」的 TOCTOU 窗口（探测失败被误当不存在会 truncate 覆盖已有文件）
    let result = match sftp
        .open_with_flags(
            &remote_path,
            russh_sftp::protocol::OpenFlags::CREATE
                | russh_sftp::protocol::OpenFlags::EXCLUDE
                | russh_sftp::protocol::OpenFlags::WRITE,
        )
        .await
    {
        Ok(file) => file.close().await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    match result {
        Ok(_) => Ok(SshActionResult {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(SshActionResult {
            ok: false,
            error: Some(format!("新建文件失败: {e}")),
        }),
    }
}

/// 修改远程文件/目录权限（安全策略见 util::check_chmod_allowed；目录可递归）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_chmod(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    mode: u32,
    recursive: Option<bool>,
    acknowledge_risk: Option<bool>,
) -> Result<SshActionResult, String> {
    let recursive = recursive.unwrap_or(false);
    if let Err(msg) =
        check_chmod_allowed(&remote_path, recursive, acknowledge_risk.unwrap_or(false))
    {
        return Ok(SshActionResult {
            ok: false,
            error: Some(msg),
        });
    }
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    let result = if recursive {
        chmod_recursive(&sftp, &remote_path, mode).await
    } else {
        set_mode(&sftp, &remote_path, mode).await
    };
    match result {
        Ok(_) => Ok(SshActionResult {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(SshActionResult {
            ok: false,
            error: Some(format!("修改权限失败: {e}")),
        }),
    }
}

/// setstat 单目标权限
async fn set_mode(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
    mode: u32,
) -> Result<(), String> {
    let attrs = russh_sftp::protocol::FileAttributes {
        permissions: Some(mode),
        ..Default::default()
    };
    sftp.set_metadata(path, attrs)
        .await
        .map_err(|e| e.to_string())
}

/// 递归 chmod：先子后己，逐目标 setstat
async fn chmod_recursive(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
    mode: u32,
) -> Result<(), String> {
    let meta = sftp.metadata(path).await.map_err(|e| e.to_string())?;
    if meta.permissions.map(is_dir_mode).unwrap_or(false) {
        let entries = sftp.read_dir(path).await.map_err(|e| e.to_string())?;
        for entry in entries {
            check_entry_name(&entry.file_name())?;
            let child = if path.ends_with('/') {
                format!("{path}{}", entry.file_name())
            } else {
                format!("{path}/{}", entry.file_name())
            };
            Box::pin(chmod_recursive(sftp, &child, mode)).await?;
        }
    }
    set_mode(sftp, path, mode).await
}
