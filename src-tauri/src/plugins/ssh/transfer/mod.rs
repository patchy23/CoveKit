//! SSH 数据传输适配器装配与数据集标识；实现分别在 catalog、records、plan、apply。
//!
//! 仅服务器档案可独立勾选，分组、隧道与书签跟随档案导出；known_hosts 不导出。
//! 记录使用模型的 serde 契约，创建与更新时间在导入时重写。
//! 目录依赖用于勾选闭包；记录引用用于导入排序与完整性校验。

mod apply;
mod catalog;
mod plan;
mod records;
#[cfg(test)]
mod tests;
use self::apply::apply_merge_blocks;
use self::apply::apply_to_staging;
use self::catalog::describe;
use self::catalog::export;
use self::catalog::with_read_conn;
use self::plan::plan_import;
use self::records::references;
use self::records::validate;
use crate::framework::data_transfer::adapter;
use crate::framework::data_transfer::adapter::DatasetAdapter;
use crate::framework::data_transfer::adapter::MergeTarget;
use crate::framework::data_transfer::adapter::StagingTarget;
use crate::framework::data_transfer::types::DatasetDescriptor;
use crate::framework::data_transfer::types::DependencyEdge;
use crate::framework::data_transfer::types::ImportContext;
use crate::framework::data_transfer::types::ImportPlanItem;
use crate::plugins::ssh::store;
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::AppHandle;

/// 数据集名：服务器档案（唯一可勾选）
pub(crate) const DATASET_PROFILES: &str = "ssh.profiles";

/// 数据集名：服务器分组（跟随带出）
pub(crate) const DATASET_GROUPS: &str = "ssh.groups";

/// 数据集名：隧道配置（跟随带出）
pub(crate) const DATASET_TUNNELS: &str = "ssh.tunnels";

/// 数据集名：目录书签（跟随带出）
pub(crate) const DATASET_BOOKMARKS: &str = "ssh.bookmarks";

/// 数据集 schema 版本（逻辑结构变更时 +1）
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// 依赖边类型：档案引用的凭证（落到 `vault.credentials`）
pub(super) const EDGE_CREDENTIAL: &str = "credential";

/// 依赖边类型：档案所属分组
pub(super) const EDGE_GROUP: &str = "group";

/// 依赖边类型：档案下的隧道
pub(super) const EDGE_TUNNEL: &str = "tunnel";

/// 依赖边类型：档案下的书签
pub(super) const EDGE_BOOKMARK: &str = "bookmark";

/// 依赖边类型：子记录所属档案（导入侧据此排序）
pub(super) const EDGE_PROFILE: &str = "profile";

/// 凭证数据集名（跨 owner 引用：SSH 只存引用，秘密本体在公共 Vault）
pub(super) const CREDENTIAL_DATASET: &str = "vault.credentials";

/// 不可单独勾选时的说明
pub(super) const NOTE_GROUP_FOLLOWS: &str = "由所选档案的分组自动带出，不可单独勾选";

/// 隧道不可单独勾选时的说明
pub(super) const NOTE_TUNNEL_FOLLOWS: &str = "随所选服务器档案自动带出，不可单独勾选";

/// 书签不可单独勾选时的说明
pub(super) const NOTE_BOOKMARK_FOLLOWS: &str = "随所选服务器档案自动带出，不可单独勾选";

/// 未绑定凭证的档案提示（导入后需在本机补全）
pub(super) const NOTE_NO_CREDENTIAL: &str = "未绑定凭证，导入后需补全凭证";

/// SSH 数据集适配器（进程级单例）
struct SshAdapter;

impl DatasetAdapter for SshAdapter {
    /// 归属 owner
    fn owner(&self) -> &'static str {
        "ssh"
    }

    /// 声明四个数据集与勾选闭包的跟随关系
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        with_read_conn(app, |conn| describe(conn))
    }

    /// 按 id 取指定数据集的记录（顺序与 `ids` 一致，缺 id 直接报错）
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        with_read_conn(app, |conn| export(conn, dataset, ids))
    }

    /// 记录体检：结构必须能反序列化回逻辑结构，关键字段不许为空
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        validate(dataset, records)
    }

    /// 记录级引用（清单 `dependencies` 用）
    fn enumerate_references(
        &self,
        dataset: &str,
        records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        references(dataset, records)
    }

    /// 导入判定：档案引用的凭证 / 分组没随包带来 → 标「待补全」，不当成完整导入
    fn plan_import(
        &self,
        dataset: &str,
        records: &[Value],
        context: &ImportContext<'_>,
    ) -> Result<Vec<ImportPlanItem>, String> {
        plan_import(dataset, records, context)
    }

    /// 写入新空间暂存的 SSH 库：建库 → 按数据集写入 → 读回自查
    fn apply_to_staging(
        &self,
        dataset: &str,
        records: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        apply_to_staging(dataset, records, target)
    }

    /// 合并/覆盖写入当前空间：四个数据集在一个事务内按决策落位（L3）
    fn apply_merge(
        &self,
        dataset_blocks: &[(String, Vec<Value>)],
        target: &MergeTarget<'_>,
    ) -> Result<BTreeMap<String, usize>, String> {
        let db = crate::framework::store::PluginDb::open(target.app, "ssh", store::MIGRATIONS)?;
        db.with_transaction(|conn| {
            apply_merge_blocks(
                conn,
                dataset_blocks,
                target.mode,
                target.id_map,
                target.decisions,
            )
        })
    }
}

/// 登记 SSH 适配器（插件装配阶段调用一次）
pub(crate) fn register() {
    static ADAPTER: SshAdapter = SshAdapter;
    adapter::register(&ADAPTER);
}

/// 关联命令登记用的数据集名集合（导入侧按顺序落库：先档案，再子记录）
pub(crate) fn dataset_order() -> [&'static str; 4] {
    [
        DATASET_GROUPS,
        DATASET_PROFILES,
        DATASET_TUNNELS,
        DATASET_BOOKMARKS,
    ]
}

/// 供导入侧判断某数据集是否属于本插件
pub(crate) fn owns(dataset: &str) -> bool {
    dataset_order().contains(&dataset)
}
