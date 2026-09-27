//! SSH 插件 · 文件传输（上传/下载/递归下载 + 协作取消）
//! 传输走独立 SFTP 通道（不与交互浏览共用的长驻会话抢请求窗口）；
//! 分块传输，进度事件经节流后推送 ssh://transfer-progress；临时文件 + 原子替换落盘。

use russh_sftp::protocol::FileAttributes;

use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::ops::{
    collect_remote_entries, register_cancel, unregister_cancel, CancelFlag, TransferState,
};
use super::util::{collect_upload_entries, replace_local_file, replace_remote_file};
use crate::plugins::ssh::conn::{close_channel, get_session, resource_id, SshHandler, SshState};
use crate::plugins::ssh::models::FileTransferProgress;

/// 进度事件节流间隔：中间事件距上次不足此间隔则丢弃（64KB 块直发会在高速链路打爆 IPC）。
/// 首块必发；完成/失败事件不走节流，保证最终状态一定送达。
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

fn ensure_not_cancelled(cancel: &CancelFlag) -> Result<(), String> {
    if cancel.is_cancelled() {
        Err("已取消".into())
    } else {
        Ok(())
    }
}

async fn open_transfer_session(
    session: &russh::client::Handle<SshHandler>,
    cancel: &CancelFlag,
) -> Result<russh_sftp::client::SftpSession, String> {
    ensure_not_cancelled(cancel)?;
    // russh 确认前不公开 ChannelId；不能丢弃创建 future 后留下无人负责的通道。
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    let request = async {
        ensure_not_cancelled(cancel)?;
        channel
            .request_subsystem(false, "sftp")
            .await
            .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
        ensure_not_cancelled(cancel)
    }
    .await;
    if let Err(error) = request {
        close_channel(&channel).await;
        return Err(error);
    }
    // 初始化持有协议任务，保留到结果以避免依赖内部的流任务脱离收尾。
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| e.to_string())?;
    if let Err(error) = ensure_not_cancelled(cancel) {
        return close_transfer_session(&sftp, Err(error)).await;
    }
    Ok(sftp)
}

async fn close_transfer_session<T>(
    sftp: &russh_sftp::client::SftpSession,
    result: Result<T, String>,
) -> Result<T, String> {
    // 只关闭本任务的 SFTP 流，不断开共享 SSH 连接。依赖 close 仅提交关闭请求。
    match sftp.close().await {
        Ok(()) => result,
        Err(close) => match result {
            Ok(value) => {
                log::warn!("文件已保存，但 SFTP 关闭请求失败 code=sftp.close_failed");
                Ok(value)
            }
            Err(error) => Err(format!("{error}；关闭 SFTP 会话失败: {close}")),
        },
    }
}

async fn cleanup_local_temp(path: &str, error: String) -> String {
    match tokio::fs::remove_file(path).await {
        Ok(()) => error,
        Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup) => format!("{error}；清理本地临时文件失败: {cleanup}"),
    }
}

async fn finish_local_write<T>(
    mut local: tokio::fs::File,
    result: Result<T, String>,
) -> Result<T, String> {
    // tokio 文件写入可仍由阻塞任务持有句柄；取消也先等 flush，再释放并删除临时文件。
    let flushed = local.flush().await;
    drop(local);
    match flushed {
        Ok(()) => result,
        Err(flush) => Err(match result {
            Ok(_) => format!("刷新本地临时文件失败: {flush}"),
            Err(error) => format!("{error}；刷新本地临时文件失败: {flush}"),
        }),
    }
}

