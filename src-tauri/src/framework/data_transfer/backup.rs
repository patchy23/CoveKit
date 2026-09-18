//! 框架 · 导入前快照（L3）：提交前把将被修改的存储复制到 `<设备根>/backups/import-<时间戳>/`
//!
//! 用途（方案 §6.1 第 3 步 / §6.4）：原地事务提交没有「上一代目录」可回退，用户级后悔
//! （导入了不想要的内容）由快照承接：合并/覆盖提交前先快照将被修改的存储，
//! 「还原到导入前」从最近一份快照恢复。
//!
//! 口径：
//! - 只快照**将被修改**的存储（调用方给相对路径清单），不整目录复制；
//! - 快照目录名带时间戳，最多保留最近 3 份（启动维护清理）；
//! - SQLite 经在线备份包含 WAL 已提交页；普通文件原样复制并原子写回；
//! - 快照在写冻结后拍摄，等待应用内在途数据库操作退出。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::framework::paths;
use crate::framework::space::index;

/// 快照目录集：`<设备根>/backups`
pub(crate) fn backups_dir(device_root: &Path) -> PathBuf {
    device_root.join("backups")
}

/// 快照清单名（快照目录内）
const MANIFEST_FILE: &str = "snapshot.json";

/// 快照保留份数（超出即由清理删除最旧的）
pub(crate) const KEEP_BACKUPS: usize = 3;

/// 快照清单（还原时要知道这些文件属于哪个空间）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    /// 被快照的空间 uid
    pub space_id: String,
    /// 拍摄时间（RFC3339）
    pub created_at: String,
    /// 相对空间根的文件清单（如 `data/ssh.db`、`preferences.json`）
    pub files: Vec<String>,
    /// 拍摄时不存在的文件，恢复时移除导入新增内容。
    #[serde(default)]
    pub absent: Vec<String>,
    /// 明确管理的目录；还原时删除快照后新增的文件。
    #[serde(default)]
    pub directories: Vec<String>,
}

/// 提交前快照：把将被修改的存储复制到新的快照目录。
///
/// 只复制存在的文件（首次导入时某些存储可能还没有）；返回快照目录。
pub(crate) fn snapshot_stores(
    device_root: &Path,
    space_id: &str,
    relative_files: &[&str],
) -> Result<PathBuf, String> {
    let space_root = index::space_root(device_root, space_id);
    // 目录名带时间戳便于人读，后缀短随机串保证同秒多次提交不互相覆盖
    let dir = backups_dir(device_root).join(format!(
        "import-{}-{}",
        paths::now_stamp(),
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建快照目录失败: {e}"))?;
    let mut copied = Vec::new();
    let mut absent = Vec::new();
    let mut directories = Vec::new();
    for relative in relative_files {
        let path = super::storage_files::resolve(&space_root, relative)?;
        if !path.exists() {
            absent.push((*relative).to_string());
            continue;
        }
        if path.is_dir() {
            directories.push((*relative).to_string());
        }
        for relative in super::storage_files::expand(&space_root, relative)? {
            let source = super::storage_files::resolve(&space_root, &relative)?;
            let target = super::storage_files::resolve(&dir, &relative)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建快照子目录失败: {e}"))?;
            }
            if super::storage_files::is_database(&source)? {
                super::storage_files::copy_database(&source, &target)?;
            } else {
                std::fs::copy(&source, &target).map_err(|e| format!("快照失败: {e}"))?;
            }
            copied.push(relative);
        }
    }
    let manifest = SnapshotManifest {
        space_id: space_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        files: copied,
        absent,
        directories,
    };
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("快照清单序列化失败: {e}"))?;
    std::fs::write(dir.join(MANIFEST_FILE), bytes).map_err(|e| format!("写快照清单失败: {e}"))?;
    Ok(dir)
}

