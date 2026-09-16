//! 隔离导入引擎（sync-202609-001 L2 · 方案 §6）
//!
//! 为什么不让导入直接写「当前空间」：导入到一半崩溃、或包里有读不懂的数据集，
//! 都会把用户现有的数据弄成半残状态。所以一律走**目标空间此前不存在**这条路径：
//!
//! ```text
//! ① 断言目标空间目录不存在          ② 写暂存目录 .patchybox-staging-<planId>
//! ③ 适配器逐数据集写入 + 自查读回    ④ 一次 rename 到 spaces/<spaceId>
//! ⑤ 写空间索引（失败 → 删掉刚建的空间目录，报告失败）
//! ```
//!
//! 全流程不读也不改当前空间：失败时当前空间必须零变化（有测试守着这条）。
//! 中途崩溃留在空间目录集里的暂存目录，由启动维护按前缀清理（`space::index::cleanup_staging`）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;
use tauri::AppHandle;

use crate::framework::secure_store::keyring_store_for;
use crate::framework::space::index as space_index;

use super::adapter::{self, StagingTarget};
use super::types::{
    CarriedBlock, DatasetDescriptor, ImportContext, ImportOutcome, ImportPlanItem, ImportReport,
    ImportSelection, PackageManifest, TransportPolicy,
};

/// 新空间的第一代（导入只产生一代：包里的数据就是这一代的内容）
const IMPORT_GENERATION: u64 = 1;

/// 计划 id（导入会话标识，同时用作暂存目录名后缀）
pub(crate) fn new_plan_id() -> String {
    format!("imp-{}", uuid::Uuid::new_v4())
}

/// 计划里要写入的一个数据集块
#[derive(Debug, Clone)]
pub(crate) struct PlannedBlock {
    /// 数据集名（`ssh.profiles` 等）
    pub dataset: String,
    /// 拥有该数据集的适配器 owner（写入时按 owner 找适配器）
    pub owner: String,
    /// 记录体（已确认非空）
    pub records: Vec<Value>,
}

/// 导入计划：包清单 + 用户选择 → 可执行的写入清单
///
/// 计划在命令层生成后**不再重复解析包**：`planId` 指向的这份结构就是唯一依据。
#[derive(Debug, Clone)]
pub(crate) struct ImportPlan {
    /// 计划 id（暂存目录名后缀）
    pub plan_id: String,
    /// 新空间 id（UUIDv4，已定）
    pub space_id: String,
    /// 新空间展示名
    pub space_name: String,
    /// 包 id（留档与去重提示用）
    pub package_id: String,
    /// 来源空间 id
    pub source_space_id: String,
    /// 来源空间展示名
    pub source_space_name: String,
    /// 包内声明的数据集与条数（留档：导入后仍能看到「包里本来有什么」）
    pub declared_counts: BTreeMap<String, usize>,
    /// 实际要写入的块
    pub blocks: Vec<PlannedBlock>,
    /// 每条记录的判定结果（界面按类分档展示）
    pub items: Vec<ImportPlanItem>,
    /// 未导入的数据集（含原因）
    pub excluded: Vec<String>,
}

impl ImportPlan {
    /// 待补全的说明（凭证未随包带出时界面要提示用户重填）
    pub(crate) fn pending_notes(&self) -> Vec<String> {
        let mut notes: Vec<String> = self
            .items
            .iter()
            .filter(|item| item.outcome == ImportOutcome::PendingReference)
            .map(|item| item.note.clone().unwrap_or_else(|| item.label.clone()))
            .collect();
        notes.sort();
        notes.dedup();
        notes
    }
}

