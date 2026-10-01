//! agent 驱动 store：目录布局 + versions.json + 本地查找
//! 布局：缓存分区 agents/drivers/<key>/，只接受兼容协议的 agent 可执行文件。
//! 获取策略（与 dbx 一致但无物理依赖）：
//!   1. 本地已有驱动二进制 → 直接使用
//!   2. Oracle 缺失时由 database 插件的独立安装 IPC 准备固定版本 agent
//!   3. 其它 agent 类型缺失时仍返回带指引的错误（给出该放的目录）

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::plugins::database::agent::driver_key;
use crate::plugins::database::models::DbType;

/// 驱动 store 根目录下的版本清单文件名
const VERSIONS_FILE: &str = "versions.json";
const MAX_VERSIONS_BYTES: u64 = 1024 * 1024;

/// agent 驱动 store（路径解析与本地版本清单）
#[derive(Clone)]
pub struct DriverStore {
    /// store 根目录（缓存分区 agents/drivers）
    root: PathBuf,
}

impl DriverStore {
    /// 从缓存分区构造驱动 store（`<root>/cache/agents/drivers`）
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let root = crate::framework::paths::cache_dir(app, "agents")?.join("drivers");
        Ok(Self { root })
    }

    /// 某类型驱动的目录（不存在时创建）
    pub fn driver_dir(&self, db_type: DbType) -> Result<PathBuf, String> {
        let dir = self.root.join(driver_key(db_type));
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建驱动目录失败: {e}"))?;
        Ok(dir)
    }

    /// 返回平台 agent 的发布目标路径。
    pub fn target_binary_path(&self, db_type: DbType) -> Result<PathBuf, String> {
        let dir = self.driver_dir(db_type)?;
        Ok(dir.join(if cfg!(windows) { "agent.exe" } else { "agent" }))
    }

    /// 驱动二进制路径：只查找 `agent(.exe)`；JAR 不是可直接启动的进程。
    pub fn agent_binary(&self, db_type: DbType) -> Option<PathBuf> {
        let dir = self.root.join(driver_key(db_type));
        let exe = if cfg!(windows) {
            dir.join("agent.exe")
        } else {
            dir.join("agent")
        };
        if is_regular_file(&exe) {
            return Some(exe);
        }
        let plain = dir.join("agent");
        if is_regular_file(&plain) {
            return Some(plain);
        }
        None
    }

    /// 读取 versions.json；仅文件缺失表示尚无版本记录。
    pub fn versions(&self) -> Result<HashMap<String, String>, String> {
        let path = self.root.join(VERSIONS_FILE);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if metadata.len() > MAX_VERSIONS_BYTES {
                    return Err("驱动版本清单超过 1 MiB 上限".into());
                }
            }
            Ok(_) => return Err("驱动版本清单不是普通文件".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(HashMap::new())
            }
            Err(error) => return Err(format!("检查驱动版本清单失败: {error}")),
        }
        let text = fs::read_to_string(path)
            .map_err(|error| format!("读取驱动版本清单失败: {error}"))?;
        serde_json::from_str(&text).map_err(|error| format!("驱动版本清单损坏: {error}"))
    }

    /// 安全更新一个版本记录；损坏的旧清单必须显式报错，不能用空表覆盖。
    pub fn record_version(&self, key: &str, version: &str) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|error| format!("创建驱动清单目录失败: {error}"))?;
        let path = self.root.join(VERSIONS_FILE);
        let mut versions = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if metadata.len() > MAX_VERSIONS_BYTES {
                    return Err("驱动版本清单超过 1 MiB 上限，拒绝覆盖".into());
                }
                let text = fs::read_to_string(&path)
                    .map_err(|error| format!("读取驱动版本清单失败: {error}"))?;
                serde_json::from_str::<HashMap<String, String>>(&text)
                    .map_err(|error| format!("驱动版本清单损坏，未覆盖原文件: {error}"))?
            }
            Ok(_) => return Err("驱动版本清单不是普通文件，拒绝覆盖".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(error) => return Err(format!("检查驱动版本清单失败: {error}")),
        };
        versions.insert(key.to_string(), version.to_string());
        let bytes = serde_json::to_vec_pretty(&versions)
            .map_err(|error| format!("序列化驱动版本清单失败: {error}"))?;
        let temporary = self
            .root
            .join(format!(".versions-{}.json", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("创建临时驱动版本清单失败: {error}"))?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let primary = format!("写入临时驱动版本清单失败: {error}");
            return Err(cleanup_temporary(&temporary, primary));
        }
        drop(file);

        let backup = self
            .root
            .join(format!(".versions-backup-{}.json", uuid::Uuid::new_v4()));
        let had_previous = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if let Err(error) = fs::rename(&path, &backup) {
                    return Err(cleanup_temporary(
                        &temporary,
                        format!("暂存旧驱动版本清单失败: {error}"),
                    ));
                }
                true
            }
            Ok(_) => {
                return Err(cleanup_temporary(
                    &temporary,
                    "驱动版本清单不是普通文件，拒绝覆盖".into(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(cleanup_temporary(
                    &temporary,
                    format!("检查驱动版本清单失败: {error}"),
                ));
            }
        };
        if let Err(error) = fs::rename(&temporary, &path) {
            let primary = format!("发布驱动版本清单失败: {error}");
            let restore = if had_previous {
                fs::rename(&backup, &path)
                    .map_err(|error| format!("恢复旧驱动版本清单失败: {error}"))
                    .err()
            } else {
                None
            };
            let cleanup = remove_temporary(&temporary).err();
            return Err([Some(primary), restore, cleanup]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("；"));
        }
        if had_previous {
            if let Err(error) = fs::remove_file(&backup) {
                log::warn!("旧驱动版本清单备份暂未清理 kind={:?}", error.kind());
            }
        }
        Ok(())
    }

    /// 确保某类型驱动可用：本地二进制 → 缺失时给出放置指引
    pub fn ensure_driver(&self, db_type: DbType) -> Result<PathBuf, String> {
        // 达梦尚无运行时契约，新建连接不展示该类型。
        if matches!(db_type, DbType::Dameng) {
            return Err("达梦驱动暂未支持（本版本未实现，后续版本提供）。".to_string());
        }
        if let Some(binary) = self.agent_binary(db_type) {
            return Ok(binary);
        }
        Err(format!(
            "缺少 {} 驱动。请将 agent 可执行文件放入 {}。",
            driver_key(db_type),
            self.root.join(driver_key(db_type)).display()
        ))
    }
}

fn is_regular_file(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Ok(metadata) if metadata.file_type().is_file())
}

fn remove_temporary(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清理临时驱动版本清单失败: {error}")),
    }
}

fn cleanup_temporary(path: &Path, cause: String) -> String {
    match remove_temporary(path) {
        Ok(()) => cause,
        Err(cleanup) => format!("{cause}；{cleanup}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_dir_named_by_type() {
        assert_eq!(driver_key(DbType::Oracle), "oracle");
        assert_eq!(driver_key(DbType::Kingbase), "kingbase");
        assert_eq!(driver_key(DbType::Vastbase), "vastbase");
        assert_eq!(driver_key(DbType::Dameng), "dameng");
    }

    #[test]
    fn damaged_versions_file_is_never_treated_as_an_empty_store() {
        let root = std::env::temp_dir().join(format!("driver-store-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join(VERSIONS_FILE);
        fs::write(&path, b"{").unwrap();
        let store = DriverStore { root: root.clone() };

        assert!(store.versions().is_err());
        assert!(store.record_version("oracle", "0.1.66").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{");

        fs::remove_dir_all(root).unwrap();
    }
}
