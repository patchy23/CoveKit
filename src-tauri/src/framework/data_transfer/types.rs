//! 数据包 DTO：逻辑清单与数据集块（两者都在密文里层；明文头不含这些信息）
//!
//! 三条约定（L2 方案 §4.2）：
//! - 传输标识沿用既有主键（档案 / 书签 / 隧道 / 收藏 id），不引入 `entityUid` 与 `revision`
//!   （2026-09-15 用户裁决：不做 UID 重映射）
//! - `secret` 策略的块**只在用户显式勾选时携带记录**：凭证随加密包搬移（§13.1「默认不带，勾选后仅经加密包」），
//!   未勾选时块只声明条数、不带记录；`device-local` 的块一律只作声明，不带记录（本机路径与二进制位置不搬移）
//! - 限额、记录数与摘要在这里统一校验：导出侧拦住超限输入，导入侧对不可信包先验后解（§4.3）
//!
//! 摘要约定：`sha256` 按本仓库的序列化结果计算（`serde_json::Value` 对象键有序），
//! 导出与导入两侧走同一序列化路径故可比；换序列化器必须同步改 `dataset_digest`，
//! 否则旧包会在「摘要不符」处被判为损坏。
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// 单包记录总数上限（§4.3）
pub(crate) const MAX_RECORDS_PER_PACKAGE: usize = 50_000;
/// 单条记录上限（字节，按序列化后大小计）
pub(crate) const MAX_RECORD_BYTES: usize = 2 * 1024 * 1024;
/// 单条记录 JSON 嵌套深度上限
pub(crate) const MAX_JSON_DEPTH: usize = 64;

/// 数据集传输策略：决定该块能不能进包、以什么形态进包（§13.1 冻结取值）
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TransportPolicy {
    /// 可搬移：id 稳定、不含设备本地事实，换机可直接用
    Portable,
    /// 设备本地：含本机路径 / 端口 / 二进制信息，不随包搬移（包内只作声明）
    DeviceLocal,
    /// 秘密：凭证等敏感值，不随包搬移，导入后由用户重填
    Secret,
    /// 敏感内容：可搬移，但导入预览必须显式提示
    SensitiveContent,
    /// 历史记录：可搬移，导入端可裁剪
    History,
}

/// 数据集块：一个 owner 的一类记录（如 `ssh.profiles`）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatasetBlock {
    /// 数据集名（`<owner>.<类别>`，跨包稳定，不随界面文案变）
    pub name: String,
    /// 该数据集自身的 schema 版本（0 非法；结构变更时递增）
    pub schema_version: u32,
    /// 传输策略
    pub policy: TransportPolicy,
    /// 记录条数（与 `records` 实际长度必须一致）
    pub record_count: usize,
    /// 记录体摘要（hex，见文件头「摘要约定」）
    pub sha256: String,
    /// 记录体（逻辑 JSON 数组）；`secret` 策略与「只声明不携带」的块为 `None`
    pub records: Option<Value>,
}

/// 依赖边：`fromId` 引用了 `toId`（如档案 → 凭证）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyEdge {
    /// 依赖类别（如 `credential`）
    pub kind: String,
    /// 引用方 id
    pub from_id: String,
    /// 被引用方 id
    pub to_id: String,
}

/// 数据包逻辑清单（加密后落在包内层，导入端据它生成预览）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageManifest {
    /// 包 id（UUIDv4，导出生成；用于「该包是否导入过」提示）
    pub package_id: String,
    /// 来源空间 id（不可信，仅展示与留档）
    pub source_space_id: String,
    /// 来源空间名（不可信，仅展示与留档）
    pub source_space_name: String,
    /// 导出时间（RFC3339；仅展示）
    pub created_at: String,
    /// 导出时的应用版本（仅展示）
    pub app_version: String,
    /// 导出时的平台（`std::env::consts::OS`；仅展示）
    pub platform: String,
    /// 数据集块
    pub datasets: Vec<DatasetBlock>,
    /// 依赖边（导入侧据此判「待补全」）
    pub dependencies: Vec<DependencyEdge>,
    /// 用户在预览里显式排除的类别 / 记录
    pub excluded: Vec<String>,
}

