//! SSH 插件 · 文件传输与操作（上传/下载/递归/删除/重命名/mkdir + 协作取消）

//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

use crate::plugins::ssh::conn::{get_sftp_session, SshState};
use crate::plugins::ssh::models::SshActionResult;
use crate::plugins::ssh::sftp::util::{is_dir_mode, LocalUploadEntry};

/// 删除远程文件/目录（目录需 recursive 或仅空目录）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_delete(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    recursive: Option<bool>,
) -> Result<SshActionResult, String> {
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
