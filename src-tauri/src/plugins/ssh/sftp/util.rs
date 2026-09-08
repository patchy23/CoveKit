//! SSH 插件 · 文件传输与操作（上传/下载/递归/删除/重命名/mkdir + 协作取消）

//! 每次操作临时开 SFTP 通道（从连接会话），无需注册表；
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送。

use russh_sftp::protocol::FileAttributes;

use std::path::{Path, PathBuf};

use crate::plugins::ssh::conn::resource_id;
use crate::plugins::ssh::models::RemoteFile;

/// 递归上传前展开的单个本地文件或目录。
pub(crate) struct LocalUploadEntry {
    /// 本地绝对或用户选择路径。
    pub(crate) local_path: PathBuf,
    /// 对应的远程目标路径。
    pub(crate) remote_path: String,
    /// 是否为目录。
    pub(crate) is_dir: bool,
    /// 文件字节数；目录为 0。
    pub(crate) size: u64,
}

/// 展开本地上传目标；目录按父目录优先排列，符号链接不跟随，避免越出用户选择范围。
pub(crate) fn collect_upload_entries(
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
pub(crate) fn replace_local_file(temp_path: &str, target_path: &str) -> Result<(), String> {
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

/// 权限位转 rwx 字符串（如 drwxr-xr-x；mode 高 4 位是文件类型）
/// 参考 ls -l 输出格式，比八进制可读（用户反馈 40755 看不懂）
pub(crate) fn format_permissions(mode: u32) -> String {
    // 文件类型位（S_IFMT）
    let kind = match mode & 0o170000 {
        0o040000 => 'd', // 目录
        0o120000 => 'l', // 符号链接
        0o010000 => 'p', // 管道
        0o060000 => 'b', // 块设备
        0o020000 => 'c', // 字符设备
        0o140000 => 's', // socket
        _ => '-',        // 普通文件
    };
    // 9 个权限位：rwxrwxrwx
    let mut out = String::with_capacity(10);
    out.push(kind);
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
}

/// 文件属性 → 对外 RemoteFile（纯函数，可单测）
pub(crate) fn to_remote_file(name: String, path: String, meta: FileAttributes) -> RemoteFile {
    let size = meta.size.unwrap_or(0);
    let is_dir = meta.permissions.map(is_dir_mode).unwrap_or(false);
    // owner/group：SFTP 线协议通常只给 uid/gid 数字，longname 解析才有名字——两级回退
    let owner = meta
        .user
        .clone()
        .or_else(|| meta.uid.map(|u| u.to_string()))
        .unwrap_or_else(|| "-".into());
    let group = meta
        .group
        .clone()
        .or_else(|| meta.gid.map(|g| g.to_string()))
        .unwrap_or_else(|| "-".into());
    RemoteFile {
        name,
        path,
        is_dir,
        size,
        modified_at: meta.mtime.unwrap_or(0) as u64 * 1000,
        permissions: meta
            .permissions
            .map(format_permissions)
            .unwrap_or_else(|| "-".into()),
        owner,
        group,
    }
}

#[cfg(test)]
mod tests {
    use super::format_permissions;

    #[test]
    fn 权限位转rwx字符串() {
        assert_eq!(format_permissions(0o040755), "drwxr-xr-x");
        assert_eq!(format_permissions(0o100644), "-rw-r--r--");
        assert_eq!(format_permissions(0o100600), "-rw-------");
        assert_eq!(format_permissions(0o120777), "lrwxrwxrwx");
    }
}

/* ── 危险路径安全策略（删除/chmod 共用；后端是最后防线，前端只藏菜单项） ── */

/// 系统目录清单（本体及子树受保护）
const SYSTEM_DIRS: [&str; 13] = [
    "/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/run", "/sbin", "/sys",
    "/usr", "/var",
];

/// 虚拟文件系统（chmod 无意义且可能报错，整树禁止）
const VIRTUAL_DIRS: [&str; 3] = ["/proc", "/sys", "/dev"];

/// 规范化远程路径：去尾部斜杠（根 "/" 除外）
fn normalize_remote_path(path: &str) -> &str {
    if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    }
}

/// path 是否为系统目录本体（恰好等于清单项）
fn is_system_dir_itself(path: &str) -> bool {
    SYSTEM_DIRS.contains(&path)
}

/// path 是否位于系统目录子树内
fn under_system_dir(path: &str) -> bool {
    SYSTEM_DIRS
        .iter()
        .filter(|d| **d != "/")
        .any(|d| path.starts_with(&format!("{d}/")))
}

/// 删除安全校验：系统目录本体及子树一律禁止；根下自定义目录（/mydata 等）允许。
/// 根目录 "/" 本身禁止（normalize 后 "/" 在 SYSTEM_DIRS 中直接命中）。
pub(crate) fn check_delete_allowed(path: &str) -> Result<(), String> {
    let p = normalize_remote_path(path);
    if is_system_dir_itself(p) || under_system_dir(p) {
        return Err(format!("系统路径 {p} 禁止删除（防止误删系统文件）"));
    }
    Ok(())
}

/// chmod 安全校验：
/// - 系统目录本体禁止（不能 chmod /etc 本身）
/// - /proc /sys /dev 虚拟文件系统整树禁止
/// - 系统目录内递归修改需 acknowledge_risk（前端弹窗红字勾选「我知道风险」）
/// - 系统目录内部文件的非递归修改允许（/sbin/sshd 加执行权限是正常运维）
pub(crate) fn check_chmod_allowed(
    path: &str,
    recursive: bool,
    acknowledge_risk: bool,
) -> Result<(), String> {
    let p = normalize_remote_path(path);
    if is_system_dir_itself(p) {
        return Err(format!("系统目录 {p} 本体禁止修改权限"));
    }
    if VIRTUAL_DIRS.iter().any(|d| p.starts_with(&format!("{d}/"))) {
        return Err(format!("虚拟文件系统 {p} 不支持修改权限"));
    }
    if recursive && under_system_dir(p) && !acknowledge_risk {
        return Err(format!("{p} 位于系统目录内，递归修改权限需确认风险"));
    }
    Ok(())
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn 删除拦截系统路径() {
        assert!(check_delete_allowed("/").is_err());
        assert!(check_delete_allowed("/etc").is_err());
        assert!(check_delete_allowed("/etc/nginx/nginx.conf").is_err());
        assert!(check_delete_allowed("/sbin/sshd").is_err());
    }

    #[test]
    fn 删除放行自定义路径() {
        assert!(check_delete_allowed("/mydata").is_ok());
        assert!(check_delete_allowed("/data/logs/app.log").is_ok());
        assert!(check_delete_allowed("/home/user/tmp").is_ok());
    }

    #[test]
    fn chmod拦截目录本体与虚拟文件系统() {
        assert!(check_chmod_allowed("/etc", false, false).is_err());
        assert!(check_chmod_allowed("/", false, false).is_err());
        assert!(check_chmod_allowed("/proc/1/status", false, false).is_err());
    }

    #[test]
    fn chmod放行系统目录内文件与递归风险确认() {
        assert!(check_chmod_allowed("/sbin/sshd", false, false).is_ok());
        assert!(check_chmod_allowed("/etc/nginx/nginx.conf", false, false).is_ok());
        // 递归在系统目录内需确认风险
        assert!(check_chmod_allowed("/var/www", true, false).is_err());
        assert!(check_chmod_allowed("/var/www", true, true).is_ok());
    }
}
