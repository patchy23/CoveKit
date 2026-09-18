//! 数据传输命令的 IPC 视图类型；不包含秘密记录体。
use crate::framework::data_transfer::types::ImportPlanItem;
use crate::framework::data_transfer::types::ImportReport;
use crate::framework::data_transfer::types::ImportSelection;
use crate::framework::data_transfer::types::TransportPolicy;

use crate::framework::data_transfer::types::ConflictDecision;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// 空间摘要（`data_spaces_list` 条目；只含非秘密信息，绝不含路径与凭证）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    /// 空间 id
    pub space_id: String,
    /// 展示名
    pub name: String,
    /// 创建（登记）时间
    pub created_at: String,
    /// 是否是当前活动空间
    pub active: bool,
    /// 是否由导入创建
    pub imported: bool,
    /// 来源空间名（不可信，仅展示）
    pub source_space_name: Option<String>,
    /// 导入时间
    pub imported_at: Option<String>,
    /// 各类别导入条数
    pub counts: BTreeMap<String, usize>,
}

/// 切换活动空间的结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSwitchResult {
    /// 切换到的空间 id
    pub space_id: String,
    /// 切换到的空间名
    pub name: String,
    /// 是否必须重启才生效（本批一律为真：不热切换）
    pub restart_required: bool,
    /// 写入后的设置版本号（界面回传用于下次保存）
    pub revision: u64,
}

/// 导出报告
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// 生成的文件路径
    pub path: String,
    /// 文件大小（字节）
    pub bytes: u64,
    /// 包标识（再次导出同内容时便于区分）
    pub package_id: String,
    /// 来源空间名（写进包内，供导入侧展示）
    pub source_space_name: String,
    /// 数据集名 → 进包条数
    pub counts: BTreeMap<String, usize>,
    /// 未进包的数据集（含只声明未携带的凭证块）
    pub excluded: Vec<String>,
    /// 是否携带了秘密（界面据此提示「包内含有凭证」）
    pub secret_included: bool,
}

/// 传输启动结果（导出与导入共用）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransferStart {
    /// 任务 id（界面在任务面板里定位这次传输）
    pub task_id: String,
    /// 导出报告（导入的 `data_import_commit` 用 `ImportCommitResult`）
    pub report: ExportReport,
}

/// 包内一个数据集的展示视图（不含记录体）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageDatasetView {
    /// 数据集名
    pub name: String,
    /// 本应用给它的展示名（来自适配器目录；不认识的数据集为空）
    pub label: String,
    /// 传输策略
    pub policy: TransportPolicy,
    /// 包内声明的 schema 版本
    pub schema_version: u32,
    /// 包内声明的条数
    pub record_count: usize,
    /// 是否携带了记录体（未携带 = 只声明）
    pub carried: bool,
    /// 本应用能否导入（未知 owner 或版本过高为否）
    pub supported: bool,
    /// 不支持的原因
    pub reason: Option<String>,
}

/// 包摘要（inspect 结果里的展示部分）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummaryView {
    /// 包标识
    pub package_id: String,
    /// 来源空间 id（不可信）
    pub source_space_id: String,
    /// 来源空间名（不可信）
    pub source_space_name: String,
    /// 导出时间（不可信）
    pub created_at: String,
    /// 导出时的应用版本（不可信）
    pub app_version: String,
    /// 导出时的平台（不可信）
    pub platform: String,
    /// 数据集视图
    pub datasets: Vec<PackageDatasetView>,
    /// 导出侧显式排除的项
    pub excluded: Vec<String>,
}

/// 二次导入提示（同一 packageId 已导入过）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateHint {
    /// 已导入到的空间 id
    pub space_id: String,
    /// 已导入到的空间名
    pub space_name: String,
    /// 导入时间
    pub imported_at: String,
}

/// inspect 结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportInspectResult {
    /// 预览上下文 id（规划与提交都要带上）
    pub inspect_id: String,
    /// 包摘要
    pub summary: PackageSummaryView,
    /// 默认选择（界面据此预勾选）
    pub defaults: ImportSelection,
    /// 逐条判定预览（新增 / 待补全 / 被排除）
    pub preview: Vec<ImportPlanItem>,
    /// 待补全说明
    pub pending: Vec<String>,
    /// 本次会排除的数据集
    pub excluded: Vec<String>,
    /// 二次导入提示
    pub duplicate: Option<DuplicateHint>,
}

/// 单条冲突的用户决策（合并导入）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictChoice {
    /// 数据集名
    pub dataset: String,
    /// 包内（来源）记录 id
    pub source_id: String,
    /// 处置（保留本地 / 采用导入 / 保留两份）
    pub decision: ConflictDecision,
}

/// plan 结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanResult {
    /// 计划 id（提交时回传）
    pub plan_id: String,
    /// 导入模式（newSpace / merge / overwrite；合并与覆盖导入进当前空间）
    pub mode: String,
    /// 目标空间存储修订号（合并/覆盖：提交时回传复核，D7）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
    /// 新空间 id（已定，提交后即成为空间目录名）
    pub space_id: String,
    /// 新空间展示名
    pub space_name: String,
    /// 逐条判定预览
    pub preview: Vec<ImportPlanItem>,
    /// 待补全说明
    pub pending: Vec<String>,
    /// 会被排除的数据集
    pub excluded: Vec<String>,
    /// 无密码时也能展示的条数对账（数据集名 → 条数）
    pub counts: BTreeMap<String, usize>,
}

/// 提交结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitResult {
    /// 任务 id
    pub task_id: String,
    /// 导入报告（新空间、条数、待补全、排除项）
    pub report: ImportReport,
}

/// 取消结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    /// 是否确有在跑的传输被请求取消
    pub cancelled: bool,
}

/// 快照条目（设置页「还原到导入前」列表用；只含非秘密信息）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    /// 快照目录名（还原时回传）
    pub dir: String,
    /// 快照所属空间 uid
    pub space_id: String,
    /// 拍摄时间（RFC3339）
    pub created_at: String,
    /// 覆盖的存储文件（空间根相对路径）
    pub files: Vec<String>,
}
