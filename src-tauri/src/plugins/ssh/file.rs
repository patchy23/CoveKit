//! SSH 插件 · 文件管理（SFTP）
//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use russh_sftp::protocol::FileAttributes;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::plugins::ssh::conn::{resource_id, SshState};
use crate::plugins::ssh::models::{
    FileListResult, FileTransferProgress, RemoteFile, SshActionResult,
};

/// 从连接会话建立 SFTP 会话（临时通道，用完即弃）
async fn sftp_session(
    ssh_state: &State<'_, SshState>,
    connection_id: &str,
) -> Result<russh_sftp::client::SftpSession, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_subsystem(false, "sftp")
        .await
        .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
    let stream = channel.into_stream();
    russh_sftp::client::SftpSession::new(stream)
        .await
        .map_err(|e| format!("SFTP 初始化失败: {e}"))
}

/// 将已完整写入的临时文件安全替换为目标文件；失败时尽量恢复旧文件。
pub(crate) async fn replace_remote_file(
    fs: &russh_sftp::client::SftpSession,
    temp_path: &str,
    target_path: &str,
) -> Result<(), String> {
    if !fs
        .try_exists(target_path)
        .await
        .map_err(|e| format!("检查远程目标失败: {e}"))?
    {
        return fs
            .rename(temp_path, target_path)
            .await
            .map_err(|e| format!("提交远程文件失败: {e}"));
    }

    let backup_path = format!("{target_path}.patchybox-backup-{}", resource_id("file"));
    fs.rename(target_path, &backup_path)
        .await
        .map_err(|e| format!("备份远程原文件失败: {e}"))?;
    if let Err(error) = fs.rename(temp_path, target_path).await {
        let restore_error = fs.rename(&backup_path, target_path).await.err();
        if restore_error.is_none() {
            let _ = fs.remove_file(temp_path).await;
        }
        return Err(match restore_error {
            Some(restore) => format!("提交远程文件失败: {error}；恢复原文件也失败: {restore}"),
            None => format!("提交远程文件失败，已恢复原文件: {error}"),
        });
    }
    if let Err(error) = fs.remove_file(&backup_path).await {
        eprintln!("[ssh] 新文件已保存，但清理远程备份失败: {error}");
    }
    Ok(())
}

/// 将已完整写入的本地临时文件替换为下载目标；失败时恢复旧文件。
fn replace_local_file(temp_path: &str, target_path: &str) -> Result<(), String> {
    if !std::path::Path::new(target_path).exists() {
        return std::fs::rename(temp_path, target_path)
            .map_err(|e| format!("提交下载文件失败: {e}"));
    }
    let backup_path = format!("{target_path}.patchybox-backup-{}", resource_id("file"));
    std::fs::rename(target_path, &backup_path).map_err(|e| format!("备份本地原文件失败: {e}"))?;
    if let Err(error) = std::fs::rename(temp_path, target_path) {
        let restore_error = std::fs::rename(&backup_path, target_path).err();
        if restore_error.is_none() {
            let _ = std::fs::remove_file(temp_path);
        }
        return Err(match restore_error {
            Some(restore) => format!("提交下载文件失败: {error}；恢复原文件也失败: {restore}"),
            None => format!("提交下载文件失败，已恢复原文件: {error}"),
        });
    }
    if let Err(error) = std::fs::remove_file(&backup_path) {
        eprintln!("[ssh] 下载成功，但清理本地备份失败: {error}");
    }
    Ok(())
}

/// 判断 u32 权限位是否为目录（S_IFDIR = 0o040000）
fn is_dir_mode(mode: u32) -> bool {
    (mode & 0o170000) == 0o040000
}

/// 文件属性 → 对外 RemoteFile（纯函数，可单测）
fn to_remote_file(name: String, path: String, meta: FileAttributes) -> RemoteFile {
    let size = meta.size.unwrap_or(0);
    let is_dir = meta.permissions.map(is_dir_mode).unwrap_or(false);
    RemoteFile {
        name,
        path,
        is_dir,
        size,
        modified_at: meta.mtime.unwrap_or(0) as u64 * 1000,
        permissions: meta
            .permissions
            .map(|p| format!("{p:o}"))
            .unwrap_or_default(),
        owner: meta.user.map(|u| u.to_string()).unwrap_or_default(),
        group: meta.group.map(|g| g.to_string()).unwrap_or_default(),
    }
}

/// 目录列表
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    path: String,
) -> Result<FileListResult, String> {
    let sftp = sftp_session(&ssh_state, &connection_id).await?;
    let entries = sftp
        .read_dir(&path)
        .await
        .map_err(|e| format!("读取目录失败: {e}"))?;
    let mut files = Vec::new();
    for entry in entries {
        let meta = entry.metadata();
        let full = if path.ends_with('/') {
            format!("{path}{}", entry.file_name())
        } else {
            format!("{path}/{}", entry.file_name())
        };
        files.push(to_remote_file(entry.file_name(), full, meta));
    }
    files.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    let parent_path = if path == "/" {
        None
    } else {
        Some(
            path.rfind('/')
                .map(|i| {
                    if i == 0 {
                        "/".to_string()
                    } else {
                        path[..i].to_string()
                    }
                })
                .unwrap_or("/".to_string()),
        )
    };
    Ok(FileListResult {
        ok: true,
        path,
        parent_path,
        files,
        error: None,
    })
}

