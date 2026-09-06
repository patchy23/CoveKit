//! SSH 插件 · 文件浏览（远程目录列表 / 本地目录列表）
//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use std::path::Path;
use tauri::State;

use crate::plugins::ssh::conn::{get_sftp_session, invalidate_sftp_session, SshState};
use crate::plugins::ssh::models::{FileListResult, RemoteFile};

use super::transfer::to_remote_file;

/// 目录列表
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_file_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    path: String,
) -> Result<FileListResult, String> {
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    let entries = match sftp.read_dir(&path).await {
        Ok(entries) => entries,
        Err(e) => {
            // 长驻会话可能已失效（服务器重启/通道被回收）：清缓存，下次操作自动重建
            invalidate_sftp_session(&ssh_state, &connection_id);
            return Err(format!("读取目录失败: {e}"));
        }
    };
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

/// 本地目录列表（复用 FileListResult 结构；权限/所有者列不适用，填充占位值）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_list(path: String) -> Result<FileListResult, String> {
    let entries = std::fs::read_dir(&path).map_err(|e| format!("读取目录失败: {e}"))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue, // 系统文件可能拒绝访问，跳过不阻断
        };
        let full = entry.path().to_string_lossy().to_string();
        let name = entry.file_name().to_string_lossy().to_string();
        files.push(RemoteFile {
            name,
            path: full,
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified_at: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            permissions: "-".into(),
            owner: "-".into(),
            group: "-".into(),
        });
    }
    files.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let parent_path = Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| !p.is_empty());
    Ok(FileListResult {
        ok: true,
        path,
        parent_path,
        files,
        error: None,
    })
}

/* ── 远程新建目录 ── */