impl PackageManifest {
    /// 新清单骨架：生成 `packageId` 与 `createdAt`，填入当前版本与平台；数据集 / 依赖 / 排除项由调用方填。
    ///
    /// 来源空间信息由调用方传入：容器层不读存储上下文，便于用夹具空间构造用例。
    pub(crate) fn new(source_space_id: &str, source_space_name: &str) -> Self {
        Self {
            package_id: uuid::Uuid::new_v4().to_string(),
            source_space_id: source_space_id.to_owned(),
            source_space_name: source_space_name.to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: std::env::consts::OS.to_owned(),
            datasets: Vec::new(),
            dependencies: Vec::new(),
            excluded: Vec::new(),
        }
    }
}

/// 记录体摘要：本仓库序列化结果的 sha256（hex 小写）
pub(crate) fn dataset_digest(records: &Value) -> Result<String, String> {
    let body = serde_json::to_vec(records).map_err(|e| format!("记录体序列化失败: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&body);
    Ok(hex::encode(hasher.finalize()))
}

/// JSON 嵌套深度（标量 = 1）：用显式栈遍历，恶意深嵌套不会耗尽调用栈
pub(crate) fn json_depth(value: &Value) -> usize {
    let mut deepest = 0usize;
    let mut stack: Vec<(&Value, usize)> = vec![(value, 1)];
    while let Some((node, depth)) = stack.pop() {
        if depth > deepest {
            deepest = depth;
        }
        match node {
            Value::Array(items) => {
                for item in items {
                    stack.push((item, depth + 1));
                }
            }
            Value::Object(map) => {
                for item in map.values() {
                    stack.push((item, depth + 1));
                }
            }
            _ => {}
        }
    }
    deepest
}

/// 校验清单自身一致性：导出前与导入解密后都调用，避免「清单说一套、记录体是另一套」。
///
/// 校验项：块名唯一非空、`schemaVersion` 非 0、记录数与记录体一致、`device-local` 块不携带记录、
/// 可搬移块声明有记录时必须带记录体、记录体摘要相符、单条记录大小与嵌套深度不超限、记录总数不超上限。
pub(crate) fn validate_manifest(manifest: &PackageManifest) -> Result<(), String> {
    if manifest.datasets.is_empty() {
        return Err("数据包清单没有任何数据集".into());
    }
    if manifest.package_id.trim().is_empty() {
        return Err("数据包清单缺少 packageId".into());
    }
    let mut seen: Vec<&str> = Vec::with_capacity(manifest.datasets.len());
    let mut total_records = 0usize;
    for block in &manifest.datasets {
        if block.name.trim().is_empty() {
            return Err("数据包清单存在没有名称的数据集块".into());
        }
        if seen.contains(&block.name.as_str()) {
            return Err(format!("数据包清单存在重名的数据集块：{}", block.name));
        }
        seen.push(&block.name);
        if block.schema_version == 0 {
            return Err(format!("数据集 {} 的 schemaVersion 非法（0）", block.name));
        }
        total_records = total_records.saturating_add(block.record_count);
        match (&block.records, block.policy) {
            (Some(_), TransportPolicy::DeviceLocal) => {
                return Err(format!(
                    "数据集 {} 声明为 device-local 却携带了记录（本机事实不随包搬移）",
                    block.name
                ));
            }
            (None, TransportPolicy::DeviceLocal | TransportPolicy::Secret) => {
                // 只声明不携带：device-local 一律如此；secret 在用户未勾选带出凭证时如此
            }
            (None, _) if block.record_count > 0 => {
                return Err(format!(
                    "数据集 {} 声明有 {} 条记录但记录体缺失",
                    block.name, block.record_count
                ));
            }
            _ => {}
        }
        let Some(records) = &block.records else {
            continue;
        };
        let Some(items) = records.as_array() else {
            return Err(format!("数据集 {} 的记录体不是数组", block.name));
        };
        if items.len() != block.record_count {
            return Err(format!(
                "数据集 {} 的记录数与声明不一致（声明 {}，实际 {}）",
                block.name,
                block.record_count,
                items.len()
            ));
        }
        let digest = dataset_digest(records)?;
        if digest != block.sha256 {
            return Err(format!(
                "数据集 {} 的 sha256 与记录体不符（包可能被改动或生成端有缺陷）",
                block.name
            ));
        }
        for item in items {
            let size = serde_json::to_vec(item)
                .map_err(|e| format!("数据集 {} 记录序列化失败: {e}", block.name))?
                .len();
            if size > MAX_RECORD_BYTES {
                return Err(format!(
                    "数据集 {} 存在超过单条上限（{} 字节）的记录",
                    block.name, MAX_RECORD_BYTES
                ));
            }
            if json_depth(item) > MAX_JSON_DEPTH {
                return Err(format!(
                    "数据集 {} 存在嵌套深度超过 {} 的记录",
                    block.name, MAX_JSON_DEPTH
                ));
            }
        }
    }
    if total_records > MAX_RECORDS_PER_PACKAGE {
        return Err(format!(
            "记录总数 {total_records} 超过单包上限 {MAX_RECORDS_PER_PACKAGE}"
        ));
    }
    Ok(())
}

/* ── 选择集与可导出目录（C2 导出链路） ───────────────────────────────── */

/// 可勾选条目（目录列表项；`id` 是传输用主键，`label`/`detail` 只用于展示）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogEntry {
    /// 所属数据集名（如 `ssh.profiles`）
    pub dataset: String,
    /// 记录 id（勾选与依赖引用的目标）
    pub id: String,
    /// 展示名（服务器名 / 凭证名）
    pub label: String,
    /// 一行补充信息（`user@host:port`）
    pub detail: String,
    /// 该条目引用了谁（勾选后按 `pulls` 带出）
    pub dependencies: Vec<DependencyEdge>,
    /// 待补全提示（缺省 = 无提示）
    pub note: Option<String>,
}

/// 数据集对另一数据集的跟随关系：`kind` 的依赖边指向 `dataset`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatasetPull {
    /// 依赖边类型（如 `credential`）
    pub kind: String,
    /// 被带出的数据集名
    pub dataset: String,
}

