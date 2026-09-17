//! 数据包 owner 适配器（sync-202609-001 L2 · §13.6 冻结命名）
//!
//! 为什么需要：框架不能替各 owner 写业务 SQL，也不该维护一份「各插件有什么数据」的镜像
//! （镜像会过期，过期清单就是错误导出）。所以每个 owner 实现本 trait 并在装配阶段登记，
//! 框架只做闭包展开、限额校验与包读写。
//!
//! 契约（改动前先读）：
//! - **只读优先**：`describe_datasets` / `export_records` / `enumerate_references` 只查询，
//!   不得写入、不得触发连接或外部程序（§6.2）；
//! - **逻辑记录**：`export_records` 返回逻辑 JSON（字段名与前端契约一致），不是原始表行；
//!   物理路径、二进制位置、主机信任状态一律不进包（§13.1 device-local）；
//! - **秘密边界**：返回 `Err` 表示「读不出来」，不得用空数组冒充「没有数据」——静默空包会让用户
//!   以为已经导出成功；
//! - `validate_records` 在导出前与导入写入前都会被调用：两边用同一份判定，不写第二套规则。
//!
//! 与 §13.6 的差异（2026-09-16 记录）：冻结命名里的 `plan_import` / `apply_to_staging` 属**导入侧**，
//! 本批 C2 只做导出链路，这两个方法在 C3（隔离导入）随首个真实消费方一起加入，
//! 避免先摆两个无人调用的空实现（§13.6 第 4 条允许未发布命名随方案调整）。

use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use tauri::AppHandle;

use super::types::{
    DatasetDescriptor, DependencyEdge, IdMap, ImportContext, ImportMode, ImportPlanItem,
    ItemDecision,
};

/// 一个 owner 的本地导入导出能力
pub(crate) trait DatasetAdapter: Send + Sync {
    /// owner 标识（与 IPC owner 一致；一个 owner 只登记一个适配器）
    fn owner(&self) -> &'static str;

    /// 声明本 owner 可导出的数据集与当前空间的候选条目（含依赖边）
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String>;

    /// 按 id 导出记录体（逻辑 JSON）
    ///
    /// `ids` 为闭包展开后的记录 id；整块带出的数据集（收藏、最近使用）传空列表表示「全部」。
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String>;

    /// 校验记录体：导出前与导入写入前共用
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String>;

