//! SSH 传输记录的校验、引用关系、身份与比较规则。

use super::DATASET_BOOKMARKS;
use super::DATASET_GROUPS;
use super::DATASET_PROFILES;
use super::DATASET_TUNNELS;
use super::EDGE_CREDENTIAL;
use super::EDGE_GROUP;
use super::EDGE_PROFILE;
use crate::framework::data_transfer::types::DependencyEdge;
use crate::framework::data_transfer::types::MergeContext;
use crate::plugins::ssh::models::ServerProfile;
use crate::plugins::ssh::models::SshBookmark;
use crate::plugins::ssh::models::SshGroup;
use crate::plugins::ssh::models::TunnelConfig;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// 记录体检（纯函数）：结构能反序列化回逻辑结构 + 关键字段非空
pub(super) fn validate(dataset: &str, records: &[Value]) -> Result<(), String> {
    match dataset {
        DATASET_PROFILES => {
            for record in records {
                let profile: ServerProfile = decode(record, "服务器档案")?;
                require("服务器档案", &profile.id, &profile.name)?;
                if profile.host.trim().is_empty() {
                    return Err(format!("服务器档案 {} 缺少主机地址", profile.id));
                }
                if profile.port == 0 {
                    return Err(format!("服务器档案 {} 端口非法（0）", profile.id));
                }
            }
        }
        DATASET_GROUPS => {
            for record in records {
                let group: SshGroup = decode(record, "服务器分组")?;
                require("服务器分组", &group.id, &group.name)?;
            }
        }
        DATASET_TUNNELS => {
            for record in records {
                let tunnel: TunnelConfig = decode(record, "隧道配置")?;
                require("隧道配置", &tunnel.id, &tunnel.name)?;
                if tunnel.profile_id.trim().is_empty() {
                    return Err(format!("隧道 {} 缺少所属档案", tunnel.id));
                }
                if tunnel.listen_port == 0 {
                    return Err(format!("隧道 {} 监听端口非法（0）", tunnel.id));
                }
            }
        }
        DATASET_BOOKMARKS => {
            for record in records {
                let bookmark: SshBookmark = decode(record, "目录书签")?;
                require("目录书签", &bookmark.id, &bookmark.name)?;
                if bookmark.profile_id.trim().is_empty() {
                    return Err(format!("书签 {} 缺少所属档案", bookmark.id));
                }
                if bookmark.path.trim().is_empty() {
                    return Err(format!("书签 {} 缺少远程路径", bookmark.id));
                }
            }
        }
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(())
}

/// 记录级引用（清单 `dependencies` 用）：档案 → 凭证/分组；隧道与书签 → 所属档案
pub(super) fn references(dataset: &str, records: &[Value]) -> Result<Vec<DependencyEdge>, String> {
    let mut edges = Vec::new();
    match dataset {
        DATASET_PROFILES => {
            for record in records {
                let profile: ServerProfile = decode(record, "服务器档案")?;
                if let Some(id) = non_empty(profile.credential_ref.as_deref()) {
                    edges.push(edge(EDGE_CREDENTIAL, &profile.id, id));
                }
                if let Some(id) = non_empty(profile.group_id.as_deref()) {
                    edges.push(edge(EDGE_GROUP, &profile.id, id));
                }
            }
        }
        DATASET_TUNNELS => {
            for record in records {
                let tunnel: TunnelConfig = decode(record, "隧道配置")?;
                edges.push(edge(EDGE_PROFILE, &tunnel.id, &tunnel.profile_id));
            }
        }
        DATASET_BOOKMARKS => {
            for record in records {
                let bookmark: SshBookmark = decode(record, "目录书签")?;
                edges.push(edge(EDGE_PROFILE, &bookmark.id, &bookmark.profile_id));
            }
        }
        DATASET_GROUPS => {}
        other => return Err(format!("SSH 适配器不支持数据集 {other}")),
    }
    Ok(edges)
}

/// 包内记录的身份与展示名
pub(super) fn record_identity(dataset: &str, record: &Value) -> Result<(String, String), String> {
    match dataset {
        DATASET_PROFILES => {
            let profile = decode::<ServerProfile>(record, "服务器档案")?;
            Ok((profile.id, profile.name))
        }
        DATASET_GROUPS => {
            let group = decode::<SshGroup>(record, "服务器分组")?;
            Ok((group.id, group.name))
        }
        DATASET_TUNNELS => {
            let tunnel = decode::<TunnelConfig>(record, "隧道配置")?;
            Ok((tunnel.id, tunnel.name))
        }
        DATASET_BOOKMARKS => {
            let bookmark = decode::<SshBookmark>(record, "目录书签")?;
            Ok((bookmark.id, bookmark.name))
        }
        other => Err(format!("SSH 适配器不支持数据集 {other}")),
    }
}

/// 包内记录的业务键（合并判定的第三条命中路径）
pub(super) fn business_key(
    dataset: &str,
    record: &Value,
    core: &MergeContext<'_>,
) -> Result<Option<String>, String> {
    match dataset {
        DATASET_PROFILES => Ok(Some(decode::<ServerProfile>(record, "服务器档案")?.name)),
        DATASET_GROUPS => Ok(Some(decode::<SshGroup>(record, "服务器分组")?.name)),
        DATASET_TUNNELS => Ok(Some(decode::<TunnelConfig>(record, "隧道配置")?.name)),
        DATASET_BOOKMARKS => {
            let bookmark = decode::<SshBookmark>(record, "目录书签")?;
            // 业务键 = 档案名 + 路径：档案名从包内档案记录解析（档案没进包则键缺失，按插入处理）
            let Some(profiles) = core.source_records.get(DATASET_PROFILES) else {
                return Ok(None);
            };
            let mut name = None;
            for item in profiles {
                let profile = decode::<ServerProfile>(item, "服务器档案")?;
                if profile.id == bookmark.profile_id {
                    name = Some(profile.name);
                    break;
                }
            }
            Ok(name.map(|name| format!("{name}::{}", bookmark.path)))
        }
        other => Err(format!("SSH 适配器不支持数据集 {other}")),
    }
}

/// 同一性比较前的归一：凭证引用永不随包（导入后一律置空待补录），不参与「内容相同」判定
pub(super) fn normalize_compare(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(map) = value.as_object_mut() {
        map.remove("credentialRef");
    }
    value
}

/// 反序列化回逻辑结构：失败即「结构不认识」，报错而不是跳过（跳过等于静默丢记录）
pub(super) fn decode<T: DeserializeOwned>(record: &Value, label: &str) -> Result<T, String> {
    serde_json::from_value(record.clone()).map_err(|e| format!("{label}记录结构不认识: {e}"))
}

/// id 与名称必须非空（导入侧靠 id 建引用，靠名称在界面里区分条目）
fn require(label: &str, id: &str, name: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err(format!("{label}记录缺少 id"));
    }
    if name.trim().is_empty() {
        return Err(format!("{label} {id} 缺少名称"));
    }
    Ok(())
}

/// 构造依赖边（`fromId` 引用 `toId`；被引用的数据集由描述符的 `pulls` 按 `kind` 决定）
pub(super) fn edge(kind: &str, from_id: &str, to_id: &str) -> DependencyEdge {
    DependencyEdge {
        kind: kind.to_string(),
        from_id: from_id.to_string(),
        to_id: to_id.to_string(),
    }
}

/// 取非空引用（空串 = 未设置，不产生引用边）
pub(super) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|item| !item.is_empty())
}