/// 由包清单与用户选择构造导入计划（不碰磁盘，便于用例覆盖）
pub(crate) fn build_plan(
    manifest: &PackageManifest,
    descriptors: &[DatasetDescriptor],
    selection: &ImportSelection,
    space_id: &str,
    space_name: &str,
    plan_id: &str,
) -> Result<ImportPlan, String> {
    let context = import_context(manifest);
    let mut blocks = Vec::new();
    let mut items = Vec::new();
    let mut excluded = Vec::new();
    let mut declared_counts = BTreeMap::new();

    for block in &manifest.datasets {
        declared_counts.insert(block.name.clone(), block.record_count);
        let descriptor = descriptors.iter().find(|item| item.name == block.name);
        let Some(descriptor) = descriptor else {
            return Err(format!(
                "数据包包含本应用不认识的数据集 {}（请升级应用后重试）",
                block.name
            ));
        };

        // 用户没勾选的数据集：明确记录为「本次未导入」，不静默丢
        if !selection.includes(&block.name) {
            excluded.push(block.name.clone());
            items.push(ImportPlanItem::excluded(
                &block.name,
                &descriptor.label,
                "本次未选择导入",
            ));
            continue;
        }

        if descriptor.schema_version < block.schema_version {
            return Err(format!(
                "数据集 {} 的 schema 版本 {} 高于当前应用支持的 {}（请升级应用后重试）",
                block.name, block.schema_version, descriptor.schema_version
            ));
        }

        if block.policy == TransportPolicy::DeviceLocal {
            return Err(format!("数据集 {} 声明为本机事实，不允许导入", block.name));
        }

        let adapter = find_adapter(descriptor)?;
        let records = match &block.records {
            Some(Value::Array(records)) => records.clone(),
            Some(_) => {
                return Err(format!("数据集 {} 的记录体不是数组", block.name));
            }
            None => {
                // 只声明未携带：secret 未确认带出时就是这样，导入后由用户重填
                excluded.push(block.name.clone());
                items.push(ImportPlanItem::excluded(
                    &block.name,
                    &descriptor.label,
                    &format!("包内只声明了 {} 条，未携带记录", block.record_count),
                ));
                continue;
            }
        };

        items.extend(adapter.plan_import(&block.name, &records, &context)?);
        if !records.is_empty() {
            blocks.push(PlannedBlock {
                dataset: block.name.clone(),
                owner: descriptor.owner.clone(),
                records,
            });
        }
    }

    if blocks.is_empty() {
        return Err("本次选择没有可导入的数据".into());
    }

    Ok(ImportPlan {
        plan_id: plan_id.to_string(),
        space_id: space_id.to_string(),
        space_name: space_name.to_string(),
        package_id: manifest.package_id.clone(),
        source_space_id: manifest.source_space_id.clone(),
        source_space_name: manifest.source_space_name.clone(),
        declared_counts,
        blocks,
        items,
        excluded,
    })
}

/// 物化导入：写暂存目录 → 适配器自查 → 一次 rename
///
/// 返回每个数据集实际写入的条数。任何一步失败都会删除暂存目录并返回错误，
/// 当前空间在此过程中**没有任何读写**。
pub(crate) fn materialize(
    device_root: &Path,
    plan: &ImportPlan,
) -> Result<BTreeMap<String, usize>, String> {
    let staging = space_index::staging_space_root(device_root, &plan.plan_id);
    // 暂存空间的**内容根**（代际目录）：与正式空间 `StorageLocation::partitioned` 同形，
    // 适配器因此只需一套相对路径（`root/data`、`root/vault`）
    let content_root =
        space_index::staging_content_root(device_root, &plan.plan_id, IMPORT_GENERATION);

    let space_dir = space_index::space_root(device_root, &plan.space_id);
    if space_dir.exists() {
        return Err(format!(
            "目标空间目录已存在，拒绝覆盖：{}",
            space_dir.display()
        ));
    }
    if staging.exists() {
        return Err(format!(
            "暂存目录已存在（计划 id 冲突）：{}",
            staging.display()
        ));
    }

    std::fs::create_dir_all(&content_root).map_err(|e| format!("创建暂存目录失败: {e}"))?;
    std::fs::create_dir_all(content_root.join("data"))
        .map_err(|e| format!("创建暂存数据目录失败: {e}"))?;
    std::fs::create_dir_all(content_root.join("vault"))
        .map_err(|e| format!("创建暂存凭证目录失败: {e}"))?;

    let target = StagingTarget {
        root: content_root.clone(),
        space_id: plan.space_id.clone(),
        keyring: keyring_store_for(&plan.space_id),
    };

    let mut counts = BTreeMap::new();
    for block in &plan.blocks {
        let adapter = match adapter::all()
            .into_iter()
            .find(|item| item.owner() == block.owner)
        {
            Some(adapter) => adapter,
            None => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(format!(
                    "数据集 {} 的适配器（{}）未登记，无法写入",
                    block.dataset, block.owner
                ));
            }
        };
        let written = match adapter.apply_to_staging(&block.dataset, &block.records, &target) {
            Ok(written) => written,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(format!("写入数据集 {} 失败: {error}", block.dataset));
            }
        };
        if written != block.records.len() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!(
                "数据集 {} 计划写入 {} 条，实际写入 {written} 条",
                block.dataset,
                block.records.len()
            ));
        }
        counts.insert(block.dataset.clone(), written);
    }

    // 一次 rename：暂存目录成为正式空间目录（同一父目录内，失败即报错）
    if let Err(error) = std::fs::rename(&staging, &space_dir) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("暂存目录改名失败: {error}"));
    }
    Ok(counts)
}

