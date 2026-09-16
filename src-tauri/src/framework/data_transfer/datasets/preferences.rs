//! 偏好数据集适配（`core.favorites` / `core.recent_tools`）
//!
//! 只带**用户可见习惯**这两项（任务书 §13.1 白名单）：收藏默认带上、最近使用默认不带。
//! 其余空间级偏好（窗口尺寸、面板展开态、主题等）属本机使用痕迹，本批不进包也不列目录——
//! 列出来却又提示「不能带」，只会让用户以为丢东西。
//!
//! 记录结构就是字符串数组里的元素（工具 id），与 `preferencesSet` 的既有形状一致：
//! 导入侧据此还原 `preferences.json` 的两个键，不做结构改造。

use serde_json::Value;
use tauri::AppHandle;

use super::super::adapter::{self, DatasetAdapter, StagingTarget};
use super::super::types::{
    DatasetDescriptor, DependencyEdge, ImportContext, ImportPlanItem, TransportPolicy,
};
use crate::framework::preferences;

/// 收藏数据集名（owner = `core`）
pub(crate) const DATASET_FAVORITES: &str = "core.favorites";
/// 最近使用数据集名（owner = `core`）
pub(crate) const DATASET_RECENT: &str = "core.recent_tools";
/// 数据集 schema 版本
pub(crate) const SCHEMA_VERSION: u32 = 1;
/// 收藏在 `preferences.json` 里的键（与 `SpaceDataKey` 白名单一致）
pub(crate) const FAVORITES_KEY: &str = "favorites";
/// 最近使用在 `preferences.json` 里的键（与 `SpaceDataKey` 白名单一致）
pub(crate) const RECENT_KEY: &str = "recentTools";

/// 习惯数据集适配器
struct PreferencesAdapter;

impl DatasetAdapter for PreferencesAdapter {
    /// 归属 owner：框架自有（`core.*`），与插件 owner 区分
    fn owner(&self) -> &'static str {
        "core"
    }

    /// 声明收藏与最近使用：都可整块勾选，不含秘密
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        let map = preferences::read_current(app)?;
        let favorites = ids_of(&map, FAVORITES_KEY)?;
        let recent = ids_of(&map, RECENT_KEY)?;
        let descriptor = |name: &str, label: &str, count: usize, default_selected: bool| {
            DatasetDescriptor {
                name: name.to_string(),
                label: label.to_string(),
                owner: self.owner().to_string(),
                policy: TransportPolicy::Portable,
                schema_version: SCHEMA_VERSION,
                selectable: true,
                contains_secret: false,
                default_selected,
                pulls: Vec::new(),
                note: None,
                record_count: count,
                // 习惯是整块勾选：不提供逐条条目（逐条挑选收藏属于没有需求的界面负担）
                entries: Vec::new(),
            }
        };
        Ok(vec![
            descriptor(DATASET_FAVORITES, "收藏工具", favorites.len(), true),
            descriptor(DATASET_RECENT, "最近使用", recent.len(), false),
        ])
    }

    /// 取数据集记录：整块返回（`ids` 为空即全量；非空时按 id 过滤，供将来逐条选择复用）
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        let key = key_of(dataset)?;
        let map = preferences::read_current(app)?;
        let all = ids_of(&map, key)?;
        let selected: Vec<Value> = if ids.is_empty() {
            all.into_iter().map(Value::String).collect()
        } else {
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                if !all.iter().any(|item| item == id) {
                    return Err(format!(
                        "{dataset} 不存在记录 {id}（列表可能已过期，请刷新后重试）"
                    ));
                }
                out.push(Value::String(id.clone()));
            }
            out
        };
        Ok(selected)
    }

    /// 记录体检：必须是工具 id 形态的非空字符串（导入侧要按 id 还原收藏）
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        key_of(dataset)?;
        for record in records {
            match record.as_str() {
                Some(id) if !id.trim().is_empty() => {}
                _ => return Err(format!("{dataset} 记录必须是工具 id（非空字符串）")),
            }
        }
        Ok(())
    }

    /// 习惯是叶子：没有它引用的别的东西
    fn enumerate_references(
        &self,
        dataset: &str,
        _records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        key_of(dataset)?;
        Ok(Vec::new())
    }

    /// 导入判定：整块带入，逐条回显工具 id（界面能看到「带进来哪些」）
    fn plan_import(
        &self,
        dataset: &str,
        records: &[Value],
        _context: &ImportContext,
    ) -> Result<Vec<ImportPlanItem>, String> {
        key_of(dataset)?;
        if records.is_empty() {
            return Ok(vec![ImportPlanItem::excluded(
                dataset,
                dataset,
                "包内这一类是空列表",
            )]);
        }
        Ok(records
            .iter()
            .filter_map(|record| record.as_str())
            .map(|tool| ImportPlanItem::added(dataset, tool, tool))
            .collect())
    }

    /// 写入暂存目录的 `preferences.json`：读改写，避免两类习惯互相覆盖
    fn apply_to_staging(
        &self,
        dataset: &str,
        records: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        let key = key_of(dataset)?;
        let path = target.root.join(PREFERENCES_FILE);
        let mut map = read_preferences_at(&path)?;
        map.insert(key.to_string(), Value::Array(records.to_vec()));
        write_preferences_at(&path, &map)?;
        // 自查：读回条数一致才算写入成功
        let back = read_preferences_at(&path)?;
        let count = ids_of(&back, key)?.len();
        if count != records.len() {
            return Err(format!(
                "{dataset} 写入后读回 {count} 条，预期 {} 条",
                records.len()
            ));
        }
        Ok(records.len())
    }
}