/// 快照列表（新→旧；含清单读取失败的条目直接跳过——损坏的快照不能拿来还原）
pub(crate) fn list_snapshots(
    device_root: &Path,
) -> Result<Vec<(PathBuf, SnapshotManifest)>, String> {
    let dir = backups_dir(device_root);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("读取快照目录失败: {error}")),
    };
    let mut snapshots = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取快照目录项失败: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let bytes = match std::fs::read(path.join(MANIFEST_FILE)) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let Ok(manifest) = serde_json::from_slice::<SnapshotManifest>(&bytes) else {
            continue;
        };
        snapshots.push((path, manifest));
    }
    // 同秒拍摄的快照 created_at 相同，用目录名（时间戳 + 随机后缀）做决胜，保证顺序确定
    snapshots.sort_by(|a, b| {
        b.1.created_at
            .cmp(&a.1.created_at)
            .then_with(|| b.0.cmp(&a.0))
    });
    Ok(snapshots)
}

/// 清理旧快照：只保留最近 `KEEP_BACKUPS` 份（启动维护与每次拍摄后调用）
pub(crate) fn prune_snapshots(device_root: &Path) -> Result<usize, String> {
    let snapshots = list_snapshots(device_root)?;
    let mut removed = 0;
    for (path, _) in snapshots.iter().skip(KEEP_BACKUPS) {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("清理旧快照 {} 失败: {e}", path.display()))?;
        removed += 1;
    }
    Ok(removed)
}