/// 适配器声明的可导出数据集
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatasetDescriptor {
    /// 数据集名（`<owner>.<集合>`）
    pub name: String,
    /// 界面文案（与 `name` 分离：文案可变，数据集名是包内标识）
    pub label: String,
    /// 归属 owner
    pub owner: String,
    /// 传输策略
    pub policy: TransportPolicy,
    /// 逻辑 schema 版本
    pub schema_version: u32,
    /// 是否可单独勾选（凭证/跟随数据集为 false）
    pub selectable: bool,
    /// 是否含秘密（界面需要显式确认）
    pub contains_secret: bool,
    /// 默认是否勾选（整块数据集用；条目级默认由界面决定）
    pub default_selected: bool,
    /// 跟随关系：本数据集记录引用了哪些数据集
    pub pulls: Vec<DatasetPull>,
    /// 不可单独勾选时的说明（缺省 = 可单独勾选）
    pub note: Option<String>,
    /// 当前空间可带出的记录数（按条勾选的数据集必须等于 `entries` 长度；整块数据集为实际条数）
    pub record_count: usize,
    /// 当前空间可勾选的条目
    pub entries: Vec<CatalogEntry>,
}

impl DatasetDescriptor {
    /// 前端展示用的摘要（不含条目明细）
    pub(crate) fn summary(&self) -> DatasetSummary {
        DatasetSummary {
            name: self.name.clone(),
            label: self.label.clone(),
            owner: self.owner.clone(),
            policy: self.policy,
            schema_version: self.schema_version,
            selectable: self.selectable,
            contains_secret: self.contains_secret,
            default_selected: self.default_selected,
            note: self.note.clone(),
            record_count: self.record_count,
        }
    }
}

/// 前端展示用的数据集摘要
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatasetSummary {
    /// 数据集名
    pub name: String,
    /// 界面文案
    pub label: String,
    /// 归属 owner
    pub owner: String,
    /// 传输策略
    pub policy: TransportPolicy,
    /// 逻辑 schema 版本
    pub schema_version: u32,
    /// 是否可单独勾选
    pub selectable: bool,
    /// 是否含秘密
    pub contains_secret: bool,
    /// 默认是否勾选
    pub default_selected: bool,
    /// 不可单独勾选时的说明
    pub note: Option<String>,
    /// 当前空间可带出的记录数（不是最终进包条数：跟随带出的条目取决于勾选）
    pub record_count: usize,
}

/// 选择集里的一项：数据集名 → 勾选的记录 id 列表
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionEntry {
    /// 数据集名
    pub dataset: String,
    /// 勾选的记录 id
    pub ids: Vec<String>,
}

/// 导出选择（前端提交，默认值由目录给出）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportSelection {
    /// 按条勾选的数据集（L2 = 服务器档案）
    pub entries: Vec<SelectionEntry>,
    /// 整块勾选的数据集（收藏 / 最近使用）
    pub datasets: Vec<String>,
    /// 是否把被引用凭证写进包（false = 只声明条数，导入后由用户重填）
    pub include_credentials: bool,
}

