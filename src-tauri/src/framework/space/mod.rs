//! 框架 · 数据空间（namespace）解析（sync-202609-001 · L1 骨架）
//!
//! 语义约定（改动前先读这里）：
//! - **唯一来源**：本模块是「当前空间」的唯一解析入口。取数据位置、凭证、偏好、密钥都必须
//!   经 `context`（其空间标识由本模块给出）；模块不得自建空间 id、不得把账号 id 当空间 id。
//! - **默认空间零迁移（红线）**：`default` 是「尚未建立空间索引的旧布局」的兼容承载位，
//!   位置形态保持 `LegacyFlat`：路径与密钥条目与升级前逐字符相同，本批不建空间目录。
//! - **不静默兜底**：标识非法时回落 `default` 保证旧数据仍可读，但必须留下可见登记
//!   （日志 + `fallback()`），不得静默改写用户配置，也不得新建空环境冒充成功。
//! - **越界零容忍**：空间 id 只接受小写 UUIDv4 或兼容承载位 `default`，作为目录名与 keyring
//!   service 片段之前必须校验（禁路径穿越与名称注入）。
//! - 本批只放置默认空间：不做新建/切换/重命名/删除，也没有空间索引与待激活状态。

use std::path::Path;
use std::sync::OnceLock;

use tauri::AppHandle;

use super::context::{StorageLocation, DEFAULT_GENERATION_ID, DEFAULT_SPACE_ID};
use super::paths;

/// 活动空间标识配置键（`settings.json` 的 `app` 对象内，设备级）
///
/// 归属见任务书 §13.1：本机空间列表与当前选择**绝不进包、不随空间切换**。
pub const KEY_ACTIVE_SPACE_ID: &str = "activeSpaceId";

/// 空间回落登记（诊断用：标识不可用时回落到默认空间这件事本身）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceFallback {
    /// 配置里的原始值（截断后，避免把异常长内容写进日志与界面）
    pub raw: String,
    /// 回落原因（面向用户的一句话，不含秘密）
    pub reason: String,
}

/// 空间解析结果：本批只有默认空间，但取值路径与后续批次一致（不写死字面量）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceResolution {
    /// 空间标识（校验通过的值；回落时为 `default`）
    pub space_id: String,
    /// 空间代际（真实代际由空间激活流程给出，本批固定为 1）
    pub generation_id: u64,
    /// 本次解析是否发生回落（None = 正常）
    pub fallback: Option<SpaceFallback>,
}

impl SpaceResolution {
    /// 默认空间（无回落）
    pub fn default_space() -> Self {
        Self {
            space_id: DEFAULT_SPACE_ID.to_string(),
            generation_id: DEFAULT_GENERATION_ID,
            fallback: None,
        }
    }

    /// 是否为兼容承载位（旧扁平布局：数据根就是设备根）
    pub fn is_legacy_default(&self) -> bool {
        self.space_id == DEFAULT_SPACE_ID
    }
}

/// 空间 id 校验：兼容承载位 `default` 或小写 UUIDv4（任务书 §13.2 冻结）
///
/// 只做形态校验，不做存在性检查：是否存在由空间索引/激活流程负责（后续批次）。
pub fn is_valid_space_id(id: &str) -> bool {
    if id == DEFAULT_SPACE_ID {
        return true;
    }
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let ok = match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        };
        if !ok {
            return false;
        }
    }
    // 版本位必须是 4（UUIDv4），变体位限 8/9/a/b（RFC 4122），避免把 v1/v7 混进来
    matches!(bytes[14], b'4') && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

/// 纯解析：把自举配置里的原始值解析为空间身份（无副作用，便于用例覆盖三种情形）
///
/// `raw` 为 `None` 表示键缺失（旧安装），`Some` 为配置值（可能为空串或非法值）。
pub fn resolve_value(raw: Option<&str>) -> SpaceResolution {
    let value = raw.map(|text| text.trim().to_string());
    match value {
        Some(value) if is_valid_space_id(&value) => SpaceResolution {
            space_id: value,
            generation_id: DEFAULT_GENERATION_ID,
            fallback: None,
        },
        Some(value) => {
            let fallback = SpaceFallback {
                raw: truncate(&value, 64),
                reason: "活动空间标识非法（应为小写 UUIDv4 或 default），已回落到默认空间；\
                         请检查自举配置里的 activeSpaceId"
                    .into(),
            };
            SpaceResolution {
                space_id: DEFAULT_SPACE_ID.to_string(),
                generation_id: DEFAULT_GENERATION_ID,
                fallback: Some(fallback),
            }
        }
        None => SpaceResolution::default_space(),
    }
}

