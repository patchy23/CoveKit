//! 框架 · 凭证引用自报（可靠性 T09）
//!
//! 为什么需要：凭证能不能删，取决于「还有哪些配置在用它」。这些配置分散在各插件自己的库里，
//! 框架不该去写各插件的业务 SQL，也不该维护一份前端镜像（镜像会过期，过期计数就是错误提示）。
//!
//! 契约：
//! - 插件实现只读的 [`CredentialReferenceProvider`] 并在装配阶段 `register`；
//! - 框架按 owner 批量扫描（一次查询覆盖全部凭证，避免每个凭证各开一次库）；
//! - 扫描失败必须返回 unknown 并把原因带上，**不得假装 0 条引用**（假装 0 会诱导误删）。

use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::AppHandle;

/// 一条引用：哪个插件的哪个对象引用了该凭证
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialReference {
    /// 插件 id
    pub owner: String,
    /// 被引用的凭证 id（批量扫描据此过滤）
    pub credential_id: String,
    /// 引用对象的稳定 id（连接 id / 配置 id）
    pub object_id: String,
    /// 引用对象的展示名
    pub object_name: String,
}

/// 扫描状态：库不可读、表结构不认识等都要如实上报
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "reason")]
pub enum ScanState {
    /// 扫描成功（0 条也是成功）
    Ok,
    /// 扫描失败，附带原因（前端必须提示「未知」而不是「无引用」）
    Unknown(String),
}

/// 单插件对该凭证的引用情况
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReferences {
    /// 插件 id
    pub owner: String,
    /// 引用的对象清单
    pub references: Vec<CredentialReference>,
    /// 扫描状态
    pub status: ScanState,
}

/// 汇总结果（前端删除确认与连接时改绑入口都用它）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSummary {
    /// 凭证 id
    pub credential_id: String,
    /// 各插件的结果（按 owner 排序，便于稳定展示）
    pub owners: Vec<OwnerReferences>,
    /// 引用总数（只统计扫描成功的插件）
    pub total: usize,
    /// 扫描失败、计数未知的插件
    pub unknown_owners: Vec<String>,
}

impl ReferenceSummary {
    /// 是否存在「计数未知」的插件：未知时不允许无提示地按「无引用」删除
    pub fn has_unknown(&self) -> bool {
        !self.unknown_owners.is_empty()
    }

    /// 删除前复核：引用数与前端确认时的不一致则为过期确认
    pub fn matches_total(&self, expected: usize) -> bool {
        !self.has_unknown() && self.total == expected
    }
}

/// 插件自报凭证引用的只读能力
///
/// 实现方只查询自己的存储；不得写入任何数据（删除冻结依赖它的可信度）。
pub trait CredentialReferenceProvider: Send + Sync {
    /// 插件 id（与 IPC owner 一致）
    fn owner(&self) -> &'static str;

    /// 批量扫描：一次性返回「本插件引用了哪些凭证」的全量映射
    ///
    /// 返回 `Err` 表示库不可读或结构不认识，框架会把它记为 unknown。
    fn scan(&self, app: &AppHandle) -> Result<Vec<CredentialReference>, String>;
}

