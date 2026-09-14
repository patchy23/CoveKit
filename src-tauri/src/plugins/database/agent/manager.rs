//! agent 驱动 store：目录布局 + versions.json + 本地查找
//! 布局：app_data_dir/agents/drivers/<key>/（agent 可执行文件或 agent.jar + versions.json）
//! 获取策略（与 dbx 一致但无物理依赖）：
//!   1. 本地已有驱动二进制 → 直接使用
//!   2. 没有 → 返回带指引的错误（给出该放的目录）
//!
//! 大文件不随安装包分发。2026-09-14 移除「镜像下载」分支：它读 `app.database.agentMirror`，
//! 而工具级设置实际落在 `app.tools.database.*` 且 database 插件从未声明设置项，
//! 条件恒为假、下载从未执行过；报错文案却还让用户去设置里配镜像。

use std::collections::HashMap;
use std::path::PathBuf;

use crate::plugins::database::agent::driver_key;
use crate::plugins::database::models::DbType;

/// 驱动 store 根目录下的版本清单文件名
const VERSIONS_FILE: &str = "versions.json";

/// agent 驱动 store（路径解析 + 版本清单 + 下载）
pub struct DriverStore {
    /// store 根目录（app_data_dir/agents/drivers）
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

    /// 驱动二进制路径：优先 `agent(.exe)`，其次 `agent.jar`（Java 驱动）
    pub fn agent_binary(&self, db_type: DbType) -> Option<PathBuf> {
        let dir = self.root.join(driver_key(db_type));
        let exe = if cfg!(windows) {
            dir.join("agent.exe")
        } else {
            dir.join("agent")
        };
        if exe.is_file() {
            return Some(exe);
        }
        let plain = dir.join("agent");
        if plain.is_file() {
            return Some(plain);
        }
        let jar = dir.join("agent.jar");
        if jar.is_file() {
            return Some(jar);
        }
        None
    }

    /// 读取 versions.json（缺省返回空表）
    pub fn versions(&self) -> HashMap<String, String> {
        let path = self.root.join(VERSIONS_FILE);
        let Ok(text) = std::fs::read_to_string(path) else {
            return HashMap::new();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    /// 确保某类型驱动可用：本地二进制 → 缺失时给出放置指引
    pub fn ensure_driver(&self, db_type: DbType) -> Result<PathBuf, String> {
        // 达梦本批次未实现（需要 Java agent + JRE，工作量大；UI 保留入口）
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
}
