//! 框架 · 空间索引（设备级自举配置里的 `spaces` 键）
//!
//! 归属见任务书 §13.1：本机空间列表与当前选择是**设备级**事实，绝不进包、不随空间切换。
//! 未建立索引的旧安装按「只有默认空间」呈现：读不到键不是错误，也不是空环境。
//!
//! 本批（C2）只提供**读**侧：导出预览与来源留档要显示来源空间名。写入（导入创建空间、
//! 切换活动空间）在 C3（隔离导入）随首个真实消费方一起落地，避免出现没有消费方的写入路径。
//!
//! 越界零容忍：索引里的键必须是合法空间 id（`space::is_valid_space_id`），结构不认识就报错，
//! 不得把非法键静默丢掉——静默丢键等于「空间凭空消失」。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::framework::context::DEFAULT_SPACE_ID;
use crate::framework::paths;

/// 空间索引配置键（`settings.json` 的 `app` 对象内，设备级）
pub const KEY_SPACES: &str = "spaces";

/// 默认空间的展示名（索引里没有 `default` 条目时的固定文案）
pub const DEFAULT_SPACE_NAME: &str = "默认空间";

/// 导入来源留档（设备级部分；`sourceLineage` 的记录级映射见导入提交）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportSource {
    /// 来源包 id（用于「该包是否导入过」提示）
    pub package_id: String,
    /// 来源空间 id（不可信，仅展示）
    pub source_space_id: String,
    /// 来源空间名（不可信，仅展示）
    pub source_space_name: String,
    /// 导入时间（RFC3339）
    pub imported_at: String,
    /// 各数据集导入条数（数据集名 → 条数）
    #[serde(default)]
    pub counts: BTreeMap<String, usize>,
}

/// 一个空间在索引里的登记项
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpaceRecord {
    /// 空间展示名（用户可命名；空则退回空间 id）
    pub name: String,
    /// 空间创建（或登记）时间（RFC3339）
    #[serde(default)]
    pub created_at: String,
    /// 导入来源留档（本机新建空间为空）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<ImportSource>,
}

impl SpaceRecord {
    /// 展示名：名称为空时退回空间 id（不显示空标题）
    pub fn display_name(&self, space_id: &str) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            space_id.to_string()
        } else {
            name.to_string()
        }
    }
}

/// 读取空间索引（缺失 = 尚未建立索引的旧安装，返回空表）
///
/// 结构不认识、键非法都返回 `Err`：这两种情况都说明设备级配置被外部改过，
/// 静默当空会让用户看到「空间列表为空」却明明存在空间目录。
pub fn read_index(app: &AppHandle) -> Result<BTreeMap<String, SpaceRecord>, String> {
    let Some(raw) = paths::read_setting(app, KEY_SPACES) else {
        return Ok(BTreeMap::new());
    };
    let Some(map) = raw.as_object() else {
        return Err("空间索引结构不认识（应为对象；设备级配置可能被外部改动）".into());
    };
    let mut index = BTreeMap::new();
    for (space_id, value) in map {
        if !super::is_valid_space_id(space_id) {
            return Err(format!("空间索引存在非法空间 id：{space_id}"));
        }
        let record: SpaceRecord = serde_json::from_value(value.clone())
            .map_err(|e| format!("空间索引条目 {space_id} 解析失败: {e}"))?;
        index.insert(space_id.clone(), record);
    }
    Ok(index)
}

/// 空间展示名（索引缺失或条目缺失时用默认文案 / 空间 id，不编造名字）
pub fn display_name(app: &AppHandle, space_id: &str) -> String {
    match read_index(app) {
        Ok(index) => match index.get(space_id) {
            Some(record) => record.display_name(space_id),
            None if space_id == DEFAULT_SPACE_ID => DEFAULT_SPACE_NAME.to_string(),
            None => space_id.to_string(),
        },
        Err(error) => {
            eprintln!("[space] 空间索引不可读，展示名退回空间 id: {error}");
            if space_id == DEFAULT_SPACE_ID {
                DEFAULT_SPACE_NAME.to_string()
            } else {
                space_id.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 名称为空时退回空间 id；有名称时用名称
    #[test]
    fn display_name_falls_back_to_id() {
        let mut record = SpaceRecord {
            name: "   ".into(),
            created_at: String::new(),
            imported_from: None,
        };
        assert_eq!(record.display_name("sp1"), "sp1");
        record.name = "工作空间".into();
        assert_eq!(record.display_name("sp1"), "工作空间");
    }

    /// 索引条目往返序列化：缺 `createdAt` 也能解析（旧条目 / 手工写入）
    #[test]
    fn record_tolerates_missing_optional_fields() {
        let record: SpaceRecord =
            serde_json::from_value(serde_json::json!({"name": "导入空间"})).expect("解析成功");
        assert_eq!(record.name, "导入空间");
        assert!(record.created_at.is_empty());
        assert!(record.imported_from.is_none());

        let with_source: SpaceRecord = serde_json::from_value(serde_json::json!({
            "name": "导入空间",
            "createdAt": "2026-09-16T10:00:00+08:00",
            "importedFrom": {
                "packageId": "pkg-1",
                "sourceSpaceId": "default",
                "sourceSpaceName": "默认空间",
                "importedAt": "2026-09-16T10:00:00+08:00",
                "counts": {"ssh.profiles": 2}
            }
        }))
        .expect("解析成功");
        let source = with_source.imported_from.expect("来源留档存在");
        assert_eq!(source.counts.get("ssh.profiles"), Some(&2));
    }
}
