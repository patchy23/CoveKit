//! SSH 插件 · 文件传输（上传/下载/递归下载 + 协作取消）
//! 传输走独立 SFTP 通道（不与交互浏览共用的长驻会话抢请求窗口）；
//! 分块传输，进度事件经节流后推送 ssh://transfer-progress；临时文件 + 原子替换落盘。

use russh_sftp::protocol::FileAttributes;

use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::ops::{collect_remote_entries, register_cancel, unregister_cancel, TransferState};
use super::util::{collect_upload_entries, replace_local_file, replace_remote_file};
use crate::plugins::ssh::conn::{get_session, resource_id, SshState};
use crate::plugins::ssh::models::FileTransferProgress;

/// 进度事件节流间隔：中间事件距上次不足此间隔则丢弃（64KB 块直发会在高速链路打爆 IPC）。
/// 首块必发；完成/失败事件不走节流，保证最终状态一定送达。
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// 进度事件节流器（每传输任务一个）
struct ProgressThrottle {
    /// 上次实际发送时间（None = 尚未发过，首块必发）
    last: Option<std::time::Instant>,
}

impl ProgressThrottle {
    fn new() -> Self {
        Self { last: None }
    }

    /// 本次中间事件是否应发送
    fn should_emit(&mut self) -> bool {
        let now = std::time::Instant::now();
        match self.last {
            Some(t) if now.duration_since(t) < PROGRESS_INTERVAL => false,
            _ => {
                self.last = Some(now);
                true
            }
        }
    }
}

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
    let log_started = std::time::Instant::now();
    let result: Result<FileTransferProgress, String> = async {
    let transfer_id = resource_id("up");
    // 本地目录递归遍历是同步阻塞 IO，移到 spawn_blocking 不卡 tokio worker
    let (scan_local, scan_remote) = (local_path.clone(), remote_path.clone());
    let entries = tokio::task::spawn_blocking(move || collect_upload_entries(&scan_local, &scan_remote))
        .await
        .map_err(|e| format!("遍历本地目录任务失败: {e}"))??;
    let total = entries.iter().map(|entry| entry.size).sum();
    let session = get_session(&ssh_state, &connection_id)?;
    // 注册取消位放在所有可失败步骤之后：前置失败不会产生永久残留的注册表项
    log::info!("文件传输开始 task={transfer_id} session={connection_id}");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let event_connection_id = connection_id.clone();

    let app2 = app.clone();
    let tid = transfer_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();
    tauri::async_runtime::spawn(async move {
        // 提升到闭包层：结束事件要携带失败时的实际已传字节数
        let mut transferred: u64 = 0;
        let mut throttle = ProgressThrottle::new();
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
                let temp_path = format!("{target_path}.covekit-upload-{}", resource_id("file"));
                // 写入阶段任一失败（含取消）都要清理远程临时文件，避免 .covekit-upload-* 残留
                let write_result: Result<(), String> = async {
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
                        if throttle.should_emit() {
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
                    Ok(())
                }
                .await;
                if let Err(error) = write_result {
                    let _ = sftp.remove_file(&temp_path).await;
                    return Err(error);
                }
                replace_remote_file(&sftp, &temp_path, &target_path).await?;
            }
            Ok(())
        }
        .await;
        // 结束事件携带实际进度：失败时从已传字节数继续展示，不回跳 0
        let succeeded = result.is_ok();
        if cancel.is_cancelled() {
            log::info!("文件传输已取消 task={tid} session={event_connection_id}");
        } else if result.is_err() {
            log::error!("文件传输失败 task={tid} session={event_connection_id} code=sftp.transfer_failed");
        } else {
            log::info!("文件传输完成 task={tid} session={event_connection_id}");
        }
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred: if succeeded { total } else { transferred },
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
    }.await;
    match &result {
        Ok(_value) => log::info!(
            "传输请求已受理 operation=ssh_file_upload elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=ssh_file_upload elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
    let log_started = std::time::Instant::now();
    let result: Result<FileTransferProgress, String> = async {
    let transfer_id = resource_id("down");
    let session = get_session(&ssh_state, &connection_id)?;
    // 注册取消位放在所有可失败步骤之后：前置失败不会产生永久残留的注册表项
    log::info!("文件传输开始 task={transfer_id} session={connection_id}");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let event_connection_id = connection_id.clone();

    let app2 = app.clone();
    let tid = transfer_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();
    tauri::async_runtime::spawn(async move {
        // 临时文件路径提升到闭包层：任何失败（含取消）统一在事后清理
        let temp_path = format!("{lpath}.covekit-download-{}", resource_id("file"));
        let mut throttle = ProgressThrottle::new();
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
            // russh-sftp size 本就是 Option<u64>，缺省 0 表示服务端未报（进度条按不定长展示）
            let total = meta.size.unwrap_or(0);
            let mut remote = sftp.open(&rpath).await.map_err(|e| e.to_string())?;
            let mut local = tokio::fs::File::create(&temp_path)
                .await
                .map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 64 * 1024];
            let mut transferred: u64 = 0;
            loop {
                if cancel.is_cancelled() {
                    // 句柄随块退出自动关闭，临时文件由闭包层统一清理
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
                if throttle.should_emit() {
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
            }
            local
                .flush()
                .await
                .map_err(|e| format!("刷新本地文件失败: {e}"))?;
            local
                .shutdown()
                .await
                .map_err(|e| format!("关闭本地文件失败: {e}"))?;
            replace_local_file(&temp_path, &lpath).await?;
            Ok(transferred)
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temp_path).await;
        }
        // 成功值是实际写入字节数；失败为 0（单文件下载失败无保留进度语义）
        let written = result.as_ref().copied().unwrap_or(0);
        if cancel.is_cancelled() {
            log::info!("文件传输已取消 task={tid} session={event_connection_id}");
        } else if result.is_err() {
            log::error!("文件传输失败 task={tid} session={event_connection_id} code=sftp.transfer_failed");
        } else {
            log::info!("文件传输完成 task={tid} session={event_connection_id}");
        }
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred: written,
                total: written,
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
    }.await;
    match &result {
        Ok(_value) => log::info!(
            "传输请求已受理 operation=ssh_file_download elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=ssh_file_download elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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
    let log_started = std::time::Instant::now();
    let result: Result<FileTransferProgress, String> = async {
    let transfer_id = resource_id("downr");
    let session = get_session(&ssh_state, &connection_id)?;
    // 注册取消位放在所有可失败步骤之后：前置失败不会产生永久残留的注册表项
    log::info!("文件传输开始 task={transfer_id} session={connection_id}");
    let cancel = register_cancel(&transfer_state, &transfer_id);
    let cancel_registry = transfer_state.0.clone();
    let overwrite = overwrite.unwrap_or(false);
    let event_connection_id = connection_id.clone();

    let app2 = app.clone();
    let tid = transfer_id.clone();
    let rpath = remote_path.clone();
    let lpath = local_path.clone();
    tauri::async_runtime::spawn(async move {
        let mut throttle = ProgressThrottle::new();
        // 提升到闭包层：结束事件携带真实总量与已传字节数（不再发 total: 0 的假进度）
        let mut total: u64 = 0;
        let mut transferred: u64 = 0;
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
            let entries = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err("已取消".into()),
                result = collect_remote_entries(&sftp, &rpath, Path::new(&lpath)) => result?,
            };
            total = entries.iter().map(|e| e.size).sum();
            for entry in entries {
                if cancel.is_cancelled() {
                    return Err("已取消".into());
                }
                if entry.is_dir {
                    tokio::fs::create_dir_all(&entry.local_path)
                        .await
                        .map_err(|e| format!("创建本地目录失败: {e}"))?;
                    continue;
                }
                if !overwrite
                    && tokio::fs::try_exists(&entry.local_path)
                        .await
                        .map_err(|e| format!("检查本地目标失败: {e}"))?
                {
                    return Err(format!("本地文件已存在：{}", entry.local_path.display()));
                }
                if let Some(parent) = Path::new(&entry.local_path).parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("创建本地目录失败: {e}"))?;
                }
                let temp_path = format!(
                    "{}.covekit-download-{}",
                    entry.local_path.display(),
                    resource_id("file")
                );
                // 写入阶段任一失败（含取消）都要清理本地临时文件，避免 .covekit-download-* 残留
                let write_result: Result<(), String> = async {
                    let mut remote = sftp
                        .open(&entry.remote_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut local = tokio::fs::File::create(&temp_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        if cancel.is_cancelled() {
                            return Err("已取消".into());
                        }
                        let n = tokio::select! {
                            biased;
                            _ = cancel.cancelled() => return Err("已取消".into()),
                            result = remote.read(&mut buf) => result.map_err(|e| e.to_string())?,
                        };
                        if n == 0 {
                            break;
                        }
                        local
                            .write_all(&buf[..n])
                            .await
                            .map_err(|e| format!("写入本地文件失败: {e}"))?;
                        transferred += n as u64;
                        if throttle.should_emit() {
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
                    }
                    local.flush().await.map_err(|e| e.to_string())?;
                    local.shutdown().await.map_err(|e| e.to_string())?;
                    if cancel.is_cancelled() { return Err("已取消".into()); }
                    Ok(())
                }
                .await;
                if write_result.is_err() {
                    // 句柄随块退出已关闭；临时文件删除失败不掩盖原始错误
                    let _ = tokio::fs::remove_file(&temp_path).await;
                }
                write_result?;
                if let Err(error) = replace_local_file(&temp_path, entry.local_path.to_string_lossy().as_ref()).await {
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Err(error);
                }
            }
            Ok(())
        }
        .await;
        if cancel.is_cancelled() {
            log::info!("文件传输已取消 task={tid} session={event_connection_id}");
        } else if result.is_err() {
            log::error!("文件传输失败 task={tid} session={event_connection_id} code=sftp.transfer_failed");
        } else {
            log::info!("文件传输完成 task={tid} session={event_connection_id}");
        }
        let _ = app2.emit(
            "ssh://transfer-progress",
            &FileTransferProgress {
                transfer_id: tid.clone(),
                connection_id: event_connection_id.clone(),
                local_path: lpath.clone(),
                remote_path: rpath.clone(),
                transferred,
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
    }.await;
    match &result {
        Ok(_value) => log::info!(
            "传输请求已受理 operation=ssh_file_download_recursive elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=ssh_file_download_recursive elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
