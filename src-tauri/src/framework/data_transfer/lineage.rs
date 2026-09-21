//! 框架 · 导入映射（sourceLineage）：`spaces/<uid>/import-map.json`
//!
//! 职责（L3 方案 §5）：记录「来源空间 + 数据集 + 来源记录 id → 本空间记录 id」的对应关系，
//! 让重复导入同一包时只做「已识别/待决策」而不是重复创建。
//!
//! 口径：
//! - **映射是意图记录，不是正确性的唯一来源**：文件缺失、`version` 不认识、条目字段非法
//!   一律视为空映射并重新判定（自愈），不阻断导入；
//! - 键唯一性：`(来源 uid, 数据集, 来源 id, 目标 id)`；同一条来源记录可被多次导入成多个
//!   目标（「作为新副本」），多个目标就是多行；
//! - 只记对应关系与最近见到它的包，**不记内容摘要**：内容比较由适配器按业务字段做，
//!   摘要会随字段演进失效；
//! - 写入原子（唯一临时名 + fsync + rename），空间级、不随任何代际/批次目录切换。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::framework::secure_store::replace_file;
use crate::framework::space::index;

/// 映射文件名（空间根下）
pub(crate) const IMPORT_MAP_FILE: &str = "import-map.json";

/// 当前映射格式版本
const MAP_VERSION: u32 = 1;

/// 一条来源 → 目标的对应记录
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineageEntry {
    /// 来源空间 uid（包 header 的 sourceSpaceId；不可信，仅作同源识别键）
    pub source_space_id: String,
    /// 数据集名（如 `ssh.profiles`）
    pub dataset: String,
    /// 来源记录 id（包内 id）
    pub source_id: String,
    /// 本空间记录 id
    pub target_id: String,
    /// 见过这条映射的包 id 列表（同一来源可被多个包反复携带）
    #[serde(default)]
    pub package_ids: Vec<String>,
    /// 最近一次见到它的时间（RFC3339）
    pub last_seen_at: String,
}

/// 映射表（整个文件的内容）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportMap {
    /// 格式版本（不认识即视为空表）
    pub version: u32,
    /// 全部映射条目
    #[serde(default)]
    pub entries: Vec<LineageEntry>,
}

impl Default for ImportMap {
    fn default() -> Self {
        Self {
            version: MAP_VERSION,
            entries: Vec::new(),
        }
    }
}

impl ImportMap {
    /// 查某条来源记录在本空间的所有目标（可能多条：同一来源被「作为新副本」导过多次）
    pub fn lookup<'a>(
        &'a self,
        source_space_id: &str,
        dataset: &str,
        source_id: &str,
    ) -> Vec<&'a LineageEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.source_space_id == source_space_id
                    && entry.dataset == dataset
                    && entry.source_id == source_id
            })
            .collect()
    }

    /// 登记一条映射：同键（来源+数据集+来源 id+目标 id）已存在则合并包列表并刷新时间，
    /// 否则新增一行
    pub fn record(
        &mut self,
        source_space_id: &str,
        dataset: &str,
        source_id: &str,
        target_id: &str,
        package_id: &str,
        seen_at: &str,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|entry| {
            entry.source_space_id == source_space_id
                && entry.dataset == dataset
                && entry.source_id == source_id
                && entry.target_id == target_id
        }) {
            if !existing.package_ids.iter().any(|id| id == package_id) {
                existing.package_ids.push(package_id.to_string());
            }
            existing.last_seen_at = seen_at.to_string();
            return;
        }
        self.entries.push(LineageEntry {
            source_space_id: source_space_id.to_string(),
            dataset: dataset.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            package_ids: vec![package_id.to_string()],
            last_seen_at: seen_at.to_string(),
        });
    }
}

/// 映射文件路径（`<设备根>/spaces/<uid>/import-map.json`）
pub(crate) fn map_path(device_root: &Path, space_id: &str) -> PathBuf {
    index::space_root(device_root, space_id).join(IMPORT_MAP_FILE)
}

/// 读取映射（缺失/损坏/版本不认识/条目非法 → 空表；自愈语义，见模块头注释）
pub(crate) fn load(path: &Path) -> ImportMap {
    let Ok(bytes) = std::fs::read(path) else {
        return ImportMap::default();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        log::warn!("导入映射文件损坏，保留原文件并按空表处理");
        return ImportMap::default();
    };
    if value.get("version").and_then(|v| v.as_u64()) != Some(MAP_VERSION as u64) {
        return ImportMap::default();
    }
    match serde_json::from_value::<ImportMap>(value) {
        Ok(map) => map,
        Err(error) => {
            log::warn!(
                "导入映射条目非法，按空表处理: {error_type}",
                error_type = std::any::type_name_of_val(&error)
            );
            ImportMap::default()
        }
    }
}

/// 写入映射（原子替换：唯一临时名 + fsync + rename）
pub(crate) fn save(path: &Path, map: &ImportMap) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建映射目录失败: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(map).map_err(|e| format!("映射序列化失败: {e}"))?;
    replace_file(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("covekit-lineage-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 读写往返 + 同键合并包列表 + 多目标多行
    #[test]
    fn record_lookup_and_roundtrip() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("import-map.json");
        let mut map = ImportMap::default();
        map.record(
            "src-a",
            "ssh.profiles",
            "p1",
            "t1",
            "pkg-1",
            "2026-09-16T10:00:00+08:00",
        );
        // 同键再导：合并包列表，不产生第二行
        map.record(
            "src-a",
            "ssh.profiles",
            "p1",
            "t1",
            "pkg-2",
            "2026-09-17T10:00:00+08:00",
        );
        // 「作为新副本」：同来源不同目标 = 另一行
        map.record(
            "src-a",
            "ssh.profiles",
            "p1",
            "t2",
            "pkg-3",
            "2026-09-18T10:00:00+08:00",
        );

        assert_eq!(map.entries.len(), 2);
        let hits = map.lookup("src-a", "ssh.profiles", "p1");
        assert_eq!(hits.len(), 2, "同来源同数据集同来源 id 可对应多个目标");
        assert_eq!(
            hits[0].package_ids,
            vec!["pkg-1".to_string(), "pkg-2".to_string()],
            "同键合并包列表"
        );

        save(&path, &map).expect("写入成功");
        let back = load(&path);
        assert_eq!(back, map, "读写往返必须一致");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 自愈：文件缺失、损坏、版本不认识都按空表处理（不阻断导入）
    #[test]
    fn load_heals_missing_corrupt_and_unknown_version() {
        let dir = temp_dir("heal");
        let path = dir.join("import-map.json");

        // 缺失
        assert!(load(&path).entries.is_empty());

        // 损坏（非 JSON）
        std::fs::write(&path, b"not json at all").unwrap();
        assert!(load(&path).entries.is_empty());

        // 版本不认识
        std::fs::write(&path, br#"{"version": 99, "entries": []}"#).unwrap();
        assert!(load(&path).entries.is_empty());

        // 条目字段非法
        std::fs::write(
            &path,
            br#"{"version": 1, "entries": [{"sourceSpaceId": 1}]}"#,
        )
        .unwrap();
        assert!(load(&path).entries.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