async fn cleanup_remote_temp(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
    error: String,
) -> String {
    match sftp.remove_file(path).await {
        Ok(()) => error,
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == russh_sftp::protocol::StatusCode::NoSuchFile =>
        {
            error
        }
        Err(cleanup) => format!("{error}；清理远程临时文件失败: {cleanup}"),
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_local_write_is_drained_before_removal_and_keeps_existing_target() {
        let root = std::env::temp_dir().join(resource_id("sftp-cancel-test"));
        tokio::fs::create_dir(&root).await.unwrap();
        let target = root.join("target");
        let temporary = root.join("temporary");
        tokio::fs::write(&target, b"old").await.unwrap();
        let mut local = tokio::fs::File::create(&temporary).await.unwrap();
        local.write_all(&vec![3; 128 * 1024]).await.unwrap();
        let result = finish_local_write::<()>(local, Err("已取消".into())).await;
        assert_eq!(result, Err("已取消".into()));
        assert_eq!(tokio::fs::metadata(&temporary).await.unwrap().len(), 128 * 1024);
        assert_eq!(
            cleanup_local_temp(temporary.to_str().unwrap(), result.unwrap_err()).await,
            "已取消"
        );
        assert!(!tokio::fs::try_exists(&temporary).await.unwrap());
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"old");
        tokio::fs::remove_dir_all(&root).await.unwrap();
    }

    #[tokio::test]
    async fn cleanup_reports_failure_but_missing_temp_preserves_original_error() {
        let root = std::env::temp_dir().join(resource_id("sftp-cleanup-test"));
        tokio::fs::create_dir(&root).await.unwrap();
        let error = cleanup_local_temp(root.to_str().unwrap(), "已取消".into()).await;
        assert!(error.starts_with("已取消；清理本地临时文件失败:"));
        tokio::fs::remove_dir(&root).await.unwrap();
        assert_eq!(
            cleanup_local_temp(root.to_str().unwrap(), "已取消".into()).await,
            "已取消"
        );
    }
}

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
    // 失效连接不启动大目录扫描；扫描后仍重新取句柄，以使用当前连接代次。
    get_session(&ssh_state, &connection_id)?;
    // 先返回任务标识，目录扫描也由后台任务负责，用户可在准备阶段取消。
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
        let mut total: u64 = 0;
        let mut throttle = ProgressThrottle::new();
        let result: Result<(), String> = async {
            let (scan_local, scan_remote) = (lpath.clone(), rpath.clone());
            let scan_cancel = cancel.clone();
            let entries = tokio::task::spawn_blocking(move || {
                collect_upload_entries(&scan_local, &scan_remote, || scan_cancel.is_cancelled())
            })
            .await
            .map_err(|e| format!("遍历本地目录任务失败: {e}"))??;
            if cancel.is_cancelled() {
                return Err("已取消".into());
            }
            total = entries.total_size();
            let session = get_session(&app2.state::<SshState>(), &event_connection_id)?;
            let sftp = open_transfer_session(&session, &cancel).await?;
            let transfer_result = async {
            // 同一任务逐文件传输，缓冲复用；只发送本轮读到的前 n 字节。
            let mut buf = vec![0u8; 64 * 1024];
            for entry in entries.into_entries() {
                if cancel.is_cancelled() {
                    return Err("已取消".into());
                }
                let target_exists = cancel
                    .wait(async {
                        sftp.try_exists(&entry.remote_path)
                            .await
                            .map_err(|e| format!("检查远程目标失败: {e}"))
                    })
                    .await?;
                if entry.is_dir {
                    if target_exists {
                        let metadata = cancel
                            .wait(async {
                                sftp.metadata(&entry.remote_path)
                                    .await
                                    .map_err(|e| format!("读取远程目录元数据失败: {e}"))
                            })
                            .await?;
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
                    cancel
                        .wait(async {
                            sftp.canonicalize(&entry.remote_path)
                                .await
                                .map_err(|e| format!("解析远程目标失败: {e}"))
                        })
                        .await?
                } else {
                    entry.remote_path.clone()
                };
                let target_permissions = if target_exists {
                    cancel
                        .wait(async {
                            sftp.metadata(&target_path)
                                .await
                                .map_err(|e| format!("读取远程目标权限失败: {e}"))
                        })
                        .await?
                        .permissions
                } else {
                    None
                };
                let temp_path = format!("{target_path}.covekit-upload-{}", resource_id("file"));
                // 写入阶段任一失败（含取消）都要清理远程临时文件，避免 .covekit-upload-* 残留
                let mut remote = match sftp.create(&temp_path).await {
                    Ok(remote) => remote,
                    Err(error) => {
                        return Err(cleanup_remote_temp(
                            &sftp,
                            &temp_path,
                            format!("创建远程文件失败: {error}"),
                        ).await);
                    }
                };
                let write_result: Result<(), String> = async {
                    ensure_not_cancelled(&cancel)?;
                    let mut local = tokio::fs::File::open(&entry.local_path)
                        .await
                        .map_err(|e| e.to_string())?;
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
                    ensure_not_cancelled(&cancel)?;
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
                // 不丢弃在途写入；每个句柄只关闭一次，排空 ACK 后才删除或提交。
                // 依赖的写 ACK 没有请求超时，取消仍须等待该安全边界。
                let write_result = match remote.shutdown().await {
                    Ok(()) => write_result,
                    Err(close) => Err(match write_result {
                        Ok(()) => format!("关闭远程临时文件失败: {close}"),
                        Err(error) => format!("{error}；关闭远程临时文件失败: {close}"),
                    }),
                };
                drop(remote);
                if let Err(error) = write_result {
                    return Err(cleanup_remote_temp(&sftp, &temp_path, error).await);
                }
                if let Err(error) = ensure_not_cancelled(&cancel) {
                    return Err(cleanup_remote_temp(&sftp, &temp_path, error).await);
                }
                // 备份/替换必须执行到成功或恢复完成，不能丢弃提交 future。
                replace_remote_file(&sftp, &temp_path, &target_path).await?;
            }
            Ok(())
            }.await;
            close_transfer_session(&sftp, transfer_result).await
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
        total: 0,
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
            let sftp = open_transfer_session(&session, &cancel).await?;
            let transfer_result = async {
            let meta = cancel
                .wait(async {
                    sftp.metadata(&rpath)
                        .await
                        .map_err(|e| format!("读取元数据失败: {e}"))
                })
                .await?;
            // russh-sftp size 本就是 Option<u64>，缺省 0 表示服务端未报（进度条按不定长展示）
            let total = meta.size.unwrap_or(0);
            let mut remote = sftp.open(&rpath).await.map_err(|e| e.to_string())?;
            let mut local = tokio::fs::File::create(&temp_path)
                .await
                .map_err(|e| e.to_string())?;
            let write_result = async {
            let mut buf = vec![0u8; 64 * 1024];
            let mut transferred: u64 = 0;
            loop {
                if cancel.is_cancelled() {
                    // 先退出读取，再排空本地写入并释放句柄，最后统一删除临时文件
                    return Err("已取消".into());
                }
                let n = cancel.wait(remote.read(&mut buf)).await?;
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
            Ok(transferred)
            }.await;
            let transferred = finish_local_write(local, write_result).await?;
            ensure_not_cancelled(&cancel)?;
            replace_local_file(&temp_path, &lpath).await?;
            Ok(transferred)
            }.await;
            close_transfer_session(&sftp, transfer_result).await
        }
        .await;
        let result = match result {
            Ok(written) => Ok(written),
            Err(error) => Err(cleanup_local_temp(&temp_path, error).await),
        };
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
            let sftp = open_transfer_session(&session, &cancel).await?;
            let transfer_result = async {
            // 扫描取消后整个独立会话退出，迟到的目录句柄不被后续操作复用。
            let entries = cancel
                .wait(collect_remote_entries(&sftp, &rpath, Path::new(&lpath)))
                .await?;
            total = entries.total_size();
            let mut buf = vec![0u8; 64 * 1024];
            for entry in entries.into_entries() {
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
                    let copy_result = async {
                    loop {
                        if cancel.is_cancelled() {
                            return Err("已取消".into());
                        }
                        let n = cancel.wait(remote.read(&mut buf)).await?;
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
                    Ok(())
                    }.await;
                    finish_local_write(local, copy_result).await?;
                    ensure_not_cancelled(&cancel)?;
                    Ok(())
                }
                .await;
                if let Err(error) = write_result {
                    return Err(cleanup_local_temp(&temp_path, error).await);
                }
                if let Err(error) = replace_local_file(&temp_path, entry.local_path.to_string_lossy().as_ref()).await {
                    return Err(cleanup_local_temp(&temp_path, error).await);
                }
            }
            Ok(())
            }.await;
            close_transfer_session(&sftp, transfer_result).await
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
