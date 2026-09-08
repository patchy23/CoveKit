//! SSH 插件 · 文件浏览（远程目录列表 / 本地目录列表）
//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use std::path::Path;
use tauri::State;

use crate::plugins::ssh::conn::{get_sftp_session, invalidate_sftp_session, SshState};
use crate::plugins::ssh::models::{FileListResult, RemoteFile, SshActionResult};

use super::util::to_remote_file;
use crate::plugins::ssh::conn::resolve_id_names;

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
    // uid/gid → 名称回填（线协议只带数字；解析结果每连接缓存，失败回退数字展示）
    let names = resolve_id_names(&ssh_state, &connection_id).await;
    if !names.users.is_empty() || !names.groups.is_empty() {
        for f in &mut files {
            if let Some(name) = f
                .owner
                .parse::<u32>()
                .ok()
                .and_then(|id| names.users.get(&id))
            {
                f.owner = name.clone();
            }
            if let Some(name) = f
                .group
                .parse::<u32>()
                .ok()
                .and_then(|id| names.groups.get(&id))
            {
                f.group = name.clone();
            }
        }
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
/// path 为空 / "/" / "\\" 时返回驱动器列表——「此电脑」是 Shell 命名空间式的虚拟层，
/// 由文件系统抽象自身处理（WinSCP/FileZilla 本地侧同款），前端不做字符串手术。
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_list(path: String) -> Result<FileListResult, String> {
    // 规范化：分隔符统一；盘符 'C:' 补尾斜杠（裸盘符是「该盘当前目录」而非根）
    let normalized = normalize_local_path(&path);
    // 虚拟根：返回驱动器列表
    if normalized.is_empty() {
        return Ok(local_drives_result());
    }
    let entries =
        std::fs::read_dir(&normalized).map_err(|e| format!("读取目录失败 [{normalized}]: {e}"))?;
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
    // 盘符根的上级 = 驱动器层（空串哨兵）；其余走 Path::parent
    let parent_path = if is_drive_root(&normalized) {
        Some(String::new())
    } else {
        Path::new(&normalized)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty())
    };
    Ok(FileListResult {
        ok: true,
        path: normalized,
        parent_path,
        files,
        error: None,
    })
}

/* ── 远程新建目录 ── */

/// 是否盘符根（'C:\\'）
fn is_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3 && bytes[1] == b':' && bytes[2] == b'\\' && bytes[0].is_ascii_alphabetic()
}

/// 驱动器列表的 FileListResult（「此电脑」虚拟层；Windows 枚举存在的盘符，其余平台给根目录）
fn local_drives_result() -> FileListResult {
    let mut drives = Vec::new();
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        if std::path::Path::new(&root).exists() {
            drives.push(RemoteFile {
                name: format!("{}:", letter as char),
                path: root,
                is_dir: true,
                size: 0,
                modified_at: 0,
                permissions: "-".into(),
                owner: "-".into(),
                group: "-".into(),
            });
        }
    }
    #[cfg(not(windows))]
    drives.push(RemoteFile {
        name: "/".into(),
        path: "/".into(),
        is_dir: true,
        size: 0,
        modified_at: 0,
        permissions: "-".into(),
        owner: "-".into(),
        group: "-".into(),
    });
    FileListResult {
        ok: true,
        path: String::new(),
        parent_path: None,
        files: drives,
        error: None,
    }
}

/// 本地路径规范化（纯函数，可单测）：分隔符统一反斜杠；裸分隔符/空串 → 空（驱动器层）；盘符补尾斜杠
fn normalize_local_path(path: &str) -> String {
    let mut normalized = path.replace('/', "\\");
    if normalized == "\\" {
        normalized.clear();
    }
    let bytes = normalized.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        normalized.push('\\');
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 裸分隔符与空串归一到驱动器层() {
        assert_eq!(normalize_local_path(""), "");
        assert_eq!(normalize_local_path("/"), "");
        assert_eq!(normalize_local_path("\\"), "");
    }

    #[test]
    fn 盘符补尾斜杠() {
        assert_eq!(normalize_local_path("C:"), "C:\\");
        assert_eq!(normalize_local_path("d:"), "d:\\");
        assert_eq!(normalize_local_path("D:/"), "D:\\");
    }

    #[test]
    fn 普通路径分隔符统一() {
        assert_eq!(normalize_local_path("D:/Tools/xx"), r"D:\Tools\xx");
        assert_eq!(normalize_local_path(r"C:\Windows"), r"C:\Windows");
    }
}

/// 本地新建文件/目录（双栏本地侧；已存在即拒绝防覆盖）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_create(path: String, is_dir: bool) -> SshActionResult {
    let p = std::path::Path::new(&path);
    if p.exists() {
        return SshActionResult {
            ok: false,
            error: Some("目标已存在".into()),
        };
    }
    let result = if is_dir {
        std::fs::create_dir(p).map_err(|e| e.to_string())
    } else {
        std::fs::File::create(p)
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    match result {
        Ok(_) => SshActionResult {
            ok: true,
            error: None,
        },
        Err(e) => SshActionResult {
            ok: false,
            error: Some(format!("新建失败 [{path}]: {e}")),
        },
    }
}

/// 本地删除文件/目录（双栏本地侧；目录递归删除，前端确认弹窗已写明不可恢复）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_delete(path: String, is_dir: bool) -> SshActionResult {
    let p = std::path::Path::new(&path);
    let result = if is_dir {
        std::fs::remove_dir_all(p).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(p).map_err(|e| e.to_string())
    };
    match result {
        Ok(_) => SshActionResult {
            ok: true,
            error: None,
        },
        Err(e) => SshActionResult {
            ok: false,
            error: Some(format!("删除失败 [{path}]: {e}")),
        },
    }
}

/// 本地重命名/移动（双栏本地侧）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_rename(old_path: String, new_path: String) -> SshActionResult {
    match std::fs::rename(&old_path, &new_path) {
        Ok(_) => SshActionResult {
            ok: true,
            error: None,
        },
        Err(e) => SshActionResult {
            ok: false,
            error: Some(format!("重命名失败 [{old_path}]: {e}")),
        },
    }
}