/// 还原指定快照：把快照内容原样写回其空间（原子替换；目标空间目录必须存在）。
///
/// 还原的是「将被修改的存储」，其余文件不动；不存在的清单项跳过（拍摄时就不存在）。
pub(crate) fn restore_snapshot(device_root: &Path, snapshot_dir: &Path) -> Result<(), String> {
    let bytes = std::fs::read(snapshot_dir.join(MANIFEST_FILE))
        .map_err(|e| format!("快照清单读取失败: {e}"))?;
    let manifest: SnapshotManifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("快照清单解析失败: {e}"))?;
    uuid::Uuid::parse_str(&manifest.space_id).map_err(|_| "快照空间身份无效")?;
    let space_root = index::space_root(device_root, &manifest.space_id);
    if !space_root.is_dir() {
        return Err(format!(
            "快照所属空间 {} 已不存在，拒绝还原",
            manifest.space_id
        ));
    }
    // 先验证整个清单，避免发现后半段缺失时已还原前半段。
    for relative in &manifest.files {
        let source = super::storage_files::resolve(snapshot_dir, relative)?;
        super::storage_files::resolve(&space_root, relative)?;
        if !source.is_file() {
            return Err("快照内容缺失，拒绝还原".into());
        }
    }
    let mut remove = manifest.absent.clone();
    for directory in &manifest.directories {
        for current in super::storage_files::expand(&space_root, directory)? {
            if !manifest.files.contains(&current) {
                remove.push(current);
            }
        }
    }
    for relative in &remove {
        super::storage_files::resolve(&space_root, relative)?;
    }
    for relative in &manifest.files {
        let source = super::storage_files::resolve(snapshot_dir, relative)?;
        let target = super::storage_files::resolve(&space_root, relative)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if super::storage_files::is_database(&source)? {
            super::storage_files::copy_database(&source, &target)?;
        } else {
            crate::framework::secure_store::replace_file(
                &target,
                &std::fs::read(&source).map_err(|e| e.to_string())?,
            )?;
        }
    }
    for relative in remove {
        let target = super::storage_files::resolve(&space_root, &relative)?;
        if target.is_dir() {
            // 路径已逐段验证且只能位于目标空间，目录内容不跟随链接。
            for file in super::storage_files::expand(&space_root, &relative)? {
                std::fs::remove_file(super::storage_files::resolve(&space_root, &file)?)
                    .map_err(|e| e.to_string())?;
            }
        } else if target.exists() {
            std::fs::remove_file(target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("covekit-backup-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SPACE: &str = "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23";

    /// 快照只复制将被修改的存储，还原原样写回
    #[test]
    fn snapshot_and_restore_roundtrip() {
        let root = temp_root("roundtrip");
        let space = index::space_root(&root, SPACE);
        std::fs::create_dir_all(space.join("data")).unwrap();
        std::fs::write(space.join("data/ssh.db"), b"before").unwrap();
        std::fs::write(space.join("preferences.json"), b"{}").unwrap();
        std::fs::write(space.join("untouched.txt"), b"keep").unwrap();

        let snapshot = snapshot_stores(
            &root,
            SPACE,
            &["data/ssh.db", "preferences.json", "import-map.json"],
        )
        .expect("快照成功");
        // import-map.json 不存在则跳过（不出现在清单里）
        let manifest_bytes = std::fs::read(snapshot.join(MANIFEST_FILE)).unwrap();
        let manifest: SnapshotManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest.files, vec!["data/ssh.db", "preferences.json"]);
        assert!(!snapshot.join("import-map.json").exists());

        // 修改原文件后还原
        std::fs::write(space.join("data/ssh.db"), b"after").unwrap();
        restore_snapshot(&root, &snapshot).expect("还原成功");
        assert_eq!(std::fs::read(space.join("data/ssh.db")).unwrap(), b"before");
        assert_eq!(std::fs::read(space.join("untouched.txt")).unwrap(), b"keep");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn wal_snapshot_restores_live_connection_and_removes_new_files() {
        let root = temp_root("wal");
        let space = index::space_root(&root, SPACE);
        std::fs::create_dir_all(space.join("data")).unwrap();
        let db = rusqlite::Connection::open(space.join("data/live.db")).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE value(v TEXT); INSERT INTO value VALUES('before');").unwrap();
        let snapshot = snapshot_stores(&root, SPACE, &["data/live.db", "new.json"]).unwrap();
        db.execute("UPDATE value SET v='after'", []).unwrap();
        std::fs::write(space.join("new.json"), b"new").unwrap();
        restore_snapshot(&root, &snapshot).unwrap();
        let value: String = db
            .query_row("SELECT v FROM value", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "before");
        assert!(!space.join("new.json").exists());
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn snapshot_rejects_paths_outside_space() {
        let root = temp_root("traversal");
        assert!(snapshot_stores(&root, SPACE, &["../outside"]).is_err());
        assert!(snapshot_stores(&root, SPACE, &["C:/outside"]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 清理只留最近 3 份
    #[test]
    fn prune_keeps_only_recent_snapshots() {
        let root = temp_root("prune");
        let space = index::space_root(&root, SPACE);
        std::fs::create_dir_all(space.join("data")).unwrap();
        std::fs::write(space.join("data/ssh.db"), b"x").unwrap();
        for _ in 0..5 {
            let _ = snapshot_stores(&root, SPACE, &["data/ssh.db"]).expect("快照成功");
        }
        let before = list_snapshots(&root).unwrap();
        assert!(before.len() >= 4, "至少有四份快照: {}", before.len());
        // 清理后只剩 KEEP_BACKUPS 份
        let removed = prune_snapshots(&root).expect("清理成功");
        assert!(removed >= 1);
        let after = list_snapshots(&root).unwrap();
        assert!(after.len() <= KEEP_BACKUPS, "清理后至多 {KEEP_BACKUPS} 份");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 还原到不存在的空间：拒绝而不是静默
    #[test]
    fn restore_refuses_missing_space() {
        let root = temp_root("restore-missing");
        let space = index::space_root(&root, SPACE);
        std::fs::create_dir_all(space.join("data")).unwrap();
        std::fs::write(space.join("data/ssh.db"), b"x").unwrap();
        let snapshot = snapshot_stores(&root, SPACE, &["data/ssh.db"]).expect("快照成功");
        std::fs::remove_dir_all(&space).unwrap();
        let error = restore_snapshot(&root, &snapshot).expect_err("空间缺失必须拒绝");
        assert!(error.contains("已不存在"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
