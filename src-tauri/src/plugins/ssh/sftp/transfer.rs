//! SSH 插件 · 文件传输与操作（上传/下载/递归/删除/重命名/mkdir + 协作取消）

//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use russh_sftp::protocol::FileAttributes;

use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::ops::{collect_remote_entries, register_cancel, unregister_cancel, TransferState};
use super::util::{collect_upload_entries, replace_local_file, replace_remote_file};
use crate::plugins::ssh::conn::{get_session, resource_id, SshState};
use crate::plugins::ssh::models::FileTransferProgress;

/// 上传文件（本地 → 远程；后台任务分块传输 + 进度事件）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_upload(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    transfer_state: State<'_, TransferState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<FileTransferProgress, String> {
    let transfer_id = resource_id("up");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let event_connection_id = connection_id.clone();
    let entries = collect_upload_entries(&local_path, &remote_path)?;
    let total = entries.iter().map(|entry| entry.size).sum();
    let session = get_session(&ssh_state, &connection_id)?;

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
            let mut transferred: u64 = 0;
            for entry in entries {
                if cancel.is_cancelled() {
                    return Err("已取消".into());
                }
                let target_exists = sftp
                    .try_exists(&entry.remote_path)
                    .await
                    .map_err(|e| format!("检查远程目标失败: {e}"))?;
                if entry.is_dir {
                    if target_exists {
                        let metadata = sftp
                            .metadata(&entry.remote_path)
                            .await
                            .map_err(|e| format!("读取远程目录元数据失败: {e}"))?;
                        if !metadata.is_dir() {
                            return Err(format!("远程目标已存在且不是目录：{}", entry.remote_path));
                        }
                    } else {
                        sftp.create_dir(&entry.remote_path)
                            .await
                            .map_err(|e| format!("创建远程目录 {} 失败: {e}", entry.remote_path))?;
                    }
                    continue;
                }

                let target_path = if target_exists {
                    sftp.canonicalize(&entry.remote_path)
                        .await
                        .map_err(|e| format!("解析远程目标失败: {e}"))?
                } else {
                    entry.remote_path.clone()
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
                let mut local = tokio::fs::File::open(&entry.local_path)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    if cancel.is_cancelled() {
                        let _ = sftp.remove_file(&temp_path).await;
                        return Err("已取消".into());
                    }
                    let n = local.read(&mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    remote
                        .write_all(&buf[..n])
                        .await
                        .map_err(|e| format!("写入远程文件失败: {e}"))?;
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
                replace_remote_file(&sftp, &temp_path, &target_path).await?;
            }
            Ok(())
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
        unregister_cancel(cancel_registry.clone(), &tid);
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
    transfer_state: State<'_, TransferState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<FileTransferProgress, String> {
    let transfer_id = resource_id("down");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let event_connection_id = connection_id.clone();
    let session = get_session(&ssh_state, &connection_id)?;

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
                if cancel.is_cancelled() {
                    drop(local);
                    let _ = std::fs::remove_file(&temp_path);
                    return Err("已取消".into());
                }
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
        let total = result.as_ref().copied().unwrap_or(0);
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
        unregister_cancel(cancel_registry.clone(), &tid);
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

/// 递归下载（远程目录 → 本地目录；进度经事件推送；支持取消；本地侧原子替换）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_download_recursive(
    app: AppHandle,
    ssh_state: State<'_, SshState>,
    transfer_state: State<'_, TransferState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
    overwrite: Option<bool>,
) -> Result<FileTransferProgress, String> {
    let transfer_id = resource_id("downr");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let session = get_session(&ssh_state, &connection_id)?;
    let overwrite = overwrite.unwrap_or(false);
    let event_connection_id = connection_id.clone();

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
            let entries = collect_remote_entries(&sftp, &rpath, Path::new(&lpath)).await?;
            let total: u64 = entries.iter().map(|e| e.size).sum();
            let mut transferred: u64 = 0;
            for entry in entries {
                if cancel.is_cancelled() {
                    return Err("已取消".into());
                }
                if entry.is_dir {
                    std::fs::create_dir_all(&entry.local_path)
                        .map_err(|e| format!("创建本地目录失败: {e}"))?;
                    continue;
                }
                if !overwrite && Path::new(&entry.local_path).exists() {
                    return Err(format!("本地文件已存在：{}", entry.local_path.display()));
                }
                if let Some(parent) = Path::new(&entry.local_path).parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("创建本地目录失败: {e}"))?;
                }
                let mut remote = sftp
                    .open(&entry.remote_path)
                    .await
                    .map_err(|e| e.to_string())?;
                let temp_path = format!(
                    "{}.patchybox-download-{}",
                    entry.local_path.display(),
                    resource_id("file")
                );
                let mut local = tokio::fs::File::create(&temp_path)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    if cancel.is_cancelled() {
                        drop(local);
                        let _ = std::fs::remove_file(&temp_path);
                        return Err("已取消".into());
                    }
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
                            local_path: entry.local_path.display().to_string(),
                            remote_path: entry.remote_path.clone(),
                            transferred,
                            total,
                            done: false,
                            error: None,
                        },
                    );
                }
                local.flush().await.map_err(|e| e.to_string())?;
                local.shutdown().await.map_err(|e| e.to_string())?;
                replace_local_file(&temp_path, entry.local_path.to_string_lossy().as_ref())?;
            }
            Ok(())
        }
        .await;
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred: 0,
                total: 0,
                done: true,
                error: result.err(),
            },
        );
        unregister_cancel(cancel_registry.clone(), &tid);
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
