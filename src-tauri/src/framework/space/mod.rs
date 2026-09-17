//! 框架 · 数据空间解析（空间身份 = 全局唯一 uid）
//!
//! 语义约定（改动前先读这里）：
//! - **唯一来源**：本模块是「当前空间」的唯一解析入口。取数据位置、凭证、偏好、密钥都必须
//!   经 `context`（其空间标识由本模块给出）；模块不得自建空间 id、不得把账号 id 当空间 id。
//! - **uid 由首启自举生成**（`bootstrap` 子模块）：一次生成、永久不变；本模块只读不写，
//!   不在解析路径上临时发明身份。
//! - **不静默兜底**：标识非法时回落到索引里标记的默认空间，但必须留下可见登记
//!   （日志 + `fallback()`）；索引里连默认条目都没有 = 自举损坏，直接报错（可见恢复），
//!   不得改写用户配置，也不得新建空环境冒充成功。
//! - **越界零容忍**：空间 id 只接受小写 UUIDv4，作为目录名与 keyring service 片段之前
//!   必须校验（禁路径穿越与名称注入）。

use std::path::Path;
use std::sync::OnceLock;

use tauri::AppHandle;

use super::context::StorageLocation;
use super::paths;

pub mod bootstrap;
pub mod index;

pub use index::display_name;

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

/// 空间解析结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceResolution {
    /// 空间标识（校验通过的 uid；回落时为索引里的默认空间 uid）
    pub space_id: String,
    /// 本次解析是否发生回落（None = 正常）
    pub fallback: Option<SpaceFallback>,
}

/// 空间 id 校验：只接受小写 UUIDv4（版本位 4、变体位 8/9/a/b，RFC 4122）
///
/// 只做形态校验，不做存在性检查：是否存在由空间索引/激活流程负责。
pub fn is_valid_space_id(id: &str) -> bool {
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
/// - `raw` 为 `None` 表示键缺失（迁移未跑或自举被清），`Some` 为配置值（可能为空串或非法值）；
/// - `default_uid` 是索引里标记为默认的空间 uid（回落目标）；连它都没有 = 自举损坏，报错。
pub fn resolve_value(
    raw: Option<&str>,
    default_uid: Option<&str>,
) -> Result<SpaceResolution, String> {
    let value = raw.map(|text| text.trim().to_string());
    match value {
        Some(value) if is_valid_space_id(&value) => Ok(SpaceResolution {
            space_id: value,
            fallback: None,
        }),
        Some(value) => match default_uid {
            Some(uid) => Ok(SpaceResolution {
                space_id: uid.to_string(),
                fallback: Some(SpaceFallback {
                    raw: truncate(&value, 64),
                    reason: "活动空间标识非法（应为小写 UUIDv4），已回落到默认空间；\
                             请检查自举配置里的 activeSpaceId"
                        .into(),
                }),
            }),
            None => Err("活动空间标识非法，且空间索引中没有默认空间条目（自举损坏）".into()),
        },
        None => match default_uid {
            Some(uid) => Ok(SpaceResolution {
                space_id: uid.to_string(),
                fallback: Some(SpaceFallback {
                    raw: String::new(),
                    reason: "自举配置缺少活动空间标识，已回落到默认空间".into(),
                }),
            }),
            None => Err("自举配置缺少活动空间标识，且空间索引中没有默认空间条目".into()),
        },
    }
}

/// 解析当前活动空间（读设备级自举配置；回落目标取索引里的默认空间条目）
pub fn resolve_active(app: &AppHandle) -> Result<SpaceResolution, String> {
    let raw = index::read_active_id(app);
    let default_uid = index::read_index(app)?
        .into_iter()
        .find(|(_, record)| record.is_default)
        .map(|(id, _)| id);
    let resolution = resolve_value(raw.as_deref(), default_uid.as_deref())?;
    if let Some(fallback) = resolution.fallback.as_ref() {
        record_fallback(fallback);
    }
    Ok(resolution)
}

/// 写入活动空间标识（设备级自举配置；由空间迁移与空间切换调用，业务模块不得调用）
pub fn record_active(app: &AppHandle, space_id: &str) -> Result<(), String> {
    if !is_valid_space_id(space_id) {
        return Err(format!("空间 id 非法，拒绝写入自举配置：{space_id}"));
    }
    paths::write_setting(app, KEY_ACTIVE_SPACE_ID, serde_json::json!(space_id))
}

/// 按解析结果构造本次启动**唯一**的存储位置描述符。
///
/// 所有空间统一为 `spaces/<uid>/` 布局（无代际层）。纯计算，不建目录、不触碰文件系统。
pub fn location_for(device_root: &Path, resolution: &SpaceResolution) -> StorageLocation {
    StorageLocation::for_space(device_root.to_path_buf(), &resolution.space_id)
}

/// 本次启动是否发生了空间回落（进程内登记；None = 正常）
pub fn fallback() -> Option<SpaceFallback> {
    FALLBACK.get().cloned().flatten()
}

/// 记录回落（只登记首因：一次启动里为什么回落到默认空间，首个原因最接近根因）
fn record_fallback(fallback: &SpaceFallback) {
    eprintln!(
        "[space] 活动空间标识不可用，已回落默认空间：raw={} reason={}",
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

/// 当前空间标识（数据上下文之外没有第二处来源；未初始化即错误——测试须先 init）
pub fn current_id() -> Result<String, String> {
    super::context::current()
        .map(|ctx| ctx.space_id().to_string())
        .ok_or_else(|| "数据上下文未初始化，当前空间 uid 不可得".to_string())
}

/// 当前空间的主密钥库访问入口（service 按空间作用域，account 不变）。
///
/// 约定：**不允许**业务模块自建 `ScopedKeyringStore` 传别的 service —— 那等于绕过空间隔离。
/// 密钥库与本地降级密钥文件必须同时归属同一空间，只做其中一层等于没做隔离。
pub(crate) fn keyring_store() -> Result<super::secure_store::ScopedKeyringStore, String> {
    Ok(super::secure_store::keyring_store_for(&current_id()?))
}

/// 进程内回落登记（启动时确定一次）
static FALLBACK: OnceLock<Option<SpaceFallback>> = OnceLock::new();

#[cfg(test)]
#[path = "../space_tests.rs"]
mod space_tests;