/// 全局注册表（进程内）
fn registry() -> &'static Mutex<Vec<&'static dyn CredentialReferenceProvider>> {
    static REGISTRY: OnceLock<Mutex<Vec<&'static dyn CredentialReferenceProvider>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// 插件装配阶段登记（重复登记同一 owner 直接覆盖，保持幂等）
///
/// 提供者必须是 `'static`：插件在启动装配时登记，生命周期覆盖整个进程。
pub fn register(provider: &'static dyn CredentialReferenceProvider) {
    let Ok(mut list) = registry().lock() else {
        eprintln!(
            "[credential_refs] 注册表锁定失败，跳过登记: {}",
            provider.owner()
        );
        return;
    };
    list.retain(|existing| existing.owner() != provider.owner());
    list.push(provider);
}

/// 扫描结果聚合（纯函数，便于单测）：把各插件的扫描结果汇总成一个凭证的引用概况
///
/// 只统计扫描成功的插件；失败的插件进入 `unknown_owners`，计数不参与 `total`，
/// 前端据此显示「部分插件计数未知」而不是「无引用」。
pub fn aggregate(
    credential_id: &str,
    scanned: Vec<(&'static str, Result<Vec<CredentialReference>, String>)>,
) -> ReferenceSummary {
    let mut owners: Vec<OwnerReferences> = Vec::new();
    let mut unknown_owners: Vec<String> = Vec::new();
    let mut total = 0usize;
    for (owner, result) in scanned {
        match result {
            Ok(all) => {
                let mut references: Vec<CredentialReference> = all
                    .into_iter()
                    .filter(|item| item.credential_id == credential_id)
                    .collect();
                references.sort_by(|a, b| {
                    a.object_name
                        .cmp(&b.object_name)
                        .then_with(|| a.object_id.cmp(&b.object_id))
                });
                total += references.len();
                owners.push(OwnerReferences {
                    owner: owner.to_string(),
                    references,
                    status: ScanState::Ok,
                });
            }
            Err(reason) => {
                unknown_owners.push(owner.to_string());
                owners.push(OwnerReferences {
                    owner: owner.to_string(),
                    references: Vec::new(),
                    status: ScanState::Unknown(reason),
                });
            }
        }
    }
    owners.sort_by(|a, b| a.owner.cmp(&b.owner));
    unknown_owners.sort();
    ReferenceSummary {
        credential_id: credential_id.to_string(),
        owners,
        total,
        unknown_owners,
    }
}

/// 查询某凭证在各插件中的引用（按 owner 批量扫描，每个插件只查一次库）
pub fn summarize(app: &AppHandle, credential_id: &str) -> ReferenceSummary {
    let providers: Vec<&'static dyn CredentialReferenceProvider> = registry()
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default();
    let scanned = providers
        .into_iter()
        .map(|provider| (provider.owner(), provider.scan(app)))
        .collect();
    aggregate(credential_id, scanned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条引用
    fn reference(
        owner: &'static str,
        credential_id: &str,
        id: &str,
        name: &str,
    ) -> CredentialReference {
        CredentialReference {
            owner: owner.to_string(),
            credential_id: credential_id.to_string(),
            object_id: id.to_string(),
            object_name: name.to_string(),
        }
    }

    /// 多个插件引用同一凭证：计数相加，按对象名排序
    #[test]
    fn counts_references_across_owners() {
        let summary = aggregate(
            "cred-1",
            vec![
                (
                    "ssh",
                    Ok(vec![
                        reference("ssh", "cred-1", "p2", "生产机"),
                        reference("ssh", "cred-1", "p1", "测试机"),
                    ]),
                ),
                ("dns", Ok(vec![reference("dns", "cred-1", "d1", "主域名")])),
            ],
        );
        assert_eq!(summary.total, 3);
        assert!(!summary.has_unknown());
        assert_eq!(summary.owners.len(), 2);
        assert_eq!(summary.owners[0].owner, "dns");
        assert_eq!(summary.owners[1].references[0].object_name, "测试机");
    }

    /// 只统计目标凭证：其他凭证的引用不能混进来
    #[test]
    fn ignores_other_credentials() {
        let summary = aggregate(
            "cred-1",
            vec![(
                "ssh",
                Ok(vec![
                    reference("ssh", "cred-1", "p1", "A"),
                    reference("ssh", "cred-2", "p2", "B"),
                ]),
            )],
        );
        assert_eq!(summary.total, 1);
        assert_eq!(summary.owners[0].references[0].object_id, "p1");
    }

    /// 插件扫描失败：计入未知，不得当成 0 条引用
    #[test]
    fn scan_failure_is_unknown_not_zero() {
        let summary = aggregate(
            "cred-1",
            vec![
                ("ssh", Err("库不可读".to_string())),
                ("dns", Ok(Vec::new())),
            ],
        );
        assert_eq!(summary.total, 0);
        assert!(summary.has_unknown());
        assert_eq!(summary.unknown_owners, vec!["ssh".to_string()]);
        let ssh = summary
            .owners
            .iter()
            .find(|o| o.owner == "ssh")
            .expect("ssh 结果存在");
        assert_eq!(ssh.status, ScanState::Unknown("库不可读".to_string()));
        // 未知状态下「按 0 条引用」删除必须被拒绝
        assert!(!summary.matches_total(0));
    }

    /// 删除前复核：引用数与确认时一致才允许直接删
    #[test]
    fn matches_total_detects_stale_confirmation() {
        let summary = aggregate(
            "cred-1",
            vec![(
                "ssh",
                Ok(vec![
                    reference("ssh", "cred-1", "p1", "A"),
                    reference("ssh", "cred-1", "p2", "B"),
                ]),
            )],
        );
        assert!(summary.matches_total(2));
        assert!(!summary.matches_total(1), "确认期间新增引用必须触发复核");
    }

    /// 空凭证 id 查询返回空结果（不 panic）
    #[test]
    fn empty_scan_is_ok_with_zero() {
        let summary = aggregate("cred-1", vec![("dns", Ok(Vec::new()))]);
        assert_eq!(summary.total, 0);
        assert!(summary.matches_total(0));
    }
}