/// 上传文件（本地 → 远程；后台任务分块传输 + 进度事件）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_upload(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<FileTransferProgress, String> {
    let transfer_id = resource_id("up");
    let event_connection_id = connection_id.clone();
    let total = std::fs::metadata(&local_path)
        .map_err(|e| format!("本地文件不可读: {e}"))?
        .len();
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;

    let app2 = app.clone();
    let tid = transfer_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();
    tauri::async_runtime::spawn(async move {
        let result: Result<(), String> = async {
            let channel = session
                .channel_open_session()
                .await
                .map_err(|e| e.to_string())?;
            channel
                .request_subsystem(false, "sftp")
                .await
                .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
            let stream = channel.into_stream();
            let sftp = russh_sftp::client::SftpSession::new(stream)
                .await
                .map_err(|e| e.to_string())?;
            let target_exists = sftp
                .try_exists(&rpath)
                .await
                .map_err(|e| format!("检查远程目标失败: {e}"))?;
            let target_path = if target_exists {
                sftp.canonicalize(&rpath)
                    .await
                    .map_err(|e| format!("解析远程目标失败: {e}"))?
            } else {
                rpath.clone()
            };
            let target_permissions = if target_exists {
                sftp.metadata(&target_path)
                    .await
                    .map_err(|e| format!("读取远程目标权限失败: {e}"))?
                    .permissions
            } else {
                None
            };
            let temp_path = format!("{target_path}.patchybox-upload-{}", resource_id("file"));
            let mut remote = sftp
                .create(&temp_path)
                .await
                .map_err(|e| format!("创建远程文件失败: {e}"))?;
            let mut local = tokio::fs::File::open(&lpath)
                .await
                .map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 64 * 1024];
            let mut transferred: u64 = 0;
            loop {
                let n = local.read(&mut buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                remote
                    .write_all(&buf[..n])
                    .await
                    .map_err(|e| format!("写入远程文件失败: {e}"))?;
                transferred += n as u64;
                // 进度事件（≥256KB 或最后一块）
                let _ = app2.emit(
                    "ssh://transfer-progress",
                    &FileTransferProgress {
                        transfer_id: tid.clone(),
                        connection_id: event_connection_id.clone(),
                        local_path: lpath.clone(),
                        remote_path: rpath.clone(),
                        transferred,
                        total,
                        done: false,
                        error: None,
                    },
                );
            }
            remote
                .flush()
                .await
                .map_err(|e| format!("刷新远程文件失败: {e}"))?;
            remote
                .shutdown()
                .await
                .map_err(|e| format!("关闭远程文件失败: {e}"))?;
            if let Some(permissions) = target_permissions {
                sftp.set_metadata(
                    &temp_path,
                    FileAttributes {
                        permissions: Some(permissions),
                        ..FileAttributes::default()
                    },
                )
                .await
                .map_err(|e| format!("保留远程文件权限失败: {e}"))?;
            }
            replace_remote_file(&sftp, &temp_path, &target_path).await
        }
        .await;
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred: if result.is_ok() { total } else { 0 },
                total,
                done: true,
                error: result.err(),
            },
        );
    });

    Ok(FileTransferProgress {
        transfer_id,
        connection_id,
        local_path,
        remote_path,
        transferred: 0,
        total,
        done: false,
        error: None,
    })
}

/// 下载文件（远程 → 本地；后台任务分块传输 + 进度事件）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_download(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<FileTransferProgress, String> {
    let transfer_id = resource_id("down");
    let event_connection_id = connection_id.clone();
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;

    let app2 = app.clone();
    let tid = transfer_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();
    tauri::async_runtime::spawn(async move {
        let result: Result<u64, String> = async {
            let channel = session
                .channel_open_session()
                .await
                .map_err(|e| e.to_string())?;
            channel
                .request_subsystem(false, "sftp")
                .await
                .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
            let stream = channel.into_stream();
            let sftp = russh_sftp::client::SftpSession::new(stream)
                .await
                .map_err(|e| e.to_string())?;
            let meta = sftp
                .metadata(&rpath)
                .await
                .map_err(|e| format!("读取元数据失败: {e}"))?;
            let total = meta.size.unwrap_or(0) as u64;
            let mut remote = sftp.open(&rpath).await.map_err(|e| e.to_string())?;
            let temp_path = format!("{lpath}.patchybox-download-{}", resource_id("file"));
            let mut local = tokio::fs::File::create(&temp_path)
                .await
                .map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 64 * 1024];
            let mut transferred: u64 = 0;
            loop {
                let n = remote.read(&mut buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                local
                    .write_all(&buf[..n])
                    .await
                    .map_err(|e| format!("写入本地文件失败: {e}"))?;
                transferred += n as u64;
                let _ = app2.emit(
                    "ssh://transfer-progress",
                    &FileTransferProgress {
                        transfer_id: tid.clone(),
                        connection_id: event_connection_id.clone(),
                        local_path: lpath.clone(),
                        remote_path: rpath.clone(),
                        transferred,
                        total,
                        done: false,
                        error: None,
                    },
                );
            }
            local
                .flush()
                .await
                .map_err(|e| format!("刷新本地文件失败: {e}"))?;
            local
                .shutdown()
                .await
                .map_err(|e| format!("关闭本地文件失败: {e}"))?;
            replace_local_file(&temp_path, &lpath)?;
            Ok(total)
        }
        .await;
        let total = result.clone().unwrap_or(0);
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred: if result.is_ok() { total } else { 0 },
                total,
                done: true,
                error: result.err(),
            },
        );
    });

    Ok(FileTransferProgress {
        transfer_id,
        connection_id,
        local_path,
        remote_path,
        transferred: 0,
        total: 0,
        done: false,
        error: None,
    })
}

/// 删除远程文件/目录（目录需 recursive 或仅空目录）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_delete(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    recursive: Option<bool>,
) -> Result<SshActionResult, String> {
    let sftp = sftp_session(&ssh_state, &connection_id).await?;
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
    let sftp = sftp_session(&ssh_state, &connection_id).await?;
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