/// 解析当前活动空间（读设备级自举配置；缺失时写入默认空间标识）
///
/// 三种情形区别对待，不做「什么都当默认空间」的静默处理：
/// - 键缺失：视作尚未建立空间标识的旧安装，写入 `default` 承载位后按默认空间运行；
/// - 值非法：**不改写用户配置**，回落默认空间并留下可见登记（数据仍可读，问题可见）；
/// - 值合法：按值解析。
pub fn resolve_active(app: &AppHandle) -> SpaceResolution {
    let raw = paths::read_setting(app, KEY_ACTIVE_SPACE_ID)
        .and_then(|value| value.as_str().map(|text| text.to_string()));
    let resolution = resolve_value(raw.as_deref());
    if let Some(fallback) = resolution.fallback.as_ref() {
        record_fallback(fallback);
    }
    if raw.is_none() {
        // 自举初始化：写入承载位。写失败不阻断启动（只影响下次解析），但必须可见。
        if let Err(error) = record_default(app) {
            eprintln!("[space] 写入默认空间标识失败: {error}");
        }
    }
    resolution
}

/// 首次启动写入默认空间标识（设备级自举配置；已存在时不覆盖）
pub fn record_default(app: &AppHandle) -> Result<(), String> {
    if paths::read_setting(app, KEY_ACTIVE_SPACE_ID).is_some() {
        return Ok(());
    }
    paths::write_setting(
        app,
        KEY_ACTIVE_SPACE_ID,
        serde_json::json!(DEFAULT_SPACE_ID),
    )
}

/// 按解析结果构造本次启动**唯一**的存储位置描述符
///
/// 默认空间保持 `LegacyFlat`（红线：本批不搬动真实数据）；其它空间按任务书 §13.3 的分区
/// 形态落位。纯计算，不建目录、不触碰文件系统。
pub fn location_for(device_root: &Path, resolution: &SpaceResolution) -> StorageLocation {
    if resolution.is_legacy_default() {
        return StorageLocation::legacy_for(device_root.to_path_buf(), &resolution.space_id);
    }
    StorageLocation::partitioned(
        device_root.to_path_buf(),
        &resolution.space_id,
        resolution.generation_id,
    )
}

/// 本次启动是否发生了空间回落（进程内登记；None = 正常）
pub fn fallback() -> Option<SpaceFallback> {
    FALLBACK.get().cloned().flatten()
}

/// 记录回落（只登记首因：一次启动里为什么回落到默认空间，首个原因最接近根因）
fn record_fallback(fallback: &SpaceFallback) {
    eprintln!(
        "[space] 活动空间标识非法，已回落默认空间：raw={} reason={}",
        fallback.raw, fallback.reason
    );
    let _ = FALLBACK.set(Some(fallback.clone()));
}

/// 截断展示用文本（按字符截，避免把多字节字符切坏）
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}…")
}

/// 当前空间标识（上下文未初始化时按默认空间：仅测试或框架极早期会出现）
pub fn current_id() -> String {
    super::context::current()
        .map(|ctx| ctx.space_id().to_string())
        .unwrap_or_else(|| DEFAULT_SPACE_ID.to_string())
}

/// 当前空间的主密钥库访问入口（service 按空间作用域，account 不变）。
///
/// 约定：**不允许**业务模块自建 `ScopedKeyringStore` 传别的 service —— 那等于绕过空间隔离。
/// 密钥库与本地降级密钥文件必须同时归属同一空间，只做其中一层等于没做隔离。
pub(crate) fn keyring_store() -> super::secure_store::ScopedKeyringStore {
    super::secure_store::keyring_store_for(&current_id())
}

/// 进程内回落登记（启动时确定一次）
static FALLBACK: OnceLock<Option<SpaceFallback>> = OnceLock::new();

#[cfg(test)]
#[path = "../space_tests.rs"]
mod space_tests;