    /// 枚举记录体里的引用（档案 → 凭证/分组等），写进清单 `dependencies`
    fn enumerate_references(
        &self,
        dataset: &str,
        records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String>;

    /// 导入判定：逐条给出「新增 / 待补全 / 被排除」及理由（预览与提交前复核共用）
    ///
    /// 只做判定，不落盘：预览可以反复调用，且**不得**因为预览而在磁盘上留下痕迹。
    fn plan_import(
        &self,
        dataset: &str,
        records: &[Value],
        context: &ImportContext<'_>,
    ) -> Result<Vec<ImportPlanItem>, String>;

    /// 把记录写入新空间的暂存目录（隔离导入的写入侧）
    ///
    /// 契约：
    /// - 只能写 `target.root` 之内（暂存目录尚未成为任何空间，写别处等于绕过隔离）；
    /// - 不得读取或改写**当前**空间的数据（导入失败时当前空间必须零变化）；
    /// - 返回实际写入条数，提交方据此与清单声明条数核对，不一致即失败并回滚暂存目录。
    fn apply_to_staging(
        &self,
        dataset: &str,
        records: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String>;

    /// 合并/覆盖写入：把记录按决策写进**当前空间**（L3）
    ///
    /// 契约：
    /// - 该 owner 的全部数据集在**一次调用**内写完，实现方负责让自己的写入落在同一个
    ///   原子边界里（sqlite 型 owner 用单事务，文件型 owner 用「读改写 + 原子替换」）；
    /// - 逐条处置以 `target.decisions` 为准（Insert/Replace/KeepBoth 才写，其余跳过）；
    /// - 引用字段（profileId/groupId 等）按 `target.id_map` 改写；凭证引用一律置空；
    /// - 返回各数据集实际写入条数，提交方据此与计划核对，不一致即整体失败。
    fn apply_merge(
        &self,
        dataset_blocks: &[(String, Vec<Value>)],
        target: &MergeTarget<'_>,
    ) -> Result<std::collections::BTreeMap<String, usize>, String>;
}

/// 合并/覆盖写入的目标上下文（当前空间；写入在 owner 自己的事务/锁内完成）
pub(crate) struct MergeTarget<'a> {
    /// 当前空间根（文件型 owner 写 preferences.json 用）
    pub space_root: &'a std::path::Path,
    /// 当前空间数据访问（sqlite 型 owner 经它打开自己的插件库）
    pub app: &'a AppHandle,
    /// 导入模式（Merge / Overwrite）
    pub mode: ImportMode,
    /// 包内（数据集, 来源 id）→ 目标 id（引用字段按它改写）
    pub id_map: &'a IdMap,
    /// 每条（数据集, 来源 id）的最终处置（计划阶段已落位）
    pub decisions: &'a std::collections::BTreeMap<(String, String), ItemDecision>,
}

/// 隔离导入的写入目标：新空间的暂存目录 + 新空间身份
///
/// 为什么带 `space_id` 与密钥库：凭证要按**新空间**的主密钥重新加密（跨空间不可解），
/// 而不是沿用来源空间的密文。
pub(crate) struct StagingTarget {
    /// 暂存空间的**内容根**（`<设备根>/spaces/.patchybox-staging-<planId>`）
    ///
    /// 与正式空间 `StorageLocation::for_space(..).root` 同形（无代际层）：适配器用
    /// `root/data`、`root/vault` 这类相对路径，暂存与「写进既有空间」共用同一条代码路径。
    pub root: std::path::PathBuf,
    /// 新空间 id（已定，rename 后即成为正式空间目录名）
    pub space_id: String,
    /// 新空间的主密钥库（服务名已按 `space_id` 作用域）
    pub keyring: crate::framework::secure_store::ScopedKeyringStore,
}

/// 全局注册表（进程内；与 `framework::credential_refs` 同款约定）
fn registry() -> &'static Mutex<Vec<&'static dyn DatasetAdapter>> {
    static REGISTRY: OnceLock<Mutex<Vec<&'static dyn DatasetAdapter>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// 装配阶段登记（重复登记同一 owner 直接覆盖，保持幂等）
pub(crate) fn register(adapter: &'static dyn DatasetAdapter) {
    let Ok(mut list) = registry().lock() else {
        eprintln!(
            "[data_transfer] 适配器注册表锁定失败，跳过登记: {}",
            adapter.owner()
        );
        return;
    };
    list.retain(|existing| existing.owner() != adapter.owner());
    list.push(adapter);
}

/// 全部已登记适配器（按 owner 排序，保证导出的数据集顺序稳定）
pub(crate) fn all() -> Vec<&'static dyn DatasetAdapter> {
    let mut list: Vec<&'static dyn DatasetAdapter> = match registry().lock() {
        Ok(list) => list.clone(),
        Err(_) => {
            eprintln!("[data_transfer] 适配器注册表锁定失败，按无适配器处理");
            Vec::new()
        }
    };
    list.sort_by_key(|adapter| adapter.owner());
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用适配器：只有一个数据集，记录体按 id 生成
    struct FakeAdapter;

    impl DatasetAdapter for FakeAdapter {
        fn owner(&self) -> &'static str {
            "fake"
        }

        fn describe_datasets(&self, _app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
            Ok(Vec::new())
        }

        fn export_records(
            &self,
            _app: &AppHandle,
            _dataset: &str,
            ids: &[String],
        ) -> Result<Vec<Value>, String> {
            Ok(ids
                .iter()
                .map(|id| serde_json::json!({ "id": id }))
                .collect())
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
            _context: &ImportContext<'_>,
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
            _dataset: &str,
            records: &[Value],
            _target: &StagingTarget,
        ) -> Result<usize, String> {
            Ok(records.len())
        }

        fn apply_merge(
            &self,
            _dataset_blocks: &[(String, Vec<Value>)],
            _target: &MergeTarget<'_>,
        ) -> Result<std::collections::BTreeMap<String, usize>, String> {
            Ok(std::collections::BTreeMap::new())
        }
    }

    /// 登记后可见，重复登记同一 owner 不产生重复项（幂等）
    #[test]
    fn register_is_idempotent_per_owner() {
        static ADAPTER: FakeAdapter = FakeAdapter;
        let before = all().len();
        register(&ADAPTER);
        register(&ADAPTER);
        let after = all();
        assert_eq!(after.len(), before + 1);
        assert_eq!(
            after.iter().filter(|item| item.owner() == "fake").count(),
            1,
            "同一 owner 只能有一个适配器"
        );
    }
}
