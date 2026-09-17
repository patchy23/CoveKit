//! 可导出目录与选择集闭包（sync-202609-001 L2 · 导出链路）
//!
//! 分工：适配器说「我有什么、按 id 给你什么」，本模块只说「用户勾的和它带出来的一共是什么」。
//! 依赖闭包（凭证、分组、隧道、书签）在这里一次性展开，前端不做第二次计算——两份计算必然会不一致。
//!
//! 三条不变量：
//! - 目录里没有的 id 一律拒绝（列表过期就报错让用户刷新，不静默少带）；
//! - 进包条数必须与闭包解析结果一致（多带或漏带都是缺陷，不是「尽力而为」）；
//! - 缺记录体只允许出现在 `device-local` 与未勾选带出的 `secret` 上（§13.1）。

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use serde_json::Value;
use tauri::AppHandle;

use super::adapter::{self, DatasetAdapter};
use super::types::{
    block_carrying, block_declared, validate_manifest, DatasetDescriptor, ExportCatalog,
    ExportSelection, PackageManifest, ResolvedSelection, TransportPolicy,
};
use crate::framework::space;

/// 收集全部适配器声明的数据集（按数据集名排序，保证导出顺序稳定）
///
/// 数据集名跨 owner 重名属于装配缺陷：这里直接报错，不静默覆盖。
pub(crate) fn collect_descriptors(app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
    let mut all: Vec<DatasetDescriptor> = Vec::new();
    for owner_adapter in adapter::all() {
        let described = owner_adapter.describe_datasets(app)?;
        for descriptor in described {
            if descriptor.owner != owner_adapter.owner() {
                return Err(format!(
                    "数据集 {} 的 owner（{}）与其适配器（{}）不一致",
                    descriptor.name,
                    descriptor.owner,
                    owner_adapter.owner()
                ));
            }
            if all.iter().any(|item| item.name == descriptor.name) {
                return Err(format!("数据集 {} 被多个适配器声明", descriptor.name));
            }
            if !descriptor.entries.is_empty() && descriptor.record_count != descriptor.entries.len()
            {
                return Err(format!(
                    "数据集 {} 的 recordCount（{}）与条目数（{}）不一致",
                    descriptor.name,
                    descriptor.record_count,
                    descriptor.entries.len()
                ));
            }
            all.push(descriptor);
        }
    }
    all.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(all)
}

/// 构造导出目录：当前空间可导出的数据集、可勾选条目与默认选择
pub(crate) fn build_catalog(app: &AppHandle) -> Result<ExportCatalog, String> {
    let descriptors = collect_descriptors(app)?;
    let space_id = space::current_id()?;
    let mut entries: Vec<_> = descriptors
        .iter()
        .flat_map(|descriptor| descriptor.entries.iter().cloned())
        .collect();
    entries.sort_by(|a, b| {
        a.dataset
            .cmp(&b.dataset)
            .then_with(|| a.label.cmp(&b.label))
            .then_with(|| a.id.cmp(&b.id))
    });
    let datasets: Vec<_> = descriptors.iter().map(|item| item.summary()).collect();

    let mut warnings: Vec<String> = Vec::new();
    let device_local: Vec<&str> = descriptors
        .iter()
        .filter(|item| item.policy == TransportPolicy::DeviceLocal)
        .map(|item| item.label.as_str())
        .collect();
    if !device_local.is_empty() {
        warnings.push(format!(
            "以下类别属于本机事实，不随包搬移：{}",
            device_local.join("、")
        ));
    }
    if !descriptors.iter().any(|item| item.selectable) {
        warnings.push("当前空间没有可勾选的记录，无法导出数据包".into());
    }

    Ok(ExportCatalog {
        source_space_id: space_id.clone(),
        source_space_name: space::display_name(app, &space_id),
        datasets,
        entries,
        defaults: default_selection(&descriptors),
        warnings,
    })
}

/// 默认选择：整块数据集按声明默认勾选（收藏默认勾、最近使用默认不勾），按条数据集默认不勾
pub(crate) fn default_selection(descriptors: &[DatasetDescriptor]) -> ExportSelection {
    ExportSelection {
        entries: Vec::new(),
        datasets: descriptors
            .iter()
            .filter(|item| item.selectable && item.default_selected)
            .map(|item| item.name.clone())
            .collect(),
        include_credentials: true,
    }
}

