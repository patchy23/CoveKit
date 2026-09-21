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
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

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
    /// 是否默认空间（首启/迁移生成的本机初始空间；同一时刻索引里只有一个）
    #[serde(default)]
    pub is_default: bool,
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

    /// 导入创建的空间条目：名称 + 创建时间 + 来源留档 + 各类别条数
    ///
    /// 来源信息（包 id / 来源空间）只做留档与「该包导入过」提示，不参与任何逻辑判断。
    pub fn imported(
        name: &str,
        created_at: &str,
        source_space_id: &str,
        source_space_name: &str,
        package_id: &str,
        counts: &BTreeMap<String, usize>,
    ) -> Self {
        Self {
            name: name.to_string(),
            created_at: created_at.to_string(),
            is_default: false,
            imported_from: Some(ImportSource {
                package_id: package_id.to_string(),
                source_space_id: source_space_id.to_string(),
                source_space_name: source_space_name.to_string(),
                imported_at: created_at.to_string(),
                counts: counts.clone(),
            }),
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

/// 空间展示名（索引缺失或条目缺失时退回空间 id，不编造名字；默认空间未命名时用固定文案）
pub fn display_name(app: &AppHandle, space_id: &str) -> String {
    match read_index(app) {
        Ok(index) => match index.get(space_id) {
            Some(record) if record.is_default && record.name.trim().is_empty() => {
                DEFAULT_SPACE_NAME.to_string()
            }
            Some(record) => record.display_name(space_id),
            None => space_id.to_string(),
        },
        Err(error) => {
            log::info!(
                "空间索引不可读，展示名退回空间 id: {error_type}",
                error_type = std::any::type_name_of_val(&error)
            );
            space_id.to_string()
        }
    }
}

/// 新建暂存目录前缀；崩溃恢复同时识别旧版前缀，其余目录不碰。
pub const STAGING_PREFIX: &str = paths::STAGING_PREFIX;

/// 空间目录集：`<设备根>/spaces`
pub fn spaces_dir(device_root: &Path) -> PathBuf {
    device_root.join("spaces")
}

/// 暂存空间根：`<设备根>/spaces/.covekit-staging-<planId>`
///
/// 与正式空间根**同级**，rename 才是一次原子搬移（跨目录树搬移会退化成复制+删除）。
pub fn staging_space_root(device_root: &Path, plan_id: &str) -> PathBuf {
    spaces_dir(device_root).join(format!("{STAGING_PREFIX}{plan_id}"))
}

/// 暂存空间的**内容根**：`<设备根>/spaces/.covekit-staging-<planId>`
///
/// 与正式空间根同形（无代际层），导入侧因此对「暂存写入」与「写入既有空间」共用相对路径。
pub fn staging_content_root(device_root: &Path, plan_id: &str) -> PathBuf {
    staging_space_root(device_root, plan_id)
}

/// 正式空间根：`<设备根>/spaces/<spaceId>`
pub fn space_root(device_root: &Path, space_id: &str) -> PathBuf {
    spaces_dir(device_root).join(space_id)
}

/// 整体写入空间索引（空间化迁移用；键逐个校验，非法键拒绝整批写入）
pub fn write_all(app: &AppHandle, index: &BTreeMap<String, SpaceRecord>) -> Result<(), String> {
    for space_id in index.keys() {
        if !super::is_valid_space_id(space_id) {
            return Err(format!("空间索引存在非法空间 id，拒绝整体写入：{space_id}"));
        }
    }
    let value = serde_json::to_value(index).map_err(|e| format!("空间索引序列化失败: {e}"))?;
    paths::write_setting(app, KEY_SPACES, value)
}

/// 索引时间戳（RFC3339，本地时区偏移）：空间创建 / 导入留档的唯一时间来源
pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

/// 写入（新增或更新）索引条目：读改写，空间 id 先行校验
pub fn write_record(app: &AppHandle, space_id: &str, record: &SpaceRecord) -> Result<(), String> {
    if !super::is_valid_space_id(space_id) {
        return Err(format!("空间 id 非法，拒绝写入空间索引：{space_id}"));
    }
    let mut index = read_index(app)?;
    index.insert(space_id.to_string(), record.clone());
    let value = serde_json::to_value(&index).map_err(|e| format!("空间索引序列化失败: {e}"))?;
    paths::write_setting(app, KEY_SPACES, value)
}

/// 读当前活动空间标识（未写过的旧安装返回 None）
pub fn read_active_id(app: &AppHandle) -> Option<String> {
    paths::read_setting(app, super::KEY_ACTIVE_SPACE_ID)
        .and_then(|value| value.as_str().map(str::to_string))
}

/// 清理残留的暂存空间目录（启动维护阶段调用）
///
/// 只删「本应用命名 + 位于空间目录集内」的目录：崩溃可能停在 rename 之前，
/// 此时暂存目录里的数据不完整且没有任何索引引用，留着只会占空间并误导排查。
/// 目录集不存在（全新安装）按「无残留」处理，不是错误。
pub fn cleanup_staging(device_root: &Path) -> Result<Vec<String>, String> {
    let spaces = spaces_dir(device_root);
    let entries = match std::fs::read_dir(&spaces) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("读取空间目录失败: {error}")),
    };
    let mut removed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取空间目录项失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if ![
            STAGING_PREFIX,
            crate::framework::brand_compat::LEGACY_STAGING_PREFIX,
        ]
        .iter()
        .any(|prefix| name.starts_with(*prefix) && name.len() > prefix.len())
        {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        std::fs::remove_dir_all(&path).map_err(|e| format!("清理残留暂存目录 {name} 失败: {e}"))?;
        removed.push(name);
    }
    removed.sort();
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 暂存目录与空间目录的落位（导入提交与启动清理共用同一套路径拼法）
    #[test]
    fn staging_and_space_paths_are_siblings() {
        let device = std::path::Path::new("D:/CoveKit");
        let staging = staging_space_root(device, "imp-7f3");
        assert_eq!(
            staging,
            device.join("spaces").join(".covekit-staging-imp-7f3")
        );
        assert_eq!(space_root(device, "abc"), device.join("spaces").join("abc"));
        // 暂存目录的父目录必须是空间目录的父目录：rename 才是一次原子搬移
        assert_eq!(staging.parent(), space_root(device, "abc").parent());
    }

    /// 空间内容根与 `StorageLocation::for_space` 同形（无代际层，适配器只认这一层）
    #[test]
    fn space_root_matches_location_layout() {
        let device = std::path::Path::new("D:/CoveKit");
        let location = crate::framework::context::StorageLocation::for_space(device, "abc");
        assert_eq!(space_root(device, "abc"), location.root);
    }

    /// 清理只删本应用命名的暂存目录，空间目录与非本应用目录一律不碰
    #[test]
    fn cleanup_removes_only_staging_dirs() {
        let dir = std::env::temp_dir().join(format!("pb-staging-clean-{}", std::process::id()));
        let spaces = spaces_dir(&dir);
        std::fs::create_dir_all(spaces.join(".covekit-staging-old")).expect("建暂存目录");
        let legacy = format!(
            "{}old",
            crate::framework::brand_compat::LEGACY_STAGING_PREFIX
        );
        std::fs::create_dir_all(spaces.join(&legacy)).expect("建旧版暂存目录");
        std::fs::create_dir_all(spaces.join(STAGING_PREFIX)).expect("建无计划标识目录");
        std::fs::create_dir_all(spaces.join(crate::framework::brand_compat::LEGACY_STAGING_PREFIX))
            .expect("建旧版无计划标识目录");
        std::fs::create_dir_all(spaces.join("11111111-1111-4111-8111-111111111111"))
            .expect("建空间目录");
        std::fs::create_dir_all(spaces.join("用户自建目录")).expect("建无关目录");

        let removed = cleanup_staging(&dir).expect("清理成功");
        let mut expected = vec![".covekit-staging-old".to_string(), legacy.clone()];
        expected.sort();
        assert_eq!(removed, expected);
        assert!(!spaces.join(".covekit-staging-old").exists());
        assert!(!spaces.join(legacy).exists());
        assert!(spaces.join(STAGING_PREFIX).exists());
        assert!(spaces
            .join(crate::framework::brand_compat::LEGACY_STAGING_PREFIX)
            .exists());
        assert!(spaces.join("11111111-1111-4111-8111-111111111111").exists());
        assert!(spaces.join("用户自建目录").exists());

        // 目录不存在（全新安装）不是错误
        let missing = dir.join("不存在的根");
        assert!(cleanup_staging(&missing)
            .expect("缺失按无残留处理")
            .is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 名称为空时退回空间 id；有名称时用名称
    #[test]
    fn display_name_falls_back_to_id() {
        let mut record = SpaceRecord {
            name: "   ".into(),
            created_at: String::new(),
            is_default: false,
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
