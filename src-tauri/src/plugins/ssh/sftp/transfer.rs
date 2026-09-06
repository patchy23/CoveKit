//! SSH 插件 · 文件传输与操作（上传/下载/递归/删除/重命名/mkdir + 协作取消）
//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use russh_sftp::protocol::FileAttributes;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::plugins::ssh::conn::{get_session, get_sftp_session, resource_id, SshState};
use crate::plugins::ssh::models::{FileTransferProgress, RemoteFile, SshActionResult};

/// 递归上传前展开的单个本地文件或目录。
struct LocalUploadEntry {
    /// 本地绝对或用户选择路径。
    local_path: PathBuf,
    /// 对应的远程目标路径。
    remote_path: String,
    /// 是否为目录。
    is_dir: bool,
    /// 文件字节数；目录为 0。
    size: u64,
}

/// 展开本地上传目标；目录按父目录优先排列，符号链接不跟随，避免越出用户选择范围。
fn collect_upload_entries(
    local_path: &str,
    remote_path: &str,
) -> Result<Vec<LocalUploadEntry>, String> {
    let root = PathBuf::from(local_path);
    let root_meta = std::fs::symlink_metadata(&root).map_err(|e| format!("本地路径不可读: {e}"))?;
    if root_meta.file_type().is_symlink() {
        return Err("暂不支持上传符号链接".into());
    }
    if root_meta.is_file() {
        return Ok(vec![LocalUploadEntry {
            local_path: root,
            remote_path: remote_path.to_string(),
            is_dir: false,
            size: root_meta.len(),
        }]);
    }
    if !root_meta.is_dir() {
        return Err("仅支持上传普通文件或目录".into());
    }

    let mut entries = vec![LocalUploadEntry {
        local_path: root.clone(),
        remote_path: remote_path.to_string(),
        is_dir: true,
        size: 0,
    }];
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        let children = std::fs::read_dir(&directory)
            .map_err(|e| format!("无法读取目录 {}: {e}", directory.display()))?;
        for child in children {
            let child = child.map_err(|e| format!("读取目录项失败: {e}"))?;
            let path = child.path();
            let file_type = child
                .file_type()
                .map_err(|e| format!("读取 {} 类型失败: {e}", path.display()))?;
            if file_type.is_symlink() {
                continue;
            }
            let metadata = child
                .metadata()
                .map_err(|e| format!("读取 {} 元数据失败: {e}", path.display()))?;
            let relative = path
                .strip_prefix(&root)
                .map_err(|e| format!("计算相对路径失败: {e}"))?;
            let remote = join_remote_path(remote_path, relative);
            if metadata.is_dir() {
                entries.push(LocalUploadEntry {
                    local_path: path.clone(),
                    remote_path: remote,
                    is_dir: true,
                    size: 0,
                });
                pending.push(path);
            } else if metadata.is_file() {
                entries.push(LocalUploadEntry {
                    local_path: path,
                    remote_path: remote,
                    is_dir: false,
                    size: metadata.len(),
                });
            }
        }
    }
    Ok(entries)
}

/// 用 POSIX 分隔符把相对本地路径拼接到远程根路径。
fn join_remote_path(root: &str, relative: &Path) -> String {
    relative
        .components()
        .fold(root.trim_end_matches('/').to_string(), |mut path, part| {
            path.push('/');
            path.push_str(&part.as_os_str().to_string_lossy());
            path
        })
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
pub(crate) fn is_dir_mode(mode: u32) -> bool {
    (mode & 0o170000) == 0o040000
}

/// 文件属性 → 对外 RemoteFile（纯函数，可单测）
pub(crate) fn to_remote_file(name: String, path: String, meta: FileAttributes) -> RemoteFile {
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
fn register_cancel(state: &TransferState, transfer_id: &str) -> CancelFlag {
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut map) = state.0.lock() {
        map.insert(transfer_id.to_string(), flag.clone());
    }
    CancelFlag(flag)
}

/// 移除取消位（任务结束时调用；接收可克隆的注册表句柄）
fn unregister_cancel(
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
async fn collect_remote_entries(
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