impl ExportSelection {
    /// 空选择：什么都没勾，但默认带出凭证（用户点开目录时的起点）
    pub(crate) fn empty() -> Self {
        Self {
            entries: Vec::new(),
            datasets: Vec::new(),
            include_credentials: true,
        }
    }
}

/// 导出目录（前端渲染用）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportCatalog {
    /// 来源空间 id
    pub source_space_id: String,
    /// 来源空间显示名
    pub source_space_name: String,
    /// 可导出数据集摘要
    pub datasets: Vec<DatasetSummary>,
    /// 全部候选条目（按数据集分组由前端完成；可勾选性看所属数据集的 `selectable`）
    pub entries: Vec<CatalogEntry>,
    /// 默认选择
    pub defaults: ExportSelection,
    /// 结构性提醒（如「当前空间没有可导出的记录」「这些类别属本机事实，不随包搬移」）
    pub warnings: Vec<String>,
}

/// 解析后的选择集：闭包已展开，数据集名 → 记录 id（空列表 = 整块带出）
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedSelection {
    /// 数据集名 → 记录 id
    pub datasets: BTreeMap<String, Vec<String>>,
    /// 是否把凭证记录写进包（false = 只声明条数）
    pub include_credentials: bool,
}

impl ResolvedSelection {
    /// 该数据集是否进包
    pub(crate) fn includes(&self, dataset: &str) -> bool {
        self.datasets.contains_key(dataset)
    }

    /// 该数据集要带出的记录 id（整块数据集为空列表）
    pub(crate) fn ids_of(&self, dataset: &str) -> &[String] {
        self.datasets.get(dataset).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// 由描述符与记录体构造「带记录」的块（条数与摘要一次算好，避免各处手填）
pub(crate) fn block_carrying(
    descriptor: &DatasetDescriptor,
    records: Vec<Value>,
) -> Result<DatasetBlock, String> {
    let record_count = records.len();
    let body = Value::Array(records);
    let sha256 = dataset_digest(&body)?;
    Ok(DatasetBlock {
        name: descriptor.name.clone(),
        schema_version: descriptor.schema_version,
        policy: descriptor.policy,
        record_count,
        sha256,
        records: Some(body),
    })
}

/// 由描述符构造「只声明不携带」的块（凭证未带出、device-local 类别）
pub(crate) fn block_declared(descriptor: &DatasetDescriptor, record_count: usize) -> DatasetBlock {
    DatasetBlock {
        name: descriptor.name.clone(),
        schema_version: descriptor.schema_version,
        policy: descriptor.policy,
        record_count,
        sha256: String::new(),
        records: None,
    }
}

/* ── 导入侧（C3 隔离导入） ─────────────────────────────────────────── */

/// 包内某数据集的携带情况
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CarriedBlock {
    /// 声明条数（含只声明未携带）
    pub record_count: usize,
    /// 记录 id 集合；`None` = 携带了记录但没有 id 概念（整块数据集）
    pub ids: Option<BTreeSet<String>>,
}

/// 导入判定上下文：适配器据此判「新增 / 待补全 / 被排除」
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImportContext {
    /// 来源空间 id（不可信，仅展示与留档）
    pub source_space_id: String,
    /// 数据集名 → 携带情况；**键不存在 = 该数据集没进包**
    pub carried: BTreeMap<String, CarriedBlock>,
}

impl ImportContext {
    /// 该数据集是否携带了记录
    pub(crate) fn is_carried(&self, dataset: &str) -> bool {
        self.carried
            .get(dataset)
            .map(|block| block.ids.is_some() || block.record_count > 0)
            .unwrap_or(false)
    }

    /// 该 id 的记录是否真的在包里
    pub(crate) fn carries_id(&self, dataset: &str, id: &str) -> bool {
        self.carried
            .get(dataset)
            .and_then(|block| block.ids.as_ref())
            .map(|ids| ids.contains(id))
            .unwrap_or(false)
    }

    /// 该 id 是否**引用方声明过、但包里没有**（用于「待补全」判定）
    pub(crate) fn declared_without_id(&self, dataset: &str, id: &str) -> bool {
        match self.carried.get(dataset) {
            None => true,
            Some(block) => match &block.ids {
                None => false,
                Some(ids) => !ids.contains(id),
            },
        }
    }
}

/// 导入后的记录处置结论
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImportOutcome {
    /// 会写入新空间
    Added,
    /// 会写入，但引用物没进包（如档案的凭证），导入后仍是待补全
    PendingReference,
    /// 本次不带入（用户排除、或所属 owner 不支持）
    Excluded,
}

/// 导入计划条目（预览列表项）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPlanItem {
    /// 所属数据集
    pub dataset: String,
    /// 记录 id（整块数据集为空串）
    pub id: String,
    /// 展示名
    pub label: String,
    /// 处置结论
    pub outcome: ImportOutcome,
    /// 结论说明（如「未绑定凭证，导入后需重新绑定」）
    pub note: Option<String>,
}

impl ImportPlanItem {
    /// 会写入新空间
    pub(crate) fn added(dataset: &str, id: &str, label: &str) -> Self {
        Self {
            dataset: dataset.to_string(),
            id: id.to_string(),
            label: label.to_string(),
            outcome: ImportOutcome::Added,
            note: None,
        }
    }

