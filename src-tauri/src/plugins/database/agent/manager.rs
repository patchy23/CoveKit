//! agent 驱动 store：目录布局 + versions.json + 镜像下载
//! 布局：app_data_dir/agents/drivers/<key>/（agent 可执行文件或 agent.jar + versions.json）
//! 获取策略（与 dbx 一致但无物理依赖）：
//!   1. 本地已有驱动二进制 → 直接使用
//!   2. 设置 database.agentMirror 配置了镜像 URL → 按 {type}/{version} 模板下载
//!   3. 均不可用 → 返回带指引的错误（手动放置或配置镜像）
//!
//! 大文件不随安装包分发；下载产物仅存本地 store。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::plugins::database::agent::driver_key;
use crate::plugins::database::models::DbType;
use tauri::Manager;

/// 驱动 store 根目录下的版本清单文件名
const VERSIONS_FILE: &str = "versions.json";

/// agent 驱动 store（路径解析 + 版本清单 + 下载）
pub struct DriverStore {
    /// store 根目录（app_data_dir/agents/drivers）
    root: PathBuf,
}

impl DriverStore {
    /// 从应用数据目录构造驱动 store
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("数据目录获取失败: {e}"))?;
        let root = dir.join("agents").join("drivers");
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

    /// 确保某类型驱动可用：本地二进制 → 镜像下载 → 明确报错
    pub async fn ensure_driver(
        &self,
        app: &tauri::AppHandle,
        db_type: DbType,
    ) -> Result<PathBuf, String> {
        // 达梦本批次未实现（需要 Java agent + JRE，工作量大；UI 保留入口）
        if matches!(db_type, DbType::Dameng) {
            return Err("达梦驱动暂未支持（本版本未实现，后续版本提供）。".to_string());
        }
        if let Some(binary) = self.agent_binary(db_type) {
            return Ok(binary);
        }
        // 尝试按镜像下载（设置 database.agentMirror，空则跳过）
        let mirror = mirror_url(app).unwrap_or_default();
        if !mirror.is_empty() {
            let version = self
                .versions()
                .get(driver_key(db_type))
                .cloned()
                .ok_or_else(|| format!("versions.json 缺少 {} 驱动版本", driver_key(db_type)))?;
            let dir = self.driver_dir(db_type)?;
            let url = mirror
                .replace("{type}", driver_key(db_type))
                .replace("{version}", &version);
            download_file(&url, &dir.join("agent")).await?;
            if let Some(binary) = self.agent_binary(db_type) {
                return Ok(binary);
            }
        }
        Err(format!(
            "缺少 {} 驱动。请将 agent 可执行文件放入 {}，或在设置中配置驱动镜像（URL 模板支持 {{type}}/{{version}}）。",
            driver_key(db_type),
            self.root.join(driver_key(db_type)).display()
        ))
    }
}

/// 读取设置的驱动镜像 URL（database.agentMirror；空串 = 不下载）
fn mirror_url(app: &tauri::AppHandle) -> Result<String, String> {
    let value = crate::framework::settings::settings_get(app.clone(), None)?;
    Ok(value
        .get("database")
        .and_then(|d| d.get("agentMirror"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// 下载文件到目标路径（reqwest；超时 60s；覆盖已存在的临时文件）
async fn download_file(url: &str, target: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载驱动失败（{url}）：{e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取下载内容失败: {e}"))?;
    let tmp = target.with_extension("tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("写入驱动临时文件失败: {e}"))?;
    std::fs::rename(&tmp, target).map_err(|e| format!("驱动文件落位失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 驱动目录按类型命名() {
        assert_eq!(driver_key(DbType::Oracle), "oracle");
        assert_eq!(driver_key(DbType::Kingbase), "kingbase");
        assert_eq!(driver_key(DbType::Vastbase), "vastbase");
        assert_eq!(driver_key(DbType::Dameng), "dameng");
    }

    #[test]
    fn 镜像url模板替换() {
        let url = "https://mirror.example.com/{type}/{version}/agent"
            .replace("{type}", "oracle")
            .replace("{version}", "0.1.48");
        assert_eq!(url, "https://mirror.example.com/oracle/0.1.48/agent");
    }
}
