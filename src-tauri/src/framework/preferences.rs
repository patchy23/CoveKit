//! 框架 · 空间级偏好文件（`<空间根>/preferences.json`）
//!
//! 归属（任务书 §13.1 冻结表，唯一来源见 `settings::is_device_key`）：
//! - **空间级**偏好（主题、语言、工具设置、最近使用）随空间隔离，写在本文件；
//! - **设备级**配置（存储根、布局版本、开机自启、默认下载目录、FRP 三项本机事实）留在
//!   自举配置 `settings.json/app` —— 换空间后「本机装在哪、开不开机自启」不该跟着变。
//!
//! 读：文件不存在返回空表；解析失败返回错误并**保留原文件**（不静默清空用户偏好）。
//! 写：唯一临时名 + 同目录 rename 原子替换（断电不会留半截 JSON）。
//! 旧值零迁移：本文件缺某个键时由 `settings` 的读路径回落读自举配置里的历史值，
//! 因此升级后用户看不到任何偏好丢失；写入才落新位置。

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use tauri::AppHandle;

/// 偏好文件名（空间根下）
pub(crate) const PREFERENCES_FILE: &str = "preferences.json";

/// 指定空间根下的偏好文件路径（纯函数，便于用夹具构造第二个空间）
pub(crate) fn path_in(space_root: &Path) -> PathBuf {
    space_root.join(PREFERENCES_FILE)
}

/// 读偏好表：文件不存在 → 空表；内容损坏 → 明确报错（原文件保持不动）
pub(crate) fn read_at(path: &Path) -> Result<Map<String, Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(format!("偏好文件读取失败: {e}")),
    };
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    serde_json::from_str::<Map<String, Value>>(&text)
        .map_err(|e| format!("偏好文件解析失败（文件保持原样，未做清除）: {e}"))
}

/// 写偏好表：唯一临时名 + 同目录原子替换
pub(crate) fn write_at(path: &Path, map: &Map<String, Value>) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("偏好文件路径没有父目录: {}", path.display()));
    };
    std::fs::create_dir_all(parent).map_err(|e| format!("偏好目录创建失败: {e}"))?;
    let tmp = path.with_file_name(format!(
        "{}.tmp-{}-{:?}",
        PREFERENCES_FILE,
        std::process::id(),
        std::thread::current().id()
    ));
    let body = serde_json::to_vec_pretty(&Value::Object(map.clone()))
        .map_err(|e| format!("偏好序列化失败: {e}"))?;
    std::fs::write(&tmp, &body).map_err(|e| format!("偏好写入失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        // 失败时不留临时文件，避免下次启动误认成用户数据
        std::fs::remove_file(&tmp).ok();
        format!("偏好文件替换失败: {e}")
    })
}

/// 当前空间的偏好文件路径（经存储位置描述符；描述符不可用时退化为自举配置的存储根）
pub(crate) fn current_path(app: &AppHandle) -> Result<PathBuf, String> {
    let root = match crate::framework::context::location() {
        Some(location) => location.root.clone(),
        None => crate::framework::paths::storage_root(app)?,
    };
    Ok(path_in(&root))
}

/// 读当前空间的偏好表
pub(crate) fn read_current(app: &AppHandle) -> Result<Map<String, Value>, String> {
    read_at(&current_path(app)?)
}

/// 写当前空间的偏好表
pub(crate) fn write_current(app: &AppHandle, map: &Map<String, Value>) -> Result<(), String> {
    write_at(&current_path(app)?, map)
}

/// 设备级顶层设置键（跨空间共享；任务书 §13.1 冻结表）。
///
/// 未列出的顶层键一律按**空间级**处理：新增字段默认随空间隔离，要设备级必须在这里显式表态。
/// 否则会出现「换空间后本机事实跟着变」的隐蔽缺陷，而这类缺陷回归测试很难覆盖。
pub(crate) fn is_device_key(key: &str) -> bool {
    matches!(key, "launchAtStartup" | "defaultDownloadDirectory")
        || super::settings::RESERVED_KEYS.contains(&key)
}

/// 工具设置里属设备级的键：`frpc` 装在哪、档案目录、下载镜像都是**本机事实**
/// （换空间后本机上的 frpc 还是同一个），按任务书 §13.1 留在设备层。
const DEVICE_TOOL_KEYS: [&str; 3] = ["frpcPath", "profileDir", "downloadMirror"];

/// 工具设置键是否设备级（仅 `frp` 的三项）
pub(crate) fn is_device_tool_key(owner: &str, key: &str) -> bool {
    owner == "frp" && DEVICE_TOOL_KEYS.contains(&key)
}

/// 一层设置表（设备层或空间层）
pub(crate) type Layer = Map<String, Value>;