/// 把用户选择展开为依赖闭包（纯函数；含全部单测覆盖）
///
/// 规则：
/// 1. 整块勾选只接受声明 `selectable` 的数据集；
/// 2. 按条勾选必须命中目录里真实存在的 id（列表过期立即报错，不静默丢记录）；
/// 3. 跟随关系反复展开到不动点（跟随出来的条目可能再引出新的跟随）；
/// 4. 勾选为空直接拒绝，不生成「空包也算成功」的结果。
pub(crate) fn resolve_selection(
    descriptors: &[DatasetDescriptor],
    selection: &ExportSelection,
) -> Result<ResolvedSelection, String> {
    let by_name: BTreeMap<&str, &DatasetDescriptor> = descriptors
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect();
    let mut sets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for name in &selection.datasets {
        let Some(descriptor) = by_name.get(name.as_str()) else {
            return Err(format!("未知数据集：{name}"));
        };
        if !descriptor.selectable {
            return Err(format!(
                "数据集 {name} 不能单独勾选{}",
                descriptor
                    .note
                    .as_deref()
                    .map(|note| format!("（{note}）"))
                    .unwrap_or_default()
            ));
        }
        sets.entry(name.clone()).or_default();
    }

    for picked in &selection.entries {
        let Some(descriptor) = by_name.get(picked.dataset.as_str()) else {
            return Err(format!("未知数据集：{}", picked.dataset));
        };
        if !descriptor.selectable {
            return Err(format!(
                "数据集 {} 不能单独勾选{}",
                picked.dataset,
                descriptor
                    .note
                    .as_deref()
                    .map(|note| format!("（{note}）"))
                    .unwrap_or_default()
            ));
        }
        let slot = sets.entry(descriptor.name.clone()).or_default();
        for id in &picked.ids {
            if !descriptor.entries.iter().any(|entry| entry.id == *id) {
                return Err(format!(
                    "数据集 {} 中不存在记录 {id}（列表可能已过期，请刷新后重选）",
                    descriptor.name
                ));
            }
            slot.insert(id.clone());
        }
    }

    if sets.is_empty() {
        return Err("没有选择任何可导出的数据".into());
    }

    // 闭包展开：每轮把「已进包条目」的依赖边按跟随关系并入目标数据集
    loop {
        let snapshot: Vec<(String, Vec<String>)> = sets
            .iter()
            .map(|(name, ids)| (name.clone(), ids.iter().cloned().collect()))
            .collect();
        let mut added = false;
        for (name, ids) in snapshot {
            let Some(descriptor) = by_name.get(name.as_str()) else {
                continue;
            };
            if descriptor.pulls.is_empty() {
                continue;
            }
            for id in ids {
                let Some(entry) = descriptor.entries.iter().find(|item| item.id == id) else {
                    continue;
                };
                for edge in &entry.dependencies {
                    let Some(pull) = descriptor.pulls.iter().find(|item| item.kind == edge.kind)
                    else {
                        continue;
                    };
                    if !by_name.contains_key(pull.dataset.as_str()) {
                        return Err(format!(
                            "数据集 {} 声明的跟随目标 {} 不存在",
                            descriptor.name, pull.dataset
                        ));
                    }
                    if sets
                        .entry(pull.dataset.clone())
                        .or_default()
                        .insert(edge.to_id.clone())
                    {
                        added = true;
                    }
                }
            }
        }
        if !added {
            break;
        }
    }

    Ok(ResolvedSelection {
        datasets: sets
            .into_iter()
            .map(|(name, ids)| (name, ids.into_iter().collect()))
            .collect(),
        include_credentials: selection.include_credentials,
    })
}

/// 按选择产出可写入包的清单（导出前的唯一入口）
///
/// 每次调用都重新读目录：目录是当前空间的快照，选择集里的 id 必须能在同一次调用里被兑现。
pub(crate) fn build_manifest(
    app: &AppHandle,
    selection: &ExportSelection,
) -> Result<PackageManifest, String> {
    let descriptors = collect_descriptors(app)?;
    let resolved = resolve_selection(&descriptors, selection)?;
    let adapters: BTreeMap<&str, &'static dyn DatasetAdapter> = adapter::all()
        .into_iter()
        .map(|item| (item.owner(), item))
        .collect();
    let space_id = space::current_id()?;
    let mut manifest = PackageManifest::new(&space_id, &space::display_name(app, &space_id));

    for (name, ids) in &resolved.datasets {
        let Some(descriptor) = descriptors.iter().find(|item| item.name == *name) else {
            return Err(format!("数据集 {name} 在清单装配时消失（目录不一致）"));
        };
        let Some(owner_adapter) = adapters.get(descriptor.owner.as_str()) else {
            return Err(format!(
                "数据集 {name} 的 owner（{}）没有登记适配器",
                descriptor.owner
            ));
        };
        // 只声明不携带：设备级事实一律如此；凭证在用户未勾选带出时同样只声明
        if descriptor.policy == TransportPolicy::DeviceLocal
            || (descriptor.policy == TransportPolicy::Secret && !resolved.include_credentials)
        {
            manifest
                .datasets
                .push(block_declared(descriptor, ids.len()));
            continue;
        }
        let records = owner_adapter.export_records(app, name, ids)?;
        owner_adapter.validate_records(name, &records)?;
        if !descriptor.entries.is_empty() && records.len() != ids.len() {
            return Err(format!(
                "数据集 {name} 实际导出 {} 条，与选择集 {} 条不一致（记录可能在选择期间被删除，请刷新后重试）",
                records.len(),
                ids.len()
            ));
        }
        manifest
            .dependencies
            .extend(owner_adapter.enumerate_references(name, &records)?);
        manifest.datasets.push(block_carrying(descriptor, records)?);
    }

    // 显式排除项：目录里有候选记录、本次没进包的类别（界面据此告知「没有带出什么」）
    manifest.excluded = descriptors
        .iter()
        .filter(|item| {
            !resolved.includes(&item.name) && (item.selectable || !item.entries.is_empty())
        })
        .map(|item| item.name.clone())
        .collect();
    manifest.dependencies.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.from_id.cmp(&b.from_id))
            .then_with(|| a.to_id.cmp(&b.to_id))
    });
    manifest.dependencies.dedup();

    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// 预览用的条数统计：数据集名 → 计划进包条数（含只声明不携带的类别）
