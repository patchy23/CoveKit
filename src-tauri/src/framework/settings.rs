//! 设置模块：settings_get / settings_set / settings_patch / settings_set_tool / settings_revision
//! / preferences_get / preferences_set
//!
//! 契约见前端 `src/core/ipc/contracts.ts`（唯一事实源）。本模块负责：
//! - 字段级校验（枚举、类型、数字有限），错误信息可直接展示给用户；
//! - 读改写串行化 + 版本号（revision）校验：并发保存不同字段不会互相覆盖，陈旧写入被拒绝；
//! - 工具级设置按 owner/key 粒度更新，前端不再回传可能陈旧的整个 `tools` 对象；
//! - 系统副作用（开机自启）先执行、成功才落盘：失败不保存虚假状态，
//!   界面显示的即系统真实状态。
//!
//! 自举键（存储根、布局版本、迁移计划）由 `framework::storage` 管理，普通写入一律拒绝：
//! 它们带「重启后生效」语义，被通用接口改写会直接破坏换根约束。

use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

use crate::framework::preferences;

/// 设置对象在 settings.json 中的键
const APP_KEY: &str = "app";

/// 版本号键（顶层，保证不进入 app 设置对象）
const REVISION_KEY: &str = "settingsRevision";

/// 自举键：只能由 `framework::storage` 的计划/恢复流程与 `framework::space` 的空间解析写入
pub(crate) const RESERVED_KEYS: [&str; 5] = [
    "storageRoot",
    "layoutVersion",
    "pendingMigration",
    "lastMigration",
    // 活动空间标识（设备级）：本机空间选择，改名/覆盖会让数据位置整体改变，必须走空间解析入口
    "activeSpaceId",
];

/// 敏感字段名特征：普通设置文件是明文，不允许出现秘密材料
const SECRET_KEY_HINTS: [&str; 6] = [
    "password",
    "passphrase",
    "secret",
    "token",
    "apikey",
    "privatekey",
];

/// 设置读改写的进程内串行锁（IPC 并发调用不会交错读改写）
fn settings_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 读取设置：key 为空返回整个应用设置对象，否则返回指定字段
///
/// 归属两层的合并视图（任务书 §13.1）：空间级取本空间 `preferences.json`、缺失回落自举配置里的
/// 历史值（升级零迁移可读）；设备级只取自举配置。历史版本曾把字段写在 settings.json 顶层，
/// 这里保留读取回落，保证升级后旧值仍可见（写回时自动落到正确层）。
#[tauri::command]
pub fn settings_get(app: AppHandle, key: Option<String>) -> Result<Value, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let device = store.get(APP_KEY).unwrap_or_else(|| serde_json::json!({}));
    let space = preferences::read_current(&app)?;
    match key {
        None => Ok(preferences::merge_view(&device, &space)),
        Some(k) => {
            let value = preferences::read_view(&device, &space, &k);
            if !value.is_null() {
                return Ok(value);
            }
            Ok(store.get(&k).unwrap_or(Value::Null))
        }
    }
}

/// 当前设置版本号（前端保存时回传，用于拒绝陈旧覆盖）
#[tauri::command]
pub fn settings_revision(app: AppHandle) -> Result<u64, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    Ok(store
        .get(REVISION_KEY)
        .and_then(|v| v.as_u64())
        .unwrap_or(0))
}

/// 保存单个设置字段（校验 → 系统副作用 → 落盘），返回新的版本号
#[tauri::command]
pub fn settings_set(app: AppHandle, key: String, value: Value) -> Result<u64, String> {
    let mut patch = Map::new();
    patch.insert(key, value);
    settings_patch(app, None, patch)
}