/// 删除刚建立的空间目录（索引写入失败时的收尾；没有索引引用，可直接删）
pub(crate) fn discard_space(device_root: &Path, space_id: &str) -> Result<(), String> {
    let dir = space_index::space_root(device_root, space_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除空间目录失败: {e}"))?;
    }
    Ok(())
}

/// 提交导入：物化 → 写索引 → 产出报告
///
/// 索引写入失败时删掉刚建立的空间目录并报错：宁可当这次导入没发生，
/// 也不要留下一个「有数据但空间列表里看不到」的孤儿目录。
pub(crate) fn commit(
    app: &AppHandle,
    device_root: &Path,
    plan: &ImportPlan,
) -> Result<ImportReport, String> {
    let counts = materialize(device_root, plan)?;
    let imported_at = space_index::now_iso();
    let record = space_index::SpaceRecord::imported(
        &plan.space_name,
        &imported_at,
        &plan.source_space_id,
        &plan.source_space_name,
        &plan.package_id,
        &counts,
    );
    if let Err(error) = space_index::write_record(app, &plan.space_id, &record) {
        let _ = discard_space(device_root, &plan.space_id);
        return Err(format!("写入空间索引失败，已回滚本次导入: {error}"));
    }
    Ok(ImportReport {
        space_id: plan.space_id.clone(),
        space_name: plan.space_name.clone(),
        source_space_id: plan.source_space_id.clone(),
        source_space_name: plan.source_space_name.clone(),
        package_id: plan.package_id.clone(),
        imported_at,
        counts,
        declared_counts: plan.declared_counts.clone(),
        pending: plan.pending_notes(),
        excluded: plan.excluded.clone(),
    })
}

