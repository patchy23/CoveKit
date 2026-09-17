//! 可移植应用和工具设置；仅接受公开 schema 白名单，设备路径和系统开关不进入包。

use super::super::{
    adapter::{self, DatasetAdapter, MergeTarget, StagingTarget},
    records,
    types::*,
};
use crate::framework::preferences;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use tauri::AppHandle;

const APP: &str = "settings.application";
const TOOLS: &str = "settings.tools";
struct SettingsAdapter;

fn read(dataset: &str, map: &Map<String, Value>) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    match dataset {
        APP => {
            for key in ["theme", "language"] {
                if let Some(value) = map.get(key) {
                    result.push(json!({"id":key,"value":value}));
                }
            }
        }
        TOOLS => {
            if let Some(value) = map
                .get("tools")
                .and_then(|tools| tools.get("ssh"))
                .and_then(|ssh| ssh.get("idleDisconnectMinutes"))
            {
                result.push(json!({"id":"ssh.idleDisconnectMinutes","value":value}));
            }
        }
        _ => return Err("未知设置数据集".into()),
    }
    Ok(result)
}

fn validate(dataset: &str, record: &Value) -> Result<(), String> {
    records::fields(record, &["id", "value"])?;
    let id = records::string(record, "id")?;
    let value = records::string(record, "value")?;
    let valid = match (dataset, id) {
        (APP, "theme") => matches!(value, "light" | "dark" | "system"),
        (APP, "language") => matches!(value, "zh-CN" | "en-US"),
        (TOOLS, "ssh.idleDisconnectMinutes") => {
            matches!(value, "0" | "1" | "10" | "30" | "60" | "120")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("设置字段或取值不在可移植白名单内".into())
    }
}

fn apply(
    map: &mut Map<String, Value>,
    dataset: &str,
    items: &[Value],
    target: Option<&MergeTarget<'_>>,
) -> Result<usize, String> {
    let mut count = 0;
    if target.is_some_and(|target| target.mode == ImportMode::Overwrite) {
        match dataset {
            APP => {
                map.remove("theme");
                map.remove("language");
            }
            TOOLS => {
                if let Some(ssh) = map
                    .get_mut("tools")
                    .and_then(|tools| tools.get_mut("ssh"))
                    .and_then(Value::as_object_mut)
                {
                    ssh.remove("idleDisconnectMinutes");
                }
            }
            _ => return Err("未知设置数据集".into()),
        }
    }
    for record in items {
        validate(dataset, record)?;
        let id = records::string(record, "id")?;
        if target.is_some_and(|target| !records::writable(target, dataset, id)) {
            continue;
        }
        if dataset == APP {
            map.insert(id.into(), record["value"].clone());
        } else {
            let tools = map
                .entry("tools")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("工具设置结构损坏")?;
            let ssh = tools
                .entry("ssh")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("SSH 设置结构损坏")?;
            ssh.insert("idleDisconnectMinutes".into(), record["value"].clone());
            // 现有 SSH 启动流程以此标记识别已明确设置的值，避免首次打开重置导入值。
            ssh.insert("idleDisconnectV2".into(), Value::Bool(true));
        }
        count += 1;
    }
    Ok(count)
}

impl DatasetAdapter for SettingsAdapter {
    fn owner(&self) -> &'static str {
        "settings"
    }
    fn storage_files(&self) -> Vec<String> {
        vec!["preferences.json".into()]
    }
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        let map = preferences::read_current(app)?;
        [(APP, "应用偏好"), (TOOLS, "工具偏好")]
            .into_iter()
            .map(|(name, label)| records::descriptor(self.owner(), name, label, &read(name, &map)?))
            .collect()
    }
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        records::select(read(dataset, &preferences::read_current(app)?)?, ids)
    }
    fn validate_records(&self, dataset: &str, items: &[Value]) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        for item in items {
            validate(dataset, item)?;
            if !ids.insert(records::string(item, "id")?) {
                return Err("设置键重复".into());
            }
        }
        Ok(())
    }
    fn enumerate_references(&self, _: &str, _: &[Value]) -> Result<Vec<DependencyEdge>, String> {
        Ok(Vec::new())
    }
    fn plan_import(
        &self,
        dataset: &str,
        items: &[Value],
        context: &ImportContext<'_>,
    ) -> Result<Vec<ImportPlanItem>, String> {
        self.validate_records(dataset, items)?;
        let local = match context.merge {
            Some(view) => read(dataset, &preferences::read_current(view.app)?)?,
            None => Vec::new(),
        };
        records::plan(dataset, items, &local, context, true)
    }
    fn apply_to_staging(
        &self,
        dataset: &str,
        items: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        let path = preferences::path_in(&target.root);
        let mut map = preferences::read_at(&path)?;
        let count = apply(&mut map, dataset, items, None)?;
        preferences::write_at(&path, &map)?;
        Ok(count)
    }
    fn apply_merge(
        &self,
        blocks: &[(String, Vec<Value>)],
        target: &MergeTarget<'_>,
    ) -> Result<BTreeMap<String, usize>, String> {
        let path = preferences::path_in(target.space_root);
        let mut map = preferences::read_at(&path)?;
        let mut counts = BTreeMap::new();
        for (dataset, items) in blocks {
            counts.insert(
                dataset.clone(),
                apply(&mut map, dataset, items, Some(target))?,
            );
        }
        preferences::write_at(&path, &map)?;
        Ok(counts)
    }
}

/// 注册框架可移植设置数据集。
pub(super) fn register() {
    static ADAPTER: SettingsAdapter = SettingsAdapter;
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_whitelist_rejects_device_fields_and_preserves_unselected_values() {
        assert!(validate(APP, &json!({"id":"launchAtStartup","value":"true"})).is_err());
        assert!(validate(TOOLS, &json!({"id":"frp.profileDir","value":"C:/private"})).is_err());
        let mut map = json!({"language":"zh-CN","tools":{"frp":{"profileDir":"local"}}})
            .as_object()
            .unwrap()
            .clone();
        apply(&mut map, APP, &[json!({"id":"theme","value":"dark"})], None).unwrap();
        assert_eq!(map["language"], "zh-CN");
        assert_eq!(map["tools"]["frp"]["profileDir"], "local");
        apply(
            &mut map,
            TOOLS,
            &[json!({"id":"ssh.idleDisconnectMinutes","value":"0"})],
            None,
        )
        .unwrap();
        assert_eq!(map["tools"]["ssh"]["idleDisconnectMinutes"], "0");
        assert_eq!(map["tools"]["ssh"]["idleDisconnectV2"], true);
    }
}
