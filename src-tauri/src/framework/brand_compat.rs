//! CoveKit 改名兼容：接续旧安装自举，不搬移数据、不删除旧配置、不触碰凭证材料。

use serde_json::Value;
use std::path::Path;
use tauri::AppHandle;

/// 已发布版本的应用标识，仅用于旧安装兼容，不用于新安装身份。
pub(crate) const LEGACY_APP_ID: &str = "com.patchy23.patchybox";

/// 旧版暂存前缀，只用于读取或清理升级前已存在的暂存目录。
pub(crate) const LEGACY_STAGING_PREFIX: &str = ".patchybox-staging-";

/// 在任何 settings.json store 读取前执行；已有新配置永不被旧安装覆盖。
pub(crate) fn prepare(app: &AppHandle) -> Result<(), String> {
    let current = super::paths::default_root(app)?;
    let parent = current.parent().ok_or("应用目录没有父目录")?;
    let legacy = parent.join(LEGACY_APP_ID);
    adopt_settings(&legacy, &current).map(|_| ())
}

fn adopt_settings(legacy: &Path, current: &Path) -> Result<bool, String> {
    let destination = current.join("settings.json");
    if destination.try_exists().map_err(|e| e.to_string())? {
        return Ok(false);
    }
    let source = legacy.join("settings.json");
    let bytes = match std::fs::read(&source) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if legacy
                .join("spaces")
                .try_exists()
                .map_err(|e| e.to_string())?
            {
                return Err("发现旧数据空间但缺少 settings.json，已停止接续以保留现场".into());
            }
            return Ok(false);
        }
        Err(error) => return Err(format!("读取旧安装自举失败：{error}")),
    };
    let mut config: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("旧安装自举损坏：{e}"))?;
    let app = config
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .ok_or("旧安装自举缺少 app 对象")?;
    // 旧默认根随应用标识改变；显式记录其原位置，现有空间、密文和自定义根均不移动。
    let configured = app.get("storageRoot").and_then(Value::as_str).unwrap_or("");
    let root = super::paths::resolve_root(configured, legacy);
    app.insert(
        "storageRoot".into(),
        Value::String(root.to_string_lossy().into_owned()),
    );
    let output = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    super::secure_store::replace_file(&destination, &output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adopts_old_root_without_moving_data_and_does_not_overwrite_new_settings() {
        let base = std::env::temp_dir().join(format!("covekit-brand-{}", uuid::Uuid::new_v4()));
        let legacy = base.join(LEGACY_APP_ID);
        let current = base.join("com.patchyx.covekit");
        std::fs::create_dir_all(&legacy).unwrap();
        let old =
            br#"{"app":{"storageRoot":"","activeSpaceId":"existing","spaces":{"existing":{}}}}"#;
        std::fs::write(legacy.join("settings.json"), old).unwrap();
        assert!(adopt_settings(&legacy, &current).unwrap());
        let target = current.join("settings.json");
        let adopted: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
        assert_eq!(
            adopted["app"]["storageRoot"],
            legacy.to_string_lossy().as_ref()
        );
        assert_eq!(adopted["app"]["activeSpaceId"], "existing");
        assert_eq!(std::fs::read(legacy.join("settings.json")).unwrap(), old);
        std::fs::write(&target, br#"{"app":{"theme":"dark"}}"#).unwrap();
        assert!(!adopt_settings(&legacy, &current).unwrap());
        assert!(std::fs::read_to_string(&target).unwrap().contains("dark"));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn corrupt_old_settings_do_not_create_an_empty_new_installation() {
        let base = std::env::temp_dir().join(format!("covekit-brand-bad-{}", uuid::Uuid::new_v4()));
        let legacy = base.join("old");
        let current = base.join("new");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("settings.json"), b"invalid").unwrap();
        assert!(adopt_settings(&legacy, &current).is_err());
        assert!(!current.join("settings.json").exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn custom_root_is_preserved_and_missing_bootstrap_does_not_hide_existing_spaces() {
        let base =
            std::env::temp_dir().join(format!("covekit-brand-custom-{}", uuid::Uuid::new_v4()));
        let legacy = base.join("old");
        let current = base.join("new");
        assert!(!adopt_settings(&legacy, &current).unwrap());
        std::fs::create_dir_all(legacy.join("spaces")).unwrap();
        assert!(adopt_settings(&legacy, &current).is_err());
        assert!(!current.join("settings.json").exists());

        let custom = base.join("custom-data");
        let config = serde_json::json!({"app": {"storageRoot": custom}});
        std::fs::write(
            legacy.join("settings.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        assert!(adopt_settings(&legacy, &current).unwrap());
        let adopted: Value =
            serde_json::from_slice(&std::fs::read(current.join("settings.json")).unwrap()).unwrap();
        assert_eq!(adopted, config);
        std::fs::remove_dir_all(base).unwrap();
    }
}