    /// 会写入，但引用的东西没随包带来（导入后仍需用户补全）
    pub(crate) fn pending_reference(dataset: &str, id: &str, label: &str, note: &str) -> Self {
        Self {
            dataset: dataset.to_string(),
            id: id.to_string(),
            label: label.to_string(),
            outcome: ImportOutcome::PendingReference,
            note: Some(note.to_string()),
        }
    }

    /// 本次不带入（用户排除、owner 不支持、只声明未携带）
    pub(crate) fn excluded(dataset: &str, label: &str, note: &str) -> Self {
        Self {
            dataset: dataset.to_string(),
            id: String::new(),
            label: label.to_string(),
            outcome: ImportOutcome::Excluded,
            note: Some(note.to_string()),
        }
    }
}

/// 导入选择（前端提交）：要带入的数据集；未列出的数据集一律排除并回显
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportSelection {
    /// 要导入的数据集名
    pub datasets: Vec<String>,
}

impl ImportSelection {
    /// 是否选入该数据集
    pub(crate) fn includes(&self, dataset: &str) -> bool {
        self.datasets.iter().any(|item| item == dataset)
    }
}

/// 导入报告（提交后返回；条数按数据集分项，便于界面逐项显示「带进来什么」）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportReport {
    /// 新空间 id
    pub space_id: String,
    /// 新空间名
    pub space_name: String,
    /// 包标识（查「是否导入过」用）
    pub package_id: String,
    /// 来源空间 id / 名（留档展示；不可信）
    pub source_space_id: String,
    /// 来源空间名（留档展示；不可信）
    pub source_space_name: String,
    /// 导入完成时间（RFC3339）
    pub imported_at: String,
    /// 数据集名 → 实际写入条数
    pub counts: BTreeMap<String, usize>,
    /// 数据集名 → 包里声明的条数（含未导入的，便于对账）
    pub declared_counts: BTreeMap<String, usize>,
    /// 待补全引用（如「档案 X 的凭证未随包带入」）
    pub pending: Vec<String>,
    /// 本次排除的数据集（含用户排除与不支持）
    pub excluded: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 构造一个内容自洽的块：记录体与 sha256/条数一致
    fn block(name: &str, policy: TransportPolicy, items: Vec<Value>) -> DatasetBlock {
        let records = Value::Array(items);
        DatasetBlock {
            name: name.into(),
            schema_version: 1,
            policy,
            record_count: records.as_array().map(Vec::len).unwrap_or(0),
            sha256: dataset_digest(&records).unwrap_or_default(),
            records: Some(records),
        }
    }