///
/// 前端只在提交前用它做确认文案；真正的条数以导出结果为准，两者由同一闭包解析得出。
#[cfg(test)]
pub(crate) fn planned_counts(resolved: &ResolvedSelection) -> BTreeMap<String, usize> {
    resolved
        .datasets
        .iter()
        .map(|(name, ids)| (name.clone(), ids.len()))
        .collect()
}

/// 从记录体里取出某字段的字符串值（适配器与导入侧共用的小工具）
#[cfg(test)]
pub(crate) fn text_field(record: &Value, field: &str) -> Option<String> {
    record
        .get(field)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::data_transfer::types::{CatalogEntry, DatasetPull, DependencyEdge};

    /// 造一个条目
    fn entry(dataset: &str, id: &str, deps: Vec<(&str, &str)>) -> CatalogEntry {
        CatalogEntry {
            dataset: dataset.into(),
            id: id.into(),
            label: format!("{dataset}-{id}"),
            detail: String::new(),
            dependencies: deps
                .into_iter()
                .map(|(kind, to)| DependencyEdge {
                    kind: kind.into(),
                    from_id: id.into(),
                    to_id: to.into(),
                })
                .collect(),
            note: None,
        }
    }

    /// 造一个数据集
    fn descriptor(
        name: &str,
        policy: TransportPolicy,
        selectable: bool,
        pulls: Vec<(&str, &str)>,
        entries: Vec<CatalogEntry>,
    ) -> DatasetDescriptor {
        let record_count = entries.len();
        DatasetDescriptor {
            name: name.into(),
            label: name.into(),
            owner: "test".into(),
            policy,
            schema_version: 1,
            selectable,
            contains_secret: policy == TransportPolicy::Secret,
            default_selected: false,
            pulls: pulls
                .into_iter()
                .map(|(kind, dataset)| DatasetPull {
                    kind: kind.into(),
                    dataset: dataset.into(),
                })
                .collect(),
            note: None,
            record_count,
            entries,
        }
    }

    /// 一份覆盖「档案 + 分组 + 隧道 + 凭证」的最小目录
    fn catalog_fixture() -> Vec<DatasetDescriptor> {
        vec![
            descriptor(
                "ssh.profiles",
                TransportPolicy::Portable,
                true,
                vec![
                    ("credential", "vault.credentials"),
                    ("group", "ssh.groups"),
                    ("tunnel", "ssh.tunnels"),
                ],
                vec![
                    entry(
                        "ssh.profiles",
                        "p1",
                        vec![("credential", "c1"), ("group", "g1")],
                    ),
                    entry(
                        "ssh.profiles",
                        "p2",
                        vec![("credential", "c1"), ("tunnel", "t1")],
                    ),
                    entry("ssh.profiles", "p3", vec![]),
                ],
            ),
            descriptor(
                "ssh.groups",
                TransportPolicy::Portable,
                false,
                vec![],
                vec![entry("ssh.groups", "g1", vec![])],
            ),
            descriptor(
                "ssh.tunnels",
                TransportPolicy::Portable,
                false,
                vec![],
                vec![entry("ssh.tunnels", "t1", vec![])],
            ),
            descriptor(
                "vault.credentials",
                TransportPolicy::Secret,
                false,
                vec![],
                vec![
                    entry("vault.credentials", "c1", vec![]),
                    entry("vault.credentials", "c2", vec![]),
                ],
            ),
            descriptor(
                "core.favorites",
                TransportPolicy::Portable,
                true,
                vec![],
                vec![],
            ),
        ]
    }

    /// 只勾一个档案：分组与凭证随引用带出，未被引用的凭证与隧道不带
    #[test]
    fn closure_pulls_referenced_only() {
        let descriptors = catalog_fixture();
        let selection = ExportSelection {
            entries: vec![super::super::types::SelectionEntry {
                dataset: "ssh.profiles".into(),
                ids: vec!["p1".into()],
            }],
            datasets: vec![],
            include_credentials: true,
        };
        let resolved = resolve_selection(&descriptors, &selection).expect("解析成功");
        assert_eq!(resolved.ids_of("ssh.profiles"), ["p1".to_string()]);
        assert_eq!(resolved.ids_of("ssh.groups"), ["g1".to_string()]);
        assert_eq!(resolved.ids_of("vault.credentials"), ["c1".to_string()]);
        assert!(!resolved.includes("ssh.tunnels"), "未引用隧道不进包");
        assert!(!resolved.includes("core.favorites"), "未勾选收藏不进包");
    }

    /// 两个档案引用同一凭证：去重成一条；未绑定凭证的档案不伪造引用
    #[test]
    fn closure_dedups_credentials_and_keeps_unbound_profile() {
        let descriptors = catalog_fixture();
        let selection = ExportSelection {
            entries: vec![super::super::types::SelectionEntry {
                dataset: "ssh.profiles".into(),
                ids: vec!["p1".into(), "p2".into(), "p3".into()],
            }],
            datasets: vec![],
            include_credentials: true,
        };
        let resolved = resolve_selection(&descriptors, &selection).expect("解析成功");
        assert_eq!(resolved.ids_of("vault.credentials"), ["c1".to_string()]);
        assert_eq!(
            resolved.ids_of("ssh.profiles").len(),
            3,
            "未绑定凭证的档案仍然进包，只是不引出凭证"
        );
        assert_eq!(resolved.ids_of("ssh.tunnels"), ["t1".to_string()]);
    }

    /// 不可单独勾选的数据集被显式勾选：报错而不是忽略
    #[test]
    fn non_selectable_dataset_rejected() {
        let descriptors = catalog_fixture();
        let selection = ExportSelection {
            entries: vec![],
            datasets: vec!["vault.credentials".into()],
            include_credentials: true,
        };
        let error = resolve_selection(&descriptors, &selection).unwrap_err();
        assert!(error.contains("不能单独勾选"), "{error}");
    }

    /// 列表过期（目录里没有的 id）：报错，不静默少带
    #[test]
    fn stale_id_rejected() {
        let descriptors = catalog_fixture();
        let selection = ExportSelection {
            entries: vec![super::super::types::SelectionEntry {
                dataset: "ssh.profiles".into(),
                ids: vec!["p9".into()],
            }],
            datasets: vec![],
            include_credentials: true,
        };
        let error = resolve_selection(&descriptors, &selection).unwrap_err();
        assert!(error.contains("不存在记录 p9"), "{error}");
    }

    /// 空选择被拒（不生成「空包也算成功」）
    #[test]
    fn empty_selection_rejected() {
        let descriptors = catalog_fixture();
        let error = resolve_selection(&descriptors, &ExportSelection::empty()).unwrap_err();
        assert!(error.contains("没有选择"), "{error}");
    }

    /// 整块数据集（收藏）勾选后进包，值为空列表表示「全部」
    #[test]
    fn whole_dataset_selection_has_empty_ids() {
        let descriptors = catalog_fixture();
        let selection = ExportSelection {
            entries: vec![],
            datasets: vec!["core.favorites".into()],
            include_credentials: true,
        };
        let resolved = resolve_selection(&descriptors, &selection).expect("解析成功");
        assert!(resolved.includes("core.favorites"));
        assert!(resolved.ids_of("core.favorites").is_empty());
        assert_eq!(planned_counts(&resolved).get("core.favorites"), Some(&0));
    }

    /// 默认选择来自数据集声明：收藏默认勾、凭证默认可带出
    #[test]
    fn default_selection_follows_flags() {
        let mut descriptors = catalog_fixture();
        if let Some(favorites) = descriptors
            .iter_mut()
            .find(|item| item.name == "core.favorites")
        {
            favorites.default_selected = true;
        }
        let defaults = default_selection(&descriptors);
        assert_eq!(defaults.datasets, vec!["core.favorites".to_string()]);
        assert!(defaults.include_credentials);
        assert!(defaults.entries.is_empty());
    }

    /// 文本取值工具：非字符串或缺字段返回 None（不 panic）
    #[test]
    fn text_field_is_safe() {
        let record = serde_json::json!({"id": "p1", "port": 22});
        assert_eq!(text_field(&record, "id").as_deref(), Some("p1"));
        assert_eq!(text_field(&record, "port"), None);
        assert_eq!(text_field(&record, "missing"), None);
    }
}