/// 导入上下文：适配器据此判断「引用的东西到底有没有随包来」
fn import_context(manifest: &PackageManifest) -> ImportContext {
    let mut carried = BTreeMap::new();
    for block in &manifest.datasets {
        let entry = match &block.records {
            Some(Value::Array(records)) => {
                let ids: BTreeSet<String> = records
                    .iter()
                    .filter_map(|record| record.get("id").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect();
                CarriedBlock {
                    record_count: records.len(),
                    ids: if ids.len() == records.len() {
                        Some(ids)
                    } else {
                        // 记录没有 id（整块数据集，如收藏）：只能按条数判断
                        None
                    },
                }
            }
            _ => CarriedBlock {
                record_count: block.record_count,
                ids: None,
            },
        };
        carried.insert(block.name.clone(), entry);
    }
    ImportContext {
        source_space_id: manifest.source_space_id.clone(),
        carried,
    }
}

/// 按描述符找适配器
fn find_adapter(
    descriptor: &DatasetDescriptor,
) -> Result<&'static dyn adapter::DatasetAdapter, String> {
    adapter::all()
        .into_iter()
        .find(|item| item.owner() == descriptor.owner)
        .ok_or_else(|| {
            format!(
                "数据集 {} 的适配器（{}）未登记",
                descriptor.name, descriptor.owner
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::data_transfer::types::{DatasetBlock, DatasetDescriptor, DependencyEdge};

    /// 测试用适配器：把记录写成 JSON 文件，便于检查「真写没写、写了几条」
    ///
    /// 约定：记录体里出现 `{"fail": true}` 时故意多报一条，用来验证「条数不符即回滚」。
    struct FakeAdapter;

    impl adapter::DatasetAdapter for FakeAdapter {
        fn owner(&self) -> &'static str {
            "t"
        }

        fn describe_datasets(&self, _app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
            Ok(Vec::new())
        }

        fn export_records(
            &self,
            _app: &AppHandle,
            _dataset: &str,
            _ids: &[String],
        ) -> Result<Vec<Value>, String> {
            Ok(Vec::new())
        }

        fn validate_records(&self, _dataset: &str, _records: &[Value]) -> Result<(), String> {
            Ok(())
        }

        fn enumerate_references(
            &self,
            _dataset: &str,
            _records: &[Value],
        ) -> Result<Vec<DependencyEdge>, String> {
            Ok(Vec::new())
        }

        fn plan_import(
            &self,
            dataset: &str,
            records: &[Value],
            _context: &ImportContext,
        ) -> Result<Vec<ImportPlanItem>, String> {
            Ok(records
                .iter()
                .map(|record| {
                    let id = record.get("id").and_then(Value::as_str).unwrap_or("?");
                    ImportPlanItem::added(dataset, id, id)
                })
                .collect())
        }

        fn apply_to_staging(
            &self,
            dataset: &str,
            records: &[Value],
            target: &StagingTarget,
        ) -> Result<usize, String> {
            let path = target.root.join("data").join(format!("{dataset}.json"));
            std::fs::write(&path, serde_json::to_vec(records).expect("序列化测试记录"))
                .map_err(|e| format!("写测试数据失败: {e}"))?;
            let forced = records
                .iter()
                .any(|record| record.get("fail").and_then(Value::as_bool).unwrap_or(false));
            Ok(if forced {
                records.len() + 1
            } else {
                records.len()
            })
        }
    }

    /// 登记测试适配器（幂等）
    fn fake() -> &'static FakeAdapter {
        static ADAPTER: FakeAdapter = FakeAdapter;
        adapter::register(&ADAPTER);
        &ADAPTER
    }

    /// 构造描述符（只填用例用得到的字段）
    fn descriptor(name: &str, owner: &str, policy: TransportPolicy) -> DatasetDescriptor {
        DatasetDescriptor {
            name: name.to_string(),
            label: name.to_string(),
            owner: owner.to_string(),
            policy,
            schema_version: 1,
            selectable: true,
            contains_secret: policy == TransportPolicy::Secret,
            default_selected: false,
            pulls: Vec::new(),
            note: None,
            record_count: 0,
            entries: Vec::new(),
        }
    }

    /// 构造块（记录体与条数、摘要一致）
    fn block(name: &str, policy: TransportPolicy, records: Vec<Value>) -> DatasetBlock {
        let body = Value::Array(records);
        DatasetBlock {
            name: name.to_string(),
            schema_version: 1,
            policy,
            record_count: body.as_array().map(Vec::len).unwrap_or(0),
            sha256: super::super::types::dataset_digest(&body).expect("摘要"),
            records: Some(body),
        }
    }

    /// 生成一个只含指定块的清单
    fn manifest(blocks: Vec<DatasetBlock>) -> PackageManifest {
        let mut manifest = PackageManifest::new("default", "默认空间");
        manifest.datasets = blocks;
        manifest
    }

    /// 未勾选的数据集进入排除项；只声明未携带的 secret 块也进排除项并说明原因
    #[test]
    fn build_plan_records_exclusions_explicitly() {
        fake();
        let manifest = manifest(vec![
            block(
                "t.records",
                TransportPolicy::Portable,
                vec![serde_json::json!({"id": "r1"})],
            ),
            DatasetBlock {
                name: "t.secret".into(),
                schema_version: 1,
                policy: TransportPolicy::Secret,
                record_count: 2,
                sha256: String::new(),
                records: None,
            },
        ]);
        let descriptors = vec![
            descriptor("t.records", "t", TransportPolicy::Portable),
            descriptor("t.secret", "t", TransportPolicy::Secret),
        ];
        let selection = ImportSelection {
            datasets: vec!["t.records".into(), "t.secret".into()],
        };
        let plan = build_plan(
            &manifest,
            &descriptors,
            &selection,
            "11111111-1111-4111-8111-111111111111",
            "导入空间",
            "imp-test",
        )
        .expect("计划生成成功");

        assert_eq!(plan.blocks.len(), 1);
        assert_eq!(plan.blocks[0].dataset, "t.records");
        assert_eq!(plan.excluded, vec!["t.secret".to_string()]);
        assert_eq!(plan.declared_counts["t.secret"], 2);
        assert!(plan
            .items
            .iter()
            .any(|item| item.outcome == ImportOutcome::Excluded));
        assert!(plan.pending_notes().is_empty());
    }

    /// 未选择任何数据集 → 没有可导入的数据（不生成空计划）
    #[test]
    fn build_plan_rejects_empty_selection() {
        fake();
        let manifest = manifest(vec![block(
            "t.records",
            TransportPolicy::Portable,
            vec![serde_json::json!({"id": "r1"})],
        )]);
        let descriptors = vec![descriptor("t.records", "t", TransportPolicy::Portable)];
        let selection = ImportSelection {
            datasets: Vec::new(),
        };
        let error = build_plan(
            &manifest,
            &descriptors,
            &selection,
            "11111111-1111-4111-8111-111111111111",
            "导入空间",
            "imp-test",
        )
        .expect_err("空选择必须报错");
        assert!(error.contains("没有可导入的数据"), "错误文案: {error}");
    }

    /// 未知数据集 / 更高 schema / device-local 一律拒绝（不允许猜着导入）
    #[test]
    fn build_plan_rejects_unknown_or_unsupported_blocks() {
        fake();
        let selection = ImportSelection {
            datasets: vec!["t.records".into(), "t.blocked".into()],
        };
        let descriptors = vec![
            descriptor("t.records", "t", TransportPolicy::Portable),
            descriptor("t.blocked", "t", TransportPolicy::Portable),
        ];

        // 未知数据集
        let unknown = manifest(vec![block(
            "t.mystery",
            TransportPolicy::Portable,
            vec![serde_json::json!({"id": "r1"})],
        )]);
        let mut selection_unknown = ImportSelection {
            datasets: vec!["t.mystery".into()],
        };
        selection_unknown.datasets.push("t.records".into());
        let mut unknown_manifest = unknown;
        unknown_manifest.datasets[0].name = "t.mystery".into();
        let error = build_plan(
            &unknown_manifest,
            &descriptors,
            &selection_unknown,
            "11111111-1111-4111-8111-111111111111",
            "导入空间",
            "imp-test",
        )
        .expect_err("未知数据集必须报错");
        assert!(error.contains("不认识的数据集"), "错误文案: {error}");

        // 更高 schema 版本
        let mut newer = manifest(vec![block(
            "t.records",
            TransportPolicy::Portable,
            vec![serde_json::json!({"id": "r1"})],
        )]);
        newer.datasets[0].schema_version = 9;
        let error = build_plan(
            &newer,
            &descriptors,
            &selection,
            "11111111-1111-4111-8111-111111111111",
            "导入空间",
            "imp-test",
        )
        .expect_err("更高 schema 必须报错");
        assert!(error.contains("schema 版本"), "错误文案: {error}");

        // device-local 块
        let local = manifest(vec![block(
            "t.blocked",
            TransportPolicy::DeviceLocal,
            vec![serde_json::json!({"id": "r1"})],
        )]);
        let mut local_selection = ImportSelection {
            datasets: vec!["t.blocked".into()],
        };
        local_selection.datasets.push("t.records".into());
        let error = build_plan(
            &local,
            &descriptors,
            &local_selection,
            "11111111-1111-4111-8111-111111111111",
            "导入空间",
            "imp-test",
        )
        .expect_err("device-local 必须报错");
        assert!(error.contains("本机事实"), "错误文案: {error}");
    }

    /// 测试用设备根（每次用不同后缀，避免并行用例互相踩）
    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("pb-import-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// 测试用空间 id（合法 UUIDv4）
    const SPACE_ID: &str = "11111111-1111-4111-8111-111111111111";

    /// 生成计划：两个数据集都勾选
    fn plan_for(blocks: Vec<DatasetBlock>, plan_id: &str) -> ImportPlan {
        let names = blocks
            .iter()
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        let manifest = manifest(blocks);
        let descriptors = names
            .iter()
            .map(|name| descriptor(name, "t", TransportPolicy::Portable))
            .collect::<Vec<_>>();
        let selection = ImportSelection { datasets: names };
        build_plan(
            &manifest,
            &descriptors,
            &selection,
            SPACE_ID,
            "导入空间",
            plan_id,
        )
        .expect("计划生成成功")
    }

    /// 物化：先写暂存目录，再一次 rename 成空间目录，暂存目录不残留
    #[test]
    fn materialize_promotes_staging_into_space() {
        fake();
        let device = temp_root("promote");
        let plan = plan_for(
            vec![
                block(
                    "t.records",
                    TransportPolicy::Portable,
                    vec![
                        serde_json::json!({"id": "r1"}),
                        serde_json::json!({"id": "r2"}),
                    ],
                ),
                block(
                    "t.other",
                    TransportPolicy::Portable,
                    vec![serde_json::json!({"id": "o1"})],
                ),
            ],
            "imp-ok",
        );
        let counts = materialize(&device, &plan).expect("物化成功");
        assert_eq!(counts["t.records"], 2);
        assert_eq!(counts["t.other"], 1);

        let data = space_index::generation_root(&device, SPACE_ID, 1).join("data");
        assert!(
            data.join("t.records.json").exists(),
            "数据应落在正式空间目录内"
        );
        assert!(data.join("t.other.json").exists());
        assert!(
            !space_index::staging_space_root(&device, "imp-ok").exists(),
            "rename 后不应残留暂存目录"
        );
        let _ = std::fs::remove_dir_all(&device);
    }

    /// 适配器报的条数与计划不符 → 整体失败并回滚（空间目录与暂存目录都不留）
    #[test]
    fn materialize_rolls_back_on_count_mismatch() {
        fake();
        let device = temp_root("rollback");
        let plan = plan_for(
            vec![block(
                "t.records",
                TransportPolicy::Portable,
                vec![serde_json::json!({"id": "r1", "fail": true})],
            )],
            "imp-fail",
        );
        let error = materialize(&device, &plan).expect_err("条数不符必须失败");
        assert!(error.contains("实际写入"), "错误文案: {error}");
        assert!(!space_index::space_root(&device, SPACE_ID).exists());
        assert!(!space_index::staging_space_root(&device, "imp-fail").exists());
        let _ = std::fs::remove_dir_all(&device);
    }

    /// 目标空间目录已存在 → 拒绝覆盖（导入只走「目标空间此前不存在」这条路径）
    #[test]
    fn materialize_refuses_existing_space_dir() {
        fake();
        let device = temp_root("occupied");
        std::fs::create_dir_all(space_index::space_root(&device, SPACE_ID)).expect("预置空间目录");
        let plan = plan_for(
            vec![block(
                "t.records",
                TransportPolicy::Portable,
                vec![serde_json::json!({"id": "r1"})],
            )],
            "imp-busy",
        );
        let error = materialize(&device, &plan).expect_err("已存在的空间必须拒绝");
        assert!(error.contains("已存在"), "错误文案: {error}");
        let _ = std::fs::remove_dir_all(&device);
    }
}