    /// 自洽清单通过校验
    #[test]
    fn consistent_manifest_passes() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        manifest.datasets.push(block(
            "ssh.profiles",
            TransportPolicy::Portable,
            vec![json!({"id": "p1", "host": "example.com"})],
        ));
        manifest.dependencies.push(DependencyEdge {
            kind: "credential".into(),
            from_id: "p1".into(),
            to_id: "cred-1".into(),
        });
        assert!(validate_manifest(&manifest).is_ok());
        assert_eq!(manifest.platform, std::env::consts::OS);
        assert!(manifest.created_at.len() > 10);
    }

    /// 重名块 / 空清单 / schemaVersion 0 被拒
    #[test]
    fn bad_shapes_rejected() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        assert!(validate_manifest(&manifest).is_err()); // 空清单

        manifest.datasets.push(block(
            "ssh.profiles",
            TransportPolicy::Portable,
            vec![json!({"id": "p1"})],
        ));
        manifest.datasets.push(block(
            "ssh.profiles",
            TransportPolicy::Portable,
            vec![json!({"id": "p2"})],
        ));
        assert!(validate_manifest(&manifest).is_err()); // 重名

        manifest.datasets.pop();
        manifest.datasets[0].schema_version = 0;
        assert!(validate_manifest(&manifest).is_err()); // 版本 0

        manifest.datasets[0].schema_version = 1;
        assert!(validate_manifest(&manifest).is_ok());
    }

    /// secret 块可携带记录（勾选后经加密包，§13.1）；只声明不带出时也合法；
    /// device-local 块携带记录被拒；可搬移块声明有记录却无记录体被拒
    #[test]
    fn secret_carries_records_only_when_selected() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        manifest.datasets.push(block(
            "vault.credentials",
            TransportPolicy::Secret,
            vec![json!({"id": "c1", "password": "x"})],
        ));
        assert!(
            validate_manifest(&manifest).is_ok(),
            "勾选带出凭证时 secret 块必须能携带记录"
        );

        // 未勾选：只声明条数，不带记录体
        manifest.datasets[0].records = None;
        assert!(validate_manifest(&manifest).is_ok());

        // device-local 永远只作声明
        let mut device = PackageManifest::new("default", "默认空间");
        device.datasets.push(block(
            "frp.paths",
            TransportPolicy::DeviceLocal,
            vec![json!({"id": "p1"})],
        ));
        assert!(validate_manifest(&device).is_err());
        device.datasets[0].records = None;
        assert!(validate_manifest(&device).is_ok());

        // 可搬移块声明有记录却无记录体 = 清单自相矛盾
        manifest.datasets[0].policy = TransportPolicy::Portable;
        assert!(validate_manifest(&manifest).is_err());

        manifest.datasets[0].record_count = 0;
        assert!(validate_manifest(&manifest).is_ok());
    }

    /// 条数与记录体不一致、摘要被改动都被拒（防「清单说一套、内容另一套」）
    #[test]
    fn count_and_digest_mismatch_rejected() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        manifest.datasets.push(block(
            "ssh.bookmarks",
            TransportPolicy::Portable,
            vec![json!({"id": "b1"}), json!({"id": "b2"})],
        ));
        manifest.datasets[0].record_count = 3;
        assert!(validate_manifest(&manifest).is_err());

        manifest.datasets[0].record_count = 2;
        manifest.datasets[0].sha256 = "00".repeat(32);
        assert!(validate_manifest(&manifest).is_err());

        manifest.datasets[0].sha256 =
            dataset_digest(&manifest.datasets[0].records.clone().unwrap_or(Value::Null))
                .unwrap_or_default();
        assert!(validate_manifest(&manifest).is_ok());
    }

    /// 单条记录超限与嵌套过深都被拒
    #[test]
    fn oversized_and_deep_records_rejected() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        let big = "x".repeat(MAX_RECORD_BYTES + 1);
        manifest.datasets.push(block(
            "ssh.tunnels",
            TransportPolicy::Portable,
            vec![json!({"id": "t1", "blob": big})],
        ));
        assert!(validate_manifest(&manifest).is_err());

        // 嵌套深度：构造 MAX_JSON_DEPTH + 1 层的数组
        let mut deep = json!(0);
        for _ in 0..MAX_JSON_DEPTH {
            deep = Value::Array(vec![deep]);
        }
        assert_eq!(json_depth(&deep), MAX_JSON_DEPTH + 1);
        manifest.datasets[0] = block("ssh.tunnels", TransportPolicy::Portable, vec![deep]);
        assert!(validate_manifest(&manifest).is_err());
    }

    /// 记录总数超上限被拒
    #[test]
    fn too_many_records_rejected() {
        let mut manifest = PackageManifest::new("default", "默认空间");
        // 不构造 5 万条真实记录：直接把声明条数与记录体长度对齐到上限 +1
        let items: Vec<Value> = (0..(MAX_RECORDS_PER_PACKAGE + 1))
            .map(|_| json!(0))
            .collect();
        manifest
            .datasets
            .push(block("history.recent", TransportPolicy::History, items));
        assert!(validate_manifest(&manifest).is_err());
    }

    /// json_depth：标量 1 层、平铺数组 2 层、对象内数组 3 层
    #[test]
    fn json_depth_counts_nesting() {
        assert_eq!(json_depth(&json!(1)), 1);
        assert_eq!(json_depth(&json!([1, 2])), 2);
        assert_eq!(json_depth(&json!({"a": [1]})), 3);
    }
}
