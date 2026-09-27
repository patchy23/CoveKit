//! 临时数据实例：目录创建和回收由目录锁串行化，存活锁保持到 owner 释放实例。
//! 此目录只能存放可丢弃临时文件；已发布结果必须移动到 owner 的持久输出目录。

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// 持有操作系统文件锁；进程异常结束也会释放，不依赖 PID 或文件时间。
pub(crate) struct TempInstance {
    directory: PathBuf,
    _alive: File,
}

impl TempInstance {
    /// 同步文件操作必须由启动阶段或 spawn_blocking 调用。
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| format!("创建临时实例目录失败: {e}"))?;
        crate::framework::secure_store::restrict_private_directory(root)?;
        let registry = OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .open(root.join("registry.lock")).map_err(|e| format!("打开临时目录锁失败: {e}"))?;
        registry.lock().map_err(|e| format!("获取临时目录锁失败: {e}"))?;
        // 创建与清理持有同一目录锁，不能在新实例创建锁文件但尚未锁住时将它误删。
        reap_locked(root)?;
        let directory = root.join(format!("instance-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).map_err(|e| format!("创建临时实例失败: {e}"))?;
        crate::framework::secure_store::restrict_private_directory(&directory)?;
        let alive = File::create_new(directory.join("alive.lock"))
            .map_err(|e| format!("创建实例存活锁失败: {e}"))?;
        alive.try_lock().map_err(|e| format!("锁定临时实例失败: {e}"))?;
        Ok(Self { directory, _alive: alive })
    }

    /// 仅交给创建该实例的 owner，不将路径作为跨 owner 的读取契约。
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

/// 调用方持有 registry.lock；只删除拿到独占存活锁的实例，链接和未知目录不跟随。
fn reap_locked(root: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|e| format!("枚举临时实例失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取临时实例失败: {e}"))?;
        let kind = entry.file_type().map_err(|e| format!("读取临时实例类型失败: {e}"))?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|name| name.strip_prefix("instance-")) else { continue; };
        if !kind.is_dir() || kind.is_symlink() || uuid::Uuid::parse_str(id).is_err() { continue; }
        let directory = entry.path();
        let lock_path = directory.join("alive.lock");
        let metadata = match std::fs::symlink_metadata(&lock_path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("读取实例存活锁失败: {error}")),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() { continue; }
        let alive = OpenOptions::new().read(true).write(true).open(&lock_path)
            .map_err(|e| format!("打开实例存活锁失败: {e}"))?;
        match alive.try_lock() {
            Err(TryLockError::WouldBlock) => continue,
            Err(TryLockError::Error(error)) => return Err(format!("检查实例存活状态失败: {error}")),
            Ok(()) => {}
        }
        for file in std::fs::read_dir(&directory).map_err(|e| format!("枚举失活实例失败: {e}"))? {
            let file = file.map_err(|e| format!("读取失活实例文件失败: {e}"))?;
            if file.file_name() == "alive.lock" { continue; }
            let kind = file.file_type().map_err(|e| format!("读取临时文件类型失败: {e}"))?;
            if !kind.is_file() || kind.is_symlink() {
                return Err("失活临时实例含未知目录或链接，已保留现场".into());
            }
            std::fs::remove_file(file.path()).map_err(|e| format!("回收失活临时文件失败: {e}"))?;
        }
        // Windows 删除锁文件前关闭句柄；目录锁仍持有，其他回收者不能进入这个间隙。
        drop(alive);
        std::fs::remove_file(lock_path).map_err(|e| format!("清理失活实例锁失败: {e}"))?;
        std::fs::remove_dir(directory).map_err(|e| format!("清理失活实例目录失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unlocked_instances_are_reclaimed_and_published_files_are_untouched() {
        let root = std::env::temp_dir().join(format!("covekit-instance-{}", uuid::Uuid::new_v4()));
        let first = TempInstance::open(&root).unwrap();
        let first_path = first.directory().to_path_buf();
        std::fs::write(first_path.join("audio.part"), b"active").unwrap();
        std::fs::write(root.join("published.mp3"), b"published").unwrap();
        let second = TempInstance::open(&root).unwrap();
        assert_eq!(std::fs::read(first_path.join("audio.part")).unwrap(), b"active");
        drop(first);
        let third = TempInstance::open(&root).unwrap();
        assert!(!first_path.exists());
        assert!(second.directory().exists());
        assert_eq!(std::fs::read(root.join("published.mp3")).unwrap(), b"published");
        drop(second);
        drop(third);
        std::fs::remove_dir_all(root).unwrap();
    }
}