/// 按归属把一批设置拆成两层：设备级（自举配置）与空间级（当前空间偏好文件）
pub(crate) fn split_patch(patch: &Layer) -> Result<(Layer, Layer), String> {
    let mut device = Map::new();
    let mut space = Map::new();
    for (key, value) in patch {
        if key == "tools" {
            let (device_tools, space_tools) = split_tools(value)?;
            if !device_tools.is_empty() {
                device.insert("tools".to_string(), Value::Object(device_tools));
            }
            if !space_tools.is_empty() {
                space.insert("tools".to_string(), Value::Object(space_tools));
            }
            continue;
        }
        if is_device_key(key) {
            device.insert(key.clone(), value.clone());
        } else {
            space.insert(key.clone(), value.clone());
        }
    }
    Ok((device, space))
}

/// 拆分工具设置（owner → key/value），设备级键单独成层
pub(crate) fn split_tools(value: &Value) -> Result<(Layer, Layer), String> {
    let owners = value
        .as_object()
        .ok_or_else(|| "工具设置必须是对象（工具 id → 设置键值）".to_string())?;
    let mut device = Map::new();
    let mut space = Map::new();
    for (owner, settings) in owners {
        let settings = settings
            .as_object()
            .ok_or_else(|| format!("工具 {owner} 的设置必须是对象"))?;
        let mut device_entry = Map::new();
        let mut space_entry = Map::new();
        for (key, setting) in settings {
            if is_device_tool_key(owner, key) {
                device_entry.insert(key.clone(), setting.clone());
            } else {
                space_entry.insert(key.clone(), setting.clone());
            }
        }
        if !device_entry.is_empty() {
            device.insert(owner.clone(), Value::Object(device_entry));
        }
        if !space_entry.is_empty() {
            space.insert(owner.clone(), Value::Object(space_entry));
        }
    }
    Ok((device, space))
}

/// 合并视图：设备级为底、空间级覆盖（工具设置按 owner/key 逐键合并，不做整对象覆盖）
pub(crate) fn merge_view(device: &Value, space: &Layer) -> Value {
    let mut merged = device.as_object().cloned().unwrap_or_default();
    merged.remove("tools");
    for (key, value) in space {
        if key == "tools" {
            continue;
        }
        merged.insert(key.clone(), value.clone());
    }
    let tools = merged_tools(device, space);
    if tools.as_object().is_some_and(|map| !map.is_empty()) {
        merged.insert("tools".to_string(), tools);
    }
    Value::Object(merged)
}