/// 空间偏好文件名（四分区布局的固定落位）
const PREFERENCES_FILE: &str = "preferences.json";

/// 读取指定目录下的偏好文件：文件不存在 = 空表（新空间就是这样）
fn read_preferences_at(path: &std::path::Path) -> Result<serde_json::Map<String, Value>, String> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serde_json::Map::new())
        }
        Err(error) => return Err(format!("读取偏好文件失败: {error}")),
    };
    if raw.is_empty() {
        return Ok(serde_json::Map::new());
    }
    let value: Value =
        serde_json::from_slice(&raw).map_err(|e| format!("偏好文件结构不认识: {e}"))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err("偏好文件结构不认识（应为对象）".into()),
    }
}

/// 写入偏好文件：先写临时文件再改名，避免半截文件（与框架原子写同款做法）
fn write_preferences_at(
    path: &std::path::Path,
    map: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(map).map_err(|e| format!("偏好序列化失败: {e}"))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, body).map_err(|e| format!("写入偏好临时文件失败: {e}"))?;
    std::fs::rename(&temp, path).map_err(|e| format!("偏好文件改名失败: {e}"))
}

/// 数据集名 → `preferences.json` 键（未知数据集直接拒绝，避免把别的键当习惯带出）
fn key_of(dataset: &str) -> Result<&'static str, String> {
    match dataset {
        DATASET_FAVORITES => Ok(FAVORITES_KEY),
        DATASET_RECENT => Ok(RECENT_KEY),
        other => Err(format!("偏好适配器不支持数据集 {other}")),
    }
}

/// 读取白名单键的 id 列表：键缺失 = 空；结构不符**报错**（不按空表处理，否则导出会静默少带）
fn ids_of(map: &serde_json::Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("偏好 {key} 含非字符串项"))
            })
            .collect(),
        Some(_) => Err(format!("偏好 {key} 结构不符（应为字符串数组）")),
    }
}

/// 登记偏好适配器（装配阶段调用一次）
pub(crate) fn register() {
    static ADAPTER: PreferencesAdapter = PreferencesAdapter;
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合法数据集映射到既有键；未知数据集被拒（不猜键名）
    #[test]
    fn dataset_names_map_to_existing_keys() {
        assert_eq!(key_of(DATASET_FAVORITES).unwrap(), "favorites");
        assert_eq!(key_of(DATASET_RECENT).unwrap(), "recentTools");
        assert!(key_of("core.theme").is_err());
    }

    /// 键缺失按空表；非数组结构报错；数组含非字符串报错
    #[test]
    fn ids_of_is_strict_about_structure() {
        let mut map = serde_json::Map::new();
        assert!(ids_of(&map, FAVORITES_KEY).unwrap().is_empty());

        map.insert(FAVORITES_KEY.into(), serde_json::json!([]));
        assert!(ids_of(&map, FAVORITES_KEY).unwrap().is_empty());

        map.insert(FAVORITES_KEY.into(), serde_json::json!(["ssh", "dns"]));
        assert_eq!(
            ids_of(&map, FAVORITES_KEY).unwrap(),
            vec!["ssh".to_string(), "dns".to_string()]
        );

        map.insert(FAVORITES_KEY.into(), serde_json::json!("ssh"));
        assert!(ids_of(&map, FAVORITES_KEY)
            .unwrap_err()
            .contains("结构不符"));

        map.insert(FAVORITES_KEY.into(), serde_json::json!(["ssh", 3]));
        assert!(ids_of(&map, FAVORITES_KEY)
            .unwrap_err()
            .contains("非字符串"));
    }

    /// 记录体检：非空字符串通过；空串与非字符串被拒
    #[test]
    fn validate_records_requires_tool_ids() {
        let adapter = PreferencesAdapter;
        assert!(adapter
            .validate_records(DATASET_FAVORITES, &[serde_json::json!("ssh")])
            .is_ok());
        assert!(adapter
            .validate_records(DATASET_FAVORITES, &[serde_json::json!(" ")])
            .is_err());
        assert!(adapter
            .validate_records(DATASET_RECENT, &[serde_json::json!({"id": "ssh"})])
            .is_err());
    }

    /// 未知数据集在三个方法上都直接拒绝
    #[test]
    fn unknown_dataset_is_rejected_everywhere() {
        let adapter = PreferencesAdapter;
        assert!(adapter.validate_records("core.theme", &[]).is_err());
        assert!(adapter.enumerate_references("core.theme", &[]).is_err());
        assert!(adapter.validate_records(DATASET_FAVORITES, &[]).is_ok());
    }
}