/// 批量保存设置字段：一次读改写、一次落盘，附带的版本号不是最新值即拒绝
///
/// 拒绝语义：其他窗口/工具已改过设置时返回错误，前端重新读取后重试，
/// 避免「拿旧快照整体覆盖」把别人的修改抹掉。
#[tauri::command]
pub fn settings_patch(
    app: AppHandle,
    revision: Option<u64>,
    patch: Map<String, Value>,
) -> Result<u64, String> {
    let log_started = std::time::Instant::now();
    let result: Result<u64, String> = (|| {
        if patch.is_empty() {
            return settings_revision(app);
        }
        let _access = crate::framework::context::database_access()?;
        for (key, value) in &patch {
            validate_field(key, value)?;
        }
        let _guard = settings_lock().lock().map_err(|e| e.to_string())?;
        let store = app.store("settings.json").map_err(|e| e.to_string())?;
        let current_revision = store
            .get(REVISION_KEY)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if let Some(expected) = revision {
            if expected != current_revision {
                return Err(
                    "设置已被其他窗口或工具修改，已放弃本次保存以免覆盖新值，请重试".to_string(),
                );
            }
        }

        // 系统副作用先执行：失败即中止，不写入任何字段（避免出现「显示已开启但系统没生效」）
        for (key, value) in &patch {
            apply_side_effects(&app, key, value)?;
        }

        // 按归属拆两层（任务书 §13.1）：设备级进自举配置、空间级进本空间偏好文件。
        // 空间级先落盘：偏好写失败即整体失败，避免出现「界面提示成功、偏好其实没存」。
        let (device_patch, space_patch) = preferences::split_patch(&patch)?;
        if !space_patch.is_empty() {
            let mut current_space = preferences::read_current(&app)?;
            for (key, value) in &space_patch {
                // 工具级设置按 owner 合并，避免整对象覆盖丢掉别的窗口刚写入的字段
                if key == "tools" {
                    merge_tools(&mut current_space, value)?;
                    continue;
                }
                current_space.insert(key.clone(), value.clone());
            }
            preferences::write_current(&app, &current_space)?;
        }

        let mut current = store.get(APP_KEY).unwrap_or_else(|| serde_json::json!({}));
        let object = current.as_object_mut().ok_or_else(|| {
            "设置根对象损坏（app 不是对象），请在设置页恢复默认后重试".to_string()
        })?;
        for (key, value) in &device_patch {
            // 工具级设置走 settings_set_tool；批量写入 tools 时按 owner 合并，避免整对象覆盖
            if key == "tools" {
                merge_tools(object, value)?;
                continue;
            }
            object.insert(key.clone(), value.clone());
        }
        store.set(APP_KEY, current);
        // 版本号只在自举配置里维护一处，空间级写入也递增：并发窗口看到的是同一个计数器
        let next_revision = current_revision.saturating_add(1);
        store.set(REVISION_KEY, serde_json::json!(next_revision));
        store.save().map_err(|e| e.to_string())?;

        Ok(next_revision)
    })();
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=settings_patch elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=settings_patch elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 读某个工具设置（合并视图：空间级偏好优先、设备级历史值回落）
///
/// 插件**禁止**直读 `settings.json`：归属拆两层后直读只能看到设备层，
/// 换空间时会读到别的空间的值或直接读不到（FRP 的 `frpcPath`/`profileDir` 曾如此）。
pub fn tool_setting(app: &AppHandle, owner: &str, key: &str) -> Option<String> {
    let store = app.store("settings.json").ok()?;
    let device = store.get(APP_KEY).unwrap_or_else(|| serde_json::json!({}));
    let space = preferences::read_current(app).ok()?;
    let tools = preferences::merged_tools(&device, &space);
    tools.get(owner)?.get(key)?.as_str().map(String::from)
}

/// 工具级设置：按 owner/key 粒度更新（前端不发送整个 tools 对象）
#[tauri::command]
pub fn settings_set_tool(
    app: AppHandle,
    tool: String,
    key: String,
    value: Value,
) -> Result<u64, String> {
    let log_started = std::time::Instant::now();
    let result: Result<u64, String> = {
        let mut entry = Map::new();
        entry.insert(key, value);
        let mut tools = Map::new();
        tools.insert(tool, Value::Object(entry));
        let mut patch = Map::new();
        patch.insert("tools".to_string(), Value::Object(tools));
        settings_patch(app, None, patch)
    };
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=settings_set_tool elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=settings_set_tool elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 空间级用户数据键白名单：收藏与最近使用是**用户数据**而非设置项
///
/// 它们与设置一样随空间隔离（任务书 §13.1 冻结表），但写入**不递增设置版本号**：
/// 收藏一下就让设置页缓存的版本号过期，会导致随后一次设置保存被误判为陈旧而拒绝。
/// 白名单是显式的：若通用接口能写任意空间级键，等于绕开设置字段的归属划分。
const SPACE_DATA_KEYS: [&str; 2] = ["favorites", "recentTools"];

/// 校验空间级数据键（纯函数，便于用例直接覆盖）
pub(crate) fn validate_space_data_key(key: &str) -> Result<(), String> {
    if SPACE_DATA_KEYS.contains(&key) {
        return Ok(());
    }
    Err(format!(
        "{} 不是空间级用户数据，此处只接受 {}；设置字段请使用 settings_patch",
        key,
        SPACE_DATA_KEYS.join("、")
    ))
}

/// 读空间级用户数据（收藏、最近使用）
///
/// 只读当前空间的偏好文件，**不做设备层回落**：这两项在设备层没有历史值，
/// 回落会让非默认空间读到别处的数据（旧 `covekit.json` 里的收藏只属于默认空间）。
#[tauri::command]
pub fn preferences_get(app: AppHandle, key: String) -> Result<Value, String> {
    validate_space_data_key(&key)?;
    Ok(preferences::read_current(&app)?
        .get(&key)
        .cloned()
        .unwrap_or(Value::Null))
}

/// 写空间级用户数据：白名单 + 字段校验 → 合并进当前空间偏好文件（同目录原子替换）
#[tauri::command]
pub fn preferences_set(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = (|| {
        let _access = crate::framework::context::database_access()?;
        validate_space_data_key(&key)?;
        validate_field(&key, &value)?;
        let _guard = settings_lock().lock().map_err(|e| e.to_string())?;
        let mut space = preferences::read_current(&app)?;
        space.insert(key, value);
        preferences::write_current(&app, &space)
    })();
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=preferences_set elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=preferences_set elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 执行某字段的系统副作用（失败即返回错误，调用方不得落盘）
fn apply_side_effects(app: &AppHandle, key: &str, value: &Value) -> Result<(), String> {
    match key {
        "launchAtStartup" => match value.as_bool() {
            Some(true) => app
                .autolaunch()
                .enable()
                .map_err(|e| format!("开启开机自启失败（系统拒绝）: {e}")),
            Some(false) => app
                .autolaunch()
                .disable()
                .map_err(|e| format!("关闭开机自启失败: {e}")),
            None => Err("launchAtStartup 必须是布尔值".into()),
        },
        _ => Ok(()),
    }
}

/// 校验单个设置字段（错误信息可直接展示）
fn validate_field(key: &str, value: &Value) -> Result<(), String> {
    if RESERVED_KEYS.contains(&key) {
        return Err(format!(
            "设置项 {key} 由存储模块管理，请使用「存储位置」设置安排迁移或恢复动作"
        ));
    }
    if key == REVISION_KEY {
        return Err(format!("设置项 {key} 由应用维护，不可直接写入"));
    }
    if suggests_secret(key) {
        return Err(format!(
            "设置项 {key} 看起来是敏感信息，普通设置文件为明文，请改用凭证管理保存"
        ));
    }
    match key {
        "theme" => expect_enum(value, &["light", "dark", "system"], "主题"),
        "language" => expect_enum(value, &["zh-CN", "en-US"], "语言"),
        "launchAtStartup" => expect_type(value, Value::is_boolean, "开机自启必须是布尔值"),
        "defaultDownloadDirectory" => {
            expect_type(value, Value::is_string, "默认下载目录必须是字符串")
        }
        "recentTools" => expect_string_array(value, "最近使用工具"),
        "favorites" => expect_string_array(value, "收藏工具"),
        "tools" => expect_tools_object(value),
        _ => {
            reject_non_finite(value)?;
            Ok(())
        }
    }
}
/// 枚举字段校验
fn expect_enum(value: &Value, allowed: &[&str], label: &str) -> Result<(), String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("{label}必须是字符串，可选值：{}", allowed.join(" / ")))?;
    if allowed.contains(&text) {
        return Ok(());
    }
    Err(format!(
        "{label}取值非法（{text}），可选值：{}",
        allowed.join(" / ")
    ))
}

/// 类型校验
fn expect_type(value: &Value, check: fn(&Value) -> bool, message: &str) -> Result<(), String> {
    if check(value) {
        return Ok(());
    }
    Err(message.to_string())
}

/// 字符串数组校验
fn expect_string_array(value: &Value, label: &str) -> Result<(), String> {
    let items = value
        .as_array()
        .ok_or_else(|| format!("{label}必须是字符串数组"))?;
    if let Some(bad) = items.iter().find(|item| !item.is_string()) {
        return Err(format!("{label}只能包含字符串，发现：{bad}"));
    }
    Ok(())
}

/// 工具设置对象校验（owner → key/value 两级对象）
fn expect_tools_object(value: &Value) -> Result<(), String> {
    let owners = value
        .as_object()
        .ok_or_else(|| "工具设置必须是对象（工具 id → 设置键值）".to_string())?;
    for (owner, settings) in owners {
        if !settings.is_object() {
            return Err(format!("工具 {owner} 的设置必须是对象"));
        }
    }
    Ok(())
}

/// 敏感字段名识别（普通设置为明文，不接受秘密材料）
fn suggests_secret(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    SECRET_KEY_HINTS.iter().any(|hint| lowered.contains(hint))
}

/// 数字有限性检查（递归；JSON 传输层的 NaN/Infinity 会变成 null，由字段类型校验兜住）
fn reject_non_finite(value: &Value) -> Result<(), String> {
    match value {
        Value::Number(number) => match number.as_f64() {
            Some(v) if v.is_finite() => Ok(()),
            Some(_) => Err("设置数值必须是有限数".into()),
            None => Err("设置数值无法解析".into()),
        },
        Value::Array(items) => items.iter().try_for_each(reject_non_finite),
        Value::Object(map) => map.values().try_for_each(reject_non_finite),
        _ => Ok(()),
    }
}

/// 工具设置按 owner 逐键合并（不整对象覆盖，避免丢掉其他窗口刚写入的同 owner 字段）
fn merge_tools(target: &mut Map<String, Value>, incoming: &Value) -> Result<(), String> {
    let incoming = incoming
        .as_object()
        .ok_or_else(|| "工具设置必须是对象".to_string())?;
    let tools = target
        .entry("tools".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let tools = tools
        .as_object_mut()
        .ok_or_else(|| "已有的工具设置结构损坏（不是对象），请恢复默认后重试".to_string())?;
    for (owner, settings) in incoming {
        let settings = settings
            .as_object()
            .ok_or_else(|| format!("工具 {owner} 的设置必须是对象"))?;
        let entry = tools
            .entry(owner.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let entry = entry
            .as_object_mut()
            .ok_or_else(|| format!("工具 {owner} 的设置结构损坏（不是对象）"))?;
        for (key, value) in settings {
            entry.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}
/// 框架装配（`covekit_module!` 按模块调用本入口）：设置模块已无自有 State，
/// 命令入库与分派 handler 由 framework/mod.rs 的静态清单生成，故原样返回。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
}

/// 清理已移除的设置键（历史残留）
///
/// 这些键已不再被任何代码读取，长期留在设置文件里会被误认为仍然生效。
/// 工具级设置见 2026-09-14 的设置项缩减：四项改为写死或自动处理，对应键一并清除；
/// `frp.frpcPath` 是例外——二进制探测仍把它当作自定义路径的第一优先级，不能清。
fn prune_removed_settings(app_config: &mut Value) {
    if let Some(object) = app_config.as_object_mut() {
        object.remove("globalHotkey");
        object.remove("globalHotkeyActive");
    }
    // 工具级设置：键被清空后整个工具分区一并移除，不留空对象
    let removed: [(&str, &[&str]); 4] = [
        ("format-tools", &["indent"]),
        ("ssh", &["autoReconnect"]),
        ("frp", &["downloadMirror", "maxLogLines"]),
        ("database", &["agentMirror"]),
    ];
    let Some(tools) = app_config.get_mut("tools").and_then(Value::as_object_mut) else {
        return;
    };
    let mut emptied: Vec<&str> = Vec::new();
    for (tool, keys) in removed {
        let Some(entry) = tools.get_mut(tool).and_then(Value::as_object_mut) else {
            continue;
        };
        for key in keys {
            entry.remove(*key);
        }
        if entry.is_empty() {
            emptied.push(tool);
        }
    }
    for tool in emptied {
        tools.remove(tool);
    }
}

/// 插件启动初始化：核对设置与系统实际状态
///
/// 核对口径：以**系统实际状态**为准修正设置里的值，
/// 这样设置页显示的就是真实生效情况（自启被系统拒绝时不会显示「已开启」）。
pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let store = match tauri_plugin_store::StoreExt::store(app, "settings.json") {
        Ok(store) => store,
        Err(e) => {
            log::warn!(
                "设置文件打开失败，跳过启动核对: {e_type}",
                e_type = std::any::type_name_of_val(&e)
            );
            return Ok(());
        }
    };
    let mut current = store.get(APP_KEY).unwrap_or_else(|| serde_json::json!({}));

    let desired = current
        .get("launchAtStartup")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if let Ok(actual) = app.autolaunch().is_enabled() {
        if actual != desired {
            log::warn!("设置与实际状态不一致（设置 {desired}，系统 {actual}），按系统状态修正");
            if let Some(object) = current.as_object_mut() {
                object.insert("launchAtStartup".to_string(), serde_json::json!(actual));
            }
        }
    }

    prune_removed_settings(&mut current);
    store.set(APP_KEY, current);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空间级用户数据键只接受收藏与最近使用：其余键必须被拒，避免绕开设置字段的归属划分
    #[test]
    fn space_data_keys_are_allowlisted() {
        for key in SPACE_DATA_KEYS {
            assert!(validate_space_data_key(key).is_ok());
        }

        // 设置字段与自举键都不允许走这条路径（自举键改写会破坏换根约束）
        assert!(validate_space_data_key("theme").is_err());
        assert!(validate_space_data_key("tools").is_err());
        assert!(validate_space_data_key("storageRoot").is_err());
        assert!(validate_space_data_key("activeSpaceId").is_err());
        // 近似键同样拒绝：白名单是精确匹配，不是前缀匹配
        assert!(validate_space_data_key("favorites2").is_err());
        assert!(validate_space_data_key("Favorite").is_err());

        // 错误信息要说清「这里能写什么、设置该走哪条命令」，前端可直接展示
        let err = validate_space_data_key("theme").unwrap_err();
        assert!(err.contains("theme"));
        assert!(err.contains("favorites"));
        assert!(err.contains("settings_patch"));
    }

    /// 收藏与最近使用的值必须是字符串数组：类型错误当场报错，不落盘
    #[test]
    fn user_data_fields_expect_string_arrays() {
        assert!(validate_field("favorites", &serde_json::json!(["ssh"])).is_ok());
        assert!(validate_field("favorites", &serde_json::json!([])).is_ok());
        assert!(validate_field("recentTools", &serde_json::json!(["dns", "frp"])).is_ok());

        assert!(validate_field("favorites", &serde_json::json!("ssh")).is_err());
        assert!(validate_field("favorites", &serde_json::json!(["ssh", 3])).is_err());
        assert!(validate_field("recentTools", &serde_json::json!({"ssh": true})).is_err());
    }

    /// 这两项按空间级处理（不在设备级清单里）：换空间后各自独立
    #[test]
    fn user_data_stays_space_scoped() {
        for key in SPACE_DATA_KEYS {
            assert!(
                !preferences::is_device_key(key),
                "{key} 必须是空间级：设备级会让换空间后仍看到别处的收藏"
            );
        }
        // 对照：真正的设备级事实仍留在设备层
        assert!(preferences::is_device_key("storageRoot"));
    }

    /// 已移除的设置键在启动核对时被清掉，仍在使用的键与无关分区不受影响
    #[test]
    fn prune_removed_settings_drops_stale_keys_only() {
        let mut config = serde_json::json!({
            "theme": "light",
            "globalHotkey": "Ctrl+Shift+Space",
            "globalHotkeyActive": true,
            "tools": {
                "format-tools": { "indent": "4" },
                "ssh": { "autoReconnect": false, "idleDisconnectMinutes": "30" },
                "frp": { "downloadMirror": "https://ghfast.top/", "maxLogLines": 500, "frpcPath": "D:/frp/frpc.exe" },
                "database": { "agentMirror": "https://mirror.example.com/{type}" }
            }
        });

        prune_removed_settings(&mut config);

        assert!(config.get("globalHotkey").is_none(), "快捷键键应清除");
        assert!(
            config.get("globalHotkeyActive").is_none(),
            "只读生效值应清除"
        );
        assert_eq!(config.get("theme").and_then(Value::as_str), Some("light"));
        let tools = config
            .get("tools")
            .and_then(Value::as_object)
            .expect("tools 仍在");
        // 键被清空的分区整体移除
        assert!(
            tools.get("format-tools").is_none(),
            "缩进设置已写死，分区应移除"
        );
        assert!(tools.get("database").is_none(), "死配置分区应移除");
        let ssh = tools
            .get("ssh")
            .and_then(Value::as_object)
            .expect("ssh 分区");
        assert!(ssh.get("autoReconnect").is_none());
        assert_eq!(
            ssh.get("idleDisconnectMinutes").and_then(Value::as_str),
            Some("30")
        );
        let frp = tools
            .get("frp")
            .and_then(Value::as_object)
            .expect("frp 分区");
        assert!(frp.get("downloadMirror").is_none());
        assert!(frp.get("maxLogLines").is_none());
        assert!(frp.get("frpcPath").is_some(), "探测仍在读该键，不能清");
    }

    /// 枚举字段：合法值通过，非法值与类型不符都能给出可展示错误
    #[test]
    fn enum_fields_are_validated() {
        assert!(validate_field("theme", &serde_json::json!("dark")).is_ok());
        let error = validate_field("theme", &serde_json::json!("blue")).unwrap_err();
        assert!(error.contains("主题取值非法"), "{error}");
        // JSON 传输层的 NaN / 未定义会成为 null，这里由类型校验拒绝
        let null_error = validate_field("theme", &Value::Null).unwrap_err();
        assert!(null_error.contains("必须是字符串"), "{null_error}");
        assert!(validate_field("language", &serde_json::json!("en-US")).is_ok());
        assert!(validate_field("language", &serde_json::json!("ja-JP")).is_err());
    }

    /// 结构与类型校验
    #[test]
    fn structural_fields_are_validated() {
        assert!(validate_field("launchAtStartup", &serde_json::json!(true)).is_ok());
        assert!(validate_field("launchAtStartup", &serde_json::json!("yes")).is_err());
        assert!(validate_field("recentTools", &serde_json::json!(["dns", "ssh"])).is_ok());
        let array_error =
            validate_field("recentTools", &serde_json::json!(["dns", 7])).unwrap_err();
        assert!(array_error.contains("只能包含字符串"), "{array_error}");
        assert!(
            validate_field("tools", &serde_json::json!({"dns": {"platform": "dnspod"}})).is_ok()
        );
        let tools_error = validate_field("tools", &serde_json::json!({"dns": 3})).unwrap_err();
        assert!(tools_error.contains("必须是对象"), "{tools_error}");
    }

    /// 自举键、Rust 维护字段与敏感字段名都被通用写入拒绝
    #[test]
    fn internal_and_secret_keys_are_refused() {
        for key in [
            "storageRoot",
            "layoutVersion",
            "pendingMigration",
            "lastMigration",
        ] {
            let error = validate_field(key, &serde_json::json!("x")).unwrap_err();
            assert!(error.contains("存储模块管理"), "{key}: {error}");
        }
        assert!(validate_field("settingsRevision", &serde_json::json!(9)).is_err());
        for key in ["vaultPassword", "apiToken", "my_secret_note", "privateKey"] {
            let error = validate_field(key, &serde_json::json!("plain")).unwrap_err();
            assert!(error.contains("敏感信息"), "{key}: {error}");
        }
    }

    /// 工具设置按 owner 逐键合并：不同工具的并发写入互不丢失
    #[test]
    fn tools_merge_keeps_other_owners() {
        let mut target = serde_json::json!({"tools": {"dns": {"platform": "dnspod"}}});
        let object = target.as_object_mut().unwrap();
        merge_tools(object, &serde_json::json!({"ssh": {"keepAlive": 30}})).unwrap();
        merge_tools(
            object,
            &serde_json::json!({"dns": {"platform": "cloudflare"}}),
        )
        .unwrap();
        let tools = object.get("tools").unwrap();
        assert_eq!(tools["dns"]["platform"], serde_json::json!("cloudflare"));
        assert_eq!(tools["ssh"]["keepAlive"], serde_json::json!(30));
    }

    /// 数字有限性检查
    #[test]
    fn numbers_must_be_finite() {
        assert!(reject_non_finite(&serde_json::json!({"a": 1.5, "b": [2, 3]})).is_ok());
        let huge = serde_json::Number::from_f64(1.0e308).unwrap();
        assert!(reject_non_finite(&Value::Number(huge)).is_ok());
        // 非有限数在 JSON 里无法表示（序列化侧会退化为 null），此处确保检查本身不放过字符串伪装
        assert!(reject_non_finite(&serde_json::json!("NaN")).is_ok());
    }
}
