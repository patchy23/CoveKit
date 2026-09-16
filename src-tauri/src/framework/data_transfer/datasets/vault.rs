//! 凭证数据集适配（`vault.credentials` · policy = `secret`）
//!
//! 三条规矩来自任务书 §13.1 / §13.7：
//! - 凭证**不可单独勾选**：只有被所选档案引用的凭证才可能进包（按引用自动带出）。
//! - 是否把记录写进包由用户确认（`includeCredentials`）：不确认时块只声明条数，
//!   导入侧据此提示「哪些凭证需要重填」——这一条是「明确排除」而不是静默丢弃。
//! - 凭证库不可读时**直接报错**：把「读不到」当成「没有凭证」，会让用户以为包是完整的。

use serde_json::Value;
use tauri::AppHandle;

use super::super::adapter::{self, DatasetAdapter};
use super::super::types::{CatalogEntry, DatasetDescriptor, DependencyEdge, TransportPolicy};
use crate::framework::vault::models::Credential;
use crate::framework::vault::{credential_summary, credentials_read_all};

/// 数据集名（owner = `vault`）
pub(crate) const DATASET: &str = "vault.credentials";
/// 数据集 schema 版本（逻辑结构变更时 +1，导入侧据此判断能否吃下）
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// 凭证数据集适配器
struct VaultAdapter;

impl DatasetAdapter for VaultAdapter {
    /// 归属 owner（与 `patchybox_module!` 的模块标识一致）
    fn owner(&self) -> &'static str {
        "vault"
    }

    /// 声明凭证数据集：不可单独勾选、含秘密，条目用于预览「将带出哪些凭证」
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        let credentials = credentials_read_all(app)?;
        let entries = credentials
            .iter()
            .map(|credential| CatalogEntry {
                dataset: DATASET.to_string(),
                id: credential.id.clone(),
                label: credential.name.clone(),
                // 展示用脱敏摘要：秘密明文不离开读取入口，也不进导出目录
                detail: credential_summary(credential).masked,
                dependencies: Vec::new(),
                note: None,
            })
            .collect::<Vec<_>>();
        Ok(vec![DatasetDescriptor {
            name: DATASET.to_string(),
            label: "凭证".to_string(),
            owner: self.owner().to_string(),
            policy: TransportPolicy::Secret,
            schema_version: SCHEMA_VERSION,
            // 勾选粒度到档案级：凭证由引用自动带出，界面只能确认「带/不带」
            selectable: false,
            contains_secret: true,
            default_selected: false,
            pulls: Vec::new(),
            note: Some("按所选档案的引用自动带出；不确认带出时，包内只声明条数".to_string()),
            record_count: entries.len(),
            entries,
        }])
    }

    /// 按 id 取凭证记录（含秘密明文 —— 只在加密包内出现，`secret` 策略已在清单里标明）
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        if dataset != DATASET {
            return Err(format!("凭证适配器不支持数据集 {dataset}"));
        }
        let credentials = credentials_read_all(app)?;
        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            let credential = credentials
                .iter()
                .find(|item| &item.id == id)
                .ok_or_else(|| format!("凭证 {id} 不存在（列表可能已过期，请刷新后重试）"))?;
            records.push(credential_record(credential)?);
        }
        Ok(records)
    }

    /// 记录体检：导入侧要靠 `id` 建引用、靠 `fields` 还原秘密，缺一不可
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        if dataset != DATASET {
            return Err(format!("凭证适配器不支持数据集 {dataset}"));
        }
        for record in records {
            let id = record.get("id").and_then(Value::as_str).unwrap_or_default();
            if id.trim().is_empty() {
                return Err("凭证记录缺少 id".into());
            }
            let name = record
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if name.trim().is_empty() {
                return Err(format!("凭证 {id} 缺少名称"));
            }
            if !record.get("fields").is_some_and(Value::is_object) {
                return Err(format!("凭证 {id} 缺少秘密字段结构"));
            }
        }
        Ok(())
    }

    /// 凭证是依赖树的叶子：没有它引用的别的东西
    fn enumerate_references(
        &self,
        dataset: &str,
        _records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        if dataset != DATASET {
            return Err(format!("凭证适配器不支持数据集 {dataset}"));
        }
        Ok(Vec::new())
    }
}

/// 单条凭证 → 传输记录（逻辑结构 = `Credential` 的 serde 形态）
fn credential_record(credential: &Credential) -> Result<Value, String> {
    serde_json::to_value(credential).map_err(|e| format!("凭证记录序列化失败: {e}"))
}

/// 登记凭证适配器（装配阶段调用一次）
pub(crate) fn register() {
    static ADAPTER: VaultAdapter = VaultAdapter;
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::vault::models::{CredentialFields, CredentialKind};

    /// 记录体检：合法凭证通过，缺 id / 缺名称 / 缺字段结构被拒
    #[test]
    fn validate_records_rejects_incomplete_credentials() {
        let adapter = VaultAdapter;
        let good = serde_json::json!({
            "id": "cred-1",
            "name": "跳板机",
            "fields": {"kind": "password", "username": "root", "password": "p"}
        });
        assert!(adapter.validate_records(DATASET, &[good.clone()]).is_ok());

        let mut no_id = good.clone();
        no_id["id"] = serde_json::json!("  ");
        assert!(adapter
            .validate_records(DATASET, &[no_id])
            .unwrap_err()
            .contains("id"));

        let mut no_name = good.clone();
        no_name["name"] = serde_json::json!("");
        assert!(adapter
            .validate_records(DATASET, &[no_name])
            .unwrap_err()
            .contains("名称"));

        let mut no_fields = good;
        no_fields["fields"] = serde_json::json!("root");
        assert!(adapter
            .validate_records(DATASET, &[no_fields])
            .unwrap_err()
            .contains("秘密字段结构"));
    }

    /// 记录序列化沿用 `Credential` 的逻辑结构（含秘密字段），id / name 原样保留
    #[test]
    fn record_keeps_logical_shape() {
        let credential = Credential {
            id: "cred-1".into(),
            name: "跳板机".into(),
            kind: CredentialKind::Password,
            fields: CredentialFields::Password {
                username: "root".into(),
                password: "secret".into(),
            },
            note: String::new(),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        };
        let value = credential_record(&credential).expect("序列化成功");
        assert_eq!(value["id"], serde_json::json!("cred-1"));
        assert_eq!(value["name"], serde_json::json!("跳板机"));
        assert_eq!(value["fields"]["type"], serde_json::json!("password"));
        assert_eq!(value["fields"]["username"], serde_json::json!("root"));
    }

    /// 凭证是叶子：引用集合为空
    #[test]
    fn credentials_are_leaves() {
        let adapter = VaultAdapter;
        assert!(adapter
            .enumerate_references(DATASET, &[])
            .unwrap()
            .is_empty());
    }
}