/// 合并工具设置视图：空间层覆盖同名 owner 的同名键，设备级键只在自举配置里
pub(crate) fn merged_tools(device: &Value, space: &Layer) -> Value {
    let mut merged = device
        .get("tools")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(space_tools) = space.get("tools").and_then(Value::as_object) {
        for (owner, settings) in space_tools {
            let entry = merged
                .entry(owner.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if let (Some(target), Some(incoming)) = (entry.as_object_mut(), settings.as_object()) {
                for (key, value) in incoming {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }
    Value::Object(merged)
}

/// 单键读：空间级键先看偏好文件、缺失回落设备层历史值；设备级键只看设备层
pub(crate) fn read_view(device: &Value, space: &Layer, key: &str) -> Value {
    if key == "tools" {
        return merged_tools(device, space);
    }
    if is_device_key(key) {
        return device.get(key).cloned().unwrap_or(Value::Null);
    }
    space
        .get(key)
        .cloned()
        .or_else(|| device.get(key).cloned())
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用例专用临时目录（进程 id + 唯一名）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "preferences-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 不存在 → 空表（不是错误）
    #[test]
    fn missing_file_reads_as_empty() {
        let dir = temp_dir("missing");
        assert!(read_at(&path_in(&dir)).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 写入后能读回，且不留临时文件
    #[test]
    fn write_then_read_roundtrip() {
        let dir = temp_dir("roundtrip");
        let path = path_in(&dir);
        let mut map = Map::new();
        map.insert("theme".into(), serde_json::json!("dark"));
        map.insert("language".into(), serde_json::json!("zh-CN"));
        write_at(&path, &map).unwrap();

        let read = read_at(&path).unwrap();
        assert_eq!(read.get("theme").and_then(Value::as_str), Some("dark"));
        let leftovers: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "不应残留临时文件: {leftovers:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 内容损坏 → 明确报错且原文件保持原样（不得静默清空用户偏好）
    #[test]
    fn corrupt_file_is_reported_and_kept() {
        let dir = temp_dir("corrupt");
        let path = path_in(&dir);
        std::fs::write(&path, b"{ not json").unwrap();
        let error = read_at(&path).unwrap_err();
        assert!(error.contains("解析失败"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 两空间各写一份：偏好文件落在各自空间根下，互不可见（隔离的路径层）
    #[test]
    fn two_spaces_keep_separate_preferences() {
        let device = temp_dir("two-spaces");
        let default_root = device.clone();
        let space_root = device
            .join("spaces")
            .join("7c1d0a94-2b6f-4e83-8f52-1a9de4c7b305")
            .join("generations")
            .join("1");

        let mut first = Map::new();
        first.insert("theme".into(), serde_json::json!("light"));
        write_at(&path_in(&default_root), &first).unwrap();
        let mut second = Map::new();
        second.insert("theme".into(), serde_json::json!("dark"));
        write_at(&path_in(&space_root), &second).unwrap();

        assert_ne!(path_in(&default_root), path_in(&space_root));
        assert_eq!(
            read_at(&path_in(&default_root))
                .unwrap()
                .get("theme")
                .and_then(Value::as_str),
            Some("light")
        );
        assert_eq!(
            read_at(&path_in(&space_root))
                .unwrap()
                .get("theme")
                .and_then(Value::as_str),
            Some("dark")
        );
        std::fs::remove_dir_all(&device).ok();
    }
    /// 归属拆分：空间级进偏好层、设备级进自举层
    #[test]
    fn patch_splits_by_scope() {
        let mut patch = Layer::new();
        patch.insert("theme".into(), serde_json::json!("dark"));
        patch.insert("language".into(), serde_json::json!("zh-CN"));
        patch.insert("launchAtStartup".into(), serde_json::json!(true));
        patch.insert(
            "defaultDownloadDirectory".into(),
            serde_json::json!("D:/dl"),
        );
        patch.insert("storageRoot".into(), serde_json::json!("D:/pb"));

        let (device, space) = split_patch(&patch).unwrap();
        assert!(device.contains_key("launchAtStartup"));
        assert!(device.contains_key("defaultDownloadDirectory"));
        assert!(device.contains_key("storageRoot"), "自举键必须留设备层");
        assert!(!space.contains_key("launchAtStartup"));
        assert_eq!(
            space.get("theme").and_then(Value::as_str),
            Some("dark"),
            "主题随空间隔离"
        );
        assert_eq!(space.get("language").and_then(Value::as_str), Some("zh-CN"));
    }

    /// 工具设置拆分：FRP 的三项本机事实留设备层，其余（含同 owner 的其他键）随空间
    #[test]
    fn frp_device_keys_stay_on_device_side() {
        let tools = serde_json::json!({
            "frp": { "frpcPath": "D:/frp/frpc.exe", "profileDir": "D:/frp/profiles", "maxLogLines": 500 },
            "ssh": { "keepAlive": 30 }
        });
        let (device_tools, space_tools) = split_tools(&tools).unwrap();
        let device_frp = device_tools.get("frp").and_then(Value::as_object).unwrap();
        assert!(device_frp.contains_key("frpcPath"));
        assert!(device_frp.contains_key("profileDir"));
        assert!(!device_frp.contains_key("maxLogLines"));
        let space_frp = space_tools.get("frp").and_then(Value::as_object).unwrap();
        assert!(space_frp.contains_key("maxLogLines"));
        assert!(space_tools.get("ssh").is_some(), "其他工具设置随空间");
        assert!(device_tools.get("ssh").is_none());
    }

    /// 合并视图：空间级覆盖设备层同名键，设备级键与工具级合并都保留
    #[test]
    fn merged_view_overlays_space_over_device() {
        let device = serde_json::json!({
            "theme": "light",
            "launchAtStartup": true,
            "tools": { "frp": { "frpcPath": "D:/frp/frpc.exe" } }
        });
        let mut space = Layer::new();
        space.insert("theme".into(), serde_json::json!("dark"));
        space.insert(
            "tools".into(),
            serde_json::json!({ "frp": { "maxLogLines": 500 } }),
        );

        let merged = merge_view(&device, &space);
        assert_eq!(merged.get("theme").and_then(Value::as_str), Some("dark"));
        assert_eq!(
            merged.get("launchAtStartup").and_then(Value::as_bool),
            Some(true),
            "设备级键必须仍在"
        );
        let frp = merged.get("tools").and_then(|t| t.get("frp")).unwrap();
        assert_eq!(frp.get("maxLogLines").and_then(Value::as_u64), Some(500));
        assert_eq!(
            frp.get("frpcPath").and_then(Value::as_str),
            Some("D:/frp/frpc.exe"),
            "设备级工具键不得被空间层抹掉"
        );
    }

    /// 单键读：偏好文件缺该键时回落设备层历史值（升级后看不到偏好丢失）
    #[test]
    fn read_view_falls_back_to_device_value() {
        let device = serde_json::json!({ "theme": "light", "launchAtStartup": false });
        let space = Layer::new();
        assert_eq!(
            read_view(&device, &space, "theme").as_str(),
            Some("light"),
            "空间层为空时读历史值"
        );
        let mut with_space = Layer::new();
        with_space.insert("theme".into(), serde_json::json!("dark"));
        assert_eq!(
            read_view(&device, &with_space, "theme").as_str(),
            Some("dark")
        );
        assert_eq!(
            read_view(&device, &with_space, "launchAtStartup").as_bool(),
            Some(false),
            "设备级键不受空间层影响"
        );
    }
}
