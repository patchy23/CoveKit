//! 逻辑记录适配的公共编排：目录、稳定身份、冲突与单库事务；业务 SQL 留在 owner。

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;
use serde_json::Value;
use tauri::AppHandle;

use super::adapter::{DatasetAdapter, MergeTarget, StagingTarget};
use super::types::*;
use crate::framework::store::PluginDb;

/// owner 自有逻辑记录与数据库操作，框架不解释业务表。
pub(crate) trait RecordStore: Send + Sync {
    /// 注册归属。
    fn owner(&self) -> &'static str;
    /// 历史存储键。
    fn storage(&self) -> &'static str;
    /// 只追加的数据库 schema。
    fn migrations(&self) -> &'static [&'static str];
    /// 数据集名、显示名；顺序即依赖写入顺序。
    fn datasets(&self) -> &'static [(&'static str, &'static str)];
    /// 读取无物理主键的逻辑记录。
    fn read(&self, conn: &Connection, dataset: &str) -> Result<Vec<Value>, String> {
        let mut records = Vec::new();
        self.visit(conn, dataset, &mut |record| { records.push(record); Ok(()) })?;
        Ok(records)
    }
    /// 顺序读取逻辑记录；取消或预算失败必须从回调传播并停止游标。
    fn visit(
        &self,
        conn: &Connection,
        dataset: &str,
        emit: &mut dyn FnMut(Value) -> Result<(), String>,
    ) -> Result<(), String>;
    /// 验证字段与业务约束。
    fn validate(&self, dataset: &str, record: &Value) -> Result<(), String>;
    /// 写入已经改写身份的记录，不启动任何业务操作。
    fn write(&self, conn: &Connection, dataset: &str, record: &Value) -> Result<(), String>;
    /// 显式覆盖选中数据集。
    fn clear(&self, conn: &Connection, dataset: &str) -> Result<(), String>;
    /// 单槽数据不允许生成第二份。
    fn singleton(&self, _dataset: &str) -> bool {
        false
    }
    /// 记录导入后的补录说明。
    fn note(&self, _dataset: &str, _record: &Value) -> Option<String> {
        None
    }
    /// 引用字段及其目标数据集。
    fn reference(&self, _dataset: &str) -> Option<(&'static str, &'static str)> {
        None
    }
}

/// 复用公共传输协议的单库 owner 适配器。
pub(crate) struct RecordsAdapter<S: RecordStore>(pub S);

impl<S: RecordStore> RecordsAdapter<S> {
    fn database(&self, app: &AppHandle) -> Result<PluginDb, String> {
        PluginDb::open(app, self.0.storage(), self.0.migrations())
    }

    fn check_dataset(&self, dataset: &str) -> Result<(), String> {
        if self.0.datasets().iter().any(|(name, _)| *name == dataset) {
            Ok(())
        } else {
            Err(format!("不支持数据集 {dataset}"))
        }
    }

    fn apply(
        &self,
        conn: &Connection,
        blocks: &[(String, Vec<Value>)],
        target: Option<&MergeTarget<'_>>,
    ) -> Result<BTreeMap<String, usize>, String> {
        let mut counts = BTreeMap::new();
        for (dataset, _) in self.0.datasets() {
            let Some((_, records)) = blocks.iter().find(|(name, _)| name == dataset) else {
                continue;
            };
            self.validate_records(dataset, records)?;
            if target.is_some_and(|target| target.mode == ImportMode::Overwrite) {
                self.0.clear(conn, dataset)?;
            }
            let mut count = 0;
            for record in records {
                let source_id = string(record, "id")?;
                let mut output = record.clone();
                if let Some(target) = target {
                    if !writable(target, dataset, source_id) {
                        continue;
                    }
                    remap_record(
                        &mut output,
                        dataset,
                        self.0.reference(dataset),
                        target.id_map,
                    )?;
                }
                self.0.write(conn, dataset, &output)?;
                count += 1;
            }
            counts.insert(dataset.to_string(), count);
        }
        Ok(counts)
    }
}

impl<S: RecordStore> DatasetAdapter for RecordsAdapter<S> {
    fn owner(&self) -> &'static str {
        self.0.owner()
    }
    fn storage_files(&self) -> Vec<String> {
        vec![format!("data/{}.db", self.0.storage())]
    }
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        self.database(app)?.with_conn(|conn| {
            self.0
                .datasets()
                .iter()
                .map(|(name, label)| {
                    let mut result = descriptor(self.owner(), name, label, &[])?;
                    if let Some((field, dataset)) = self.0.reference(name) {
                        result.pulls.push(DatasetPull {
                            kind: field.into(),
                            dataset: dataset.into(),
                        });
                    }
                    self.0.visit(conn, name, &mut |record| {
                        let mut entry = CatalogEntry {
                            dataset: (*name).into(),
                            id: string(&record, "id")?.into(),
                            label: self::label(&record).into(),
                            detail: String::new(),
                            dependencies: Vec::new(),
                            note: None,
                        };
                        if let Some((field, _)) = self.0.reference(name) {
                            if let Some(id) = record
                                .get(field)
                                .and_then(Value::as_str)
                                .filter(|id| !id.is_empty())
                            {
                                entry.dependencies.push(DependencyEdge {
                                    kind: field.into(),
                                    from_id: entry.id.clone(),
                                    to_id: id.into(),
                                });
                            }
                        }
                        result.entries.push(entry);
                        Ok(())
                    })?;
                    result.record_count = result.entries.len();
                    Ok(result)
                })
                .collect()
        })
    }
    fn visit_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
        emit: &mut dyn FnMut(Value) -> Result<(), String>,
    ) -> Result<(), String> {
        self.check_dataset(dataset)?;
        self.database(app)?.with_conn(|conn| {
            visit_selected(ids, emit, |selected| self.0.visit(conn, dataset, selected))
        })
    }
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        self.check_dataset(dataset)?;
        let mut seen = BTreeSet::new();
        for record in records {
            let id = string(record, "id")?;
            if id.is_empty() || !seen.insert(id) {
                return Err(format!("{dataset} 的记录身份为空或重复"));
            }
            self.0.validate(dataset, record)?;
        }
        Ok(())
    }
    fn enumerate_references(
        &self,
        dataset: &str,
        records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        self.check_dataset(dataset)?;
        let Some((field, _)) = self.0.reference(dataset) else {
            return Ok(Vec::new());
        };
        records
            .iter()
            .filter(|record| {
                record
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|id| !id.is_empty())
            })
            .map(|record| {
                Ok(DependencyEdge {
                    kind: field.into(),
                    from_id: string(record, "id")?.into(),
                    to_id: string(record, field)?.into(),
                })
            })
            .collect()
    }
    fn plan_import(
        &self,
        dataset: &str,
        records: &[Value],
        context: &ImportContext<'_>,
    ) -> Result<Vec<ImportPlanItem>, String> {
        self.validate_records(dataset, records)?;
        if let Some((field, dependency)) = self.0.reference(dataset) {
            for record in records {
                let id = string(record, field)?;
                if !id.is_empty() && !context.carries_id(dependency, id) {
                    return Err(format!(
                        "{dataset} 缺少关联数据集 {dependency} 的记录，请同时导入"
                    ));
                }
            }
        }
        let local = match context.merge {
            Some(view) => self
                .database(view.app)?
                .with_conn(|conn| self.0.read(conn, dataset))?,
            None => Vec::new(),
        };
        let mut normalized = records.to_vec();
        if let (Some((field, dependency)), Some(view)) = (self.0.reference(dataset), context.merge)
        {
            let dependencies = self
                .database(view.app)?
                .with_conn(|conn| self.0.read(conn, dependency))?;
            for record in &mut normalized {
                let source = string(record, field)?;
                if view.core.decisions.get(&(dependency.into(), source.into()))
                    == Some(&ConflictDecision::KeepBoth)
                {
                    // 新副本的目标身份在整份计划完成后才生成，预览不能把旧引用误判为相同。
                    record[field] = Value::String(format!("pending-copy:{source}"));
                    continue;
                }
                if let Some(entry) = view.core.lineage.entries.iter().rev().find(|entry| {
                    entry.source_space_id == context.source_space_id
                        && entry.dataset == dependency
                        && entry.source_id == source
                        && dependencies
                            .iter()
                            .any(|record| record["id"].as_str() == Some(entry.target_id.as_str()))
                }) {
                    record[field] = Value::String(entry.target_id.clone());
                }
            }
        }
        let mut items = plan(
            dataset,
            &normalized,
            &local,
            context,
            self.0.singleton(dataset),
        )?;
        for (item, record) in items.iter_mut().zip(records) {
            item.note = self.0.note(dataset, record).or(item.note.take());
        }
        Ok(items)
    }
    fn apply_to_staging(
        &self,
        dataset: &str,
        records: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        let db = PluginDb::open_at(
            &target
                .root
                .join("data")
                .join(format!("{}.db", self.0.storage())),
            self.0.migrations(),
        )?;
        db.with_transaction(|conn| self.apply(conn, &[(dataset.into(), records.to_vec())], None))
            .map(|counts| counts.get(dataset).copied().unwrap_or(0))
    }
    fn apply_merge(
        &self,
        blocks: &[(String, Vec<Value>)],
        target: &MergeTarget<'_>,
    ) -> Result<BTreeMap<String, usize>, String> {
        self.database(target.app)?
            .with_transaction(|conn| self.apply(conn, blocks, Some(target)))
    }
}

fn remap_record(
    record: &mut Value,
    dataset: &str,
    reference: Option<(&str, &str)>,
    ids: &IdMap,
) -> Result<(), String> {
    let source_id = string(record, "id")?;
    record["id"] = Value::String(
        ids.get(&(dataset.into(), source_id.into()))
            .ok_or("导入记录缺少目标身份")?
            .clone(),
    );
    if let Some((field, dependency)) = reference {
        let source = string(record, field)?;
        if !source.is_empty() {
            record[field] = Value::String(
                ids.get(&(dependency.into(), source.into()))
                    .ok_or("关联记录未导入，请同时选择依赖数据集")?
                    .clone(),
            );
        }
    }
    Ok(())
}

/// 必填字符串字段，拒绝缺字段及错误类型。
pub(crate) fn string<'a>(record: &'a Value, field: &str) -> Result<&'a str, String> {
    record
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("导入记录字段 {field} 必须为字符串"))
}

/// 版本内只接受声明字段，拒绝静默丢弃未来字段或外来引用。
pub(crate) fn fields(record: &Value, allowed: &[&str]) -> Result<(), String> {
    let map = record.as_object().ok_or("导入记录必须是对象")?;
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("导入记录包含当前版本不支持的字段".into());
    }
    Ok(())
}

/// 显示名仅用于冲突候选，不自动授权覆盖。
pub(crate) fn label(record: &Value) -> &str {
    ["label", "name", "title", "id"]
        .iter()
        .find_map(|key| record.get(key).and_then(Value::as_str))
        .unwrap_or("")
}

/// 从 owner 的 JSON 查询构造逻辑记录，解析错误不能冒充空表。
pub(crate) fn query(conn: &Connection, sql: &str) -> Result<Vec<Value>, String> {
    let mut records = Vec::new();
    query_each(conn, sql, &mut |record| { records.push(record); Ok(()) })?;
    Ok(records)
}

/// 保持 SQLite 游标，只持有当前记录；消费方拒绝后不读取下一行。
pub(crate) fn query_each(
    conn: &Connection,
    sql: &str,
    emit: &mut dyn FnMut(Value) -> Result<(), String>,
) -> Result<(), String> {
    let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let record = serde_json::from_str(&row.map_err(|e| e.to_string())?)
            .map_err(|_| "本地逻辑记录解析失败".to_string())?;
        emit(record)?;
    }
    Ok(())
}

/// 逐条可选的普通数据目录；原文不额外标记危险内容。
pub(crate) fn descriptor(
    owner: &str,
    name: &str,
    title: &str,
    records: &[Value],
) -> Result<DatasetDescriptor, String> {
    let entries = records
        .iter()
        .map(|record| {
            Ok(CatalogEntry {
                dataset: name.into(),
                id: string(record, "id")?.into(),
                label: label(record).into(),
                detail: String::new(),
                dependencies: Vec::new(),
                note: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(DatasetDescriptor {
        name: name.into(),
        label: title.into(),
        owner: owner.into(),
        policy: TransportPolicy::Portable,
        schema_version: 1,
        selectable: true,
        contains_secret: false,
        default_selected: false,
        pulls: Vec::new(),
        note: None,
        record_count: entries.len(),
        entries,
    })
}

/// 按原始游标顺序交付选中项，保留重复项供统一校验；缺失身份不伪装成功。
pub(crate) fn visit_selected(
    ids: &[String],
    emit: &mut dyn FnMut(Value) -> Result<(), String>,
    read: impl FnOnce(&mut dyn FnMut(Value) -> Result<(), String>) -> Result<(), String>,
) -> Result<(), String> {
    let selected: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut missing = selected.clone();
    read(&mut |record| {
        let id = record.get("id").and_then(Value::as_str).unwrap_or_default();
        if selected.is_empty() || selected.contains(id) {
            missing.remove(id);
            emit(record)?;
        }
        Ok(())
    })?;
    if missing.is_empty() { Ok(()) }
    else { Err("所选记录已变化，请刷新后重新选择".into()) }
}

/// 验证所选身份仍存在；空身份集代表显式整类选择。
#[cfg(test)]
pub(crate) fn select(records: Vec<Value>, ids: &[String]) -> Result<Vec<Value>, String> {
    if ids.is_empty() {
        return Ok(records);
    }
    let selected: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut missing = selected.clone();
    for record in &records {
        if let Some(id) = record.get("id").and_then(Value::as_str) {
            missing.remove(id);
        }
    }
    if !missing.is_empty() {
        return Err("所选记录已变化，请刷新后重新选择".into());
    }
    Ok(records
        .into_iter()
        .filter(|record| {
            record
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| selected.contains(id))
        })
        .collect())
}

/// 统一来源映射、同名冲突及删除后恢复的判定。
pub(crate) fn plan(
    dataset: &str,
    records: &[Value],
    local: &[Value],
    context: &ImportContext<'_>,
    singleton: bool,
) -> Result<Vec<ImportPlanItem>, String> {
    plan_records(
        dataset,
        records,
        local,
        &context.source_space_id,
        context.merge.map(|view| view.core),
        singleton,
    )
}

fn plan_records(
    dataset: &str,
    records: &[Value],
    local: &[Value],
    source_space: &str,
    merge: Option<MergeContext<'_>>,
    singleton: bool,
) -> Result<Vec<ImportPlanItem>, String> {
    if records.is_empty() {
        return Ok(Vec::new());
    }
    // 索引仅借用本次快照；同名/同身份重复时保持原先 find 的首项优先规则。
    let mut by_id = BTreeMap::new();
    let mut by_label = BTreeMap::new();
    let mut mapped_targets = BTreeMap::new();
    // 单条预览只扫描一次即可，不为它建立整张本地表的索引。
    let indexed = records.len() > 1;
    if let Some(core) = merge.filter(|core| indexed && core.mode != ImportMode::Overwrite) {
        for entry in local {
            if let Some(id) = entry.get("id").and_then(Value::as_str) {
                by_id.entry(id).or_insert(entry);
            }
            by_label.entry(label(entry)).or_insert(entry);
        }
        for entry in &core.lineage.entries {
            if entry.source_space_id != source_space || entry.dataset != dataset {
                continue;
            }
            // 原规则：最后一条仍存在的映射；若全部目标已删除，保留首条用于恢复提示。
            let target = mapped_targets
                .entry(entry.source_id.as_str())
                .or_insert(entry.target_id.as_str());
            if by_id.contains_key(entry.target_id.as_str()) {
                *target = entry.target_id.as_str();
            }
        }
    }
    records
        .iter()
        .map(|record| {
            let id = string(record, "id")?;
            let mut item = ImportPlanItem::added(dataset, id, label(record));
            let Some(core) = merge else {
                return Ok(item);
            };
            if core.mode == ImportMode::Overwrite {
                return Ok(item);
            }
            let mapped = if indexed {
                mapped_targets.get(id).copied()
            } else {
                let mut mappings = core.lineage.entries.iter().filter(|entry| {
                    entry.source_space_id == source_space
                        && entry.dataset == dataset
                        && entry.source_id == id
                });
                mappings
                    .clone()
                    .rev()
                    .find(|entry| {
                        local.iter().any(|record| {
                            record.get("id").and_then(Value::as_str)
                                == Some(entry.target_id.as_str())
                        })
                    })
                    .or_else(|| mappings.next())
                    .map(|entry| entry.target_id.as_str())
            };
            let hit = if indexed {
                by_id.get(mapped.unwrap_or(id)).copied()
            } else {
                local.iter().find(|entry| {
                    entry.get("id").and_then(Value::as_str) == Some(mapped.unwrap_or(id))
                })
            }
            .or_else(|| {
                if mapped.is_none() && !singleton {
                    if indexed {
                        by_label.get(label(record)).copied()
                    } else {
                        local.iter().find(|entry| label(entry) == label(record))
                    }
                } else {
                    None
                }
            });
            let decision = core.decisions.get(&(dataset.into(), id.into()));
            if singleton && decision == Some(&ConflictDecision::KeepBoth) {
                return Err(format!("{dataset} 为单槽配置，不能保留两份"));
            }
            if let Some(hit) = hit {
                let target = string(hit, "id")?;
                item.target_id = Some(target.into());
                if same_record_except_id(record, hit) && (mapped.is_some() || target == id) {
                    item.decision = ItemDecision::Identical;
                } else {
                    item.conflict = true;
                    item.decision = match decision {
                        Some(ConflictDecision::UseImported) => ItemDecision::Replace,
                        Some(ConflictDecision::KeepBoth) => {
                            item.target_id = None;
                            ItemDecision::KeepBoth
                        }
                        _ => ItemDecision::Skip,
                    };
                }
            } else if mapped.is_some() {
                item.conflict = true;
                item.decision = match decision {
                    Some(ConflictDecision::UseImported) => ItemDecision::Insert,
                    Some(ConflictDecision::KeepBoth) => ItemDecision::KeepBoth,
                    _ => ItemDecision::RestorePrompt,
                };
                item.note = Some("原导入记录已被删除，默认不恢复".into());
            }
            Ok(item)
        })
        .collect()
}

/// 双方身份已验证；只忽略顶层 id，嵌套字段和缺失/null 差异仍按 JSON 原语义比较。
fn same_record_except_id(source: &Value, target: &Value) -> bool {
    source
        .as_object()
        .zip(target.as_object())
        .is_some_and(|(source, target)| {
            source.len() == target.len()
                && source
                    .iter()
                    .all(|(key, value)| key == "id" || target.get(key) == Some(value))
        })
}

/// 只有计划明确允许的记录才进入事务。
pub(crate) fn writable(target: &MergeTarget<'_>, dataset: &str, id: &str) -> bool {
    target.mode == ImportMode::Overwrite
        || matches!(
            target.decisions.get(&(dataset.into(), id.into())),
            Some(
                ItemDecision::Insert
                    | ItemDecision::PendingReference
                    | ItemDecision::Replace
                    | ItemDecision::KeepBoth
            )
        )
}

#[cfg(test)]
mod tests {
    use super::super::lineage::ImportMap;
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_stops_before_reading_later_broken_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE records(body TEXT); INSERT INTO records VALUES('{\"id\":\"a\"}'),('invalid json');").unwrap();
        let mut delivered = 0;
        let error = query_each(&conn, "SELECT body FROM records ORDER BY rowid", &mut |_| {
            delivered += 1;
            Err("已取消".into())
        }).unwrap_err();
        assert_eq!(error, "已取消");
        assert_eq!(delivered, 1);
    }

    #[test]
    fn streaming_selection_preserves_order_and_propagates_consumer_failure() {
        let input = vec![json!({"id":"b"}), json!({"id":"a"}), json!({"id":"b"})];
        let mut output = Vec::new();
        visit_selected(&["a".into(), "b".into()], &mut |item| { output.push(item); Ok(()) }, |emit| {
            for item in input.clone() { emit(item)?; }
            Ok(())
        }).unwrap();
        assert_eq!(output, input);
        let mut consumed = 0;
        let error = visit_selected(&[], &mut |_| Err("预算不足".into()), |emit| {
            for item in input { consumed += 1; emit(item)?; }
            Ok(())
        }).unwrap_err();
        assert_eq!(error, "预算不足");
        assert_eq!(consumed, 1);
    }

    #[test]
    fn copied_records_remap_references_and_reject_missing_targets() {
        let ids = BTreeMap::from([
            (
                ("database.history".into(), "history".into()),
                "history-copy".into(),
            ),
            (
                ("database.connections".into(), "connection".into()),
                "connection-copy".into(),
            ),
        ]);
        let source = json!({"id":"history","connectionId":"connection","sql":"SELECT 1"});
        let mut record = source.clone();
        remap_record(
            &mut record,
            "database.history",
            Some(("connectionId", "database.connections")),
            &ids,
        )
        .unwrap();
        assert_eq!(record["id"], "history-copy");
        assert_eq!(record["connectionId"], "connection-copy");
        assert_eq!(record["sql"], source["sql"]);
        let mut record = source;
        let mut missing = ids;
        missing.remove(&("database.connections".into(), "connection".into()));
        assert!(remap_record(
            &mut record,
            "database.history",
            Some(("connectionId", "database.connections")),
            &missing
        )
        .is_err());
    }

    #[test]
    fn same_name_requires_decision_and_singletons_reject_keep_both() {
        let lineage = ImportMap::default();
        let sources = BTreeMap::new();
        let decisions = BTreeMap::new();
        let core = MergeContext {
            mode: ImportMode::Merge,
            lineage: &lineage,
            decisions: &decisions,
            source_records: &sources,
        };
        let source = vec![json!({"id":"source","name":"同名","value":1})];
        let local = vec![json!({"id":"local","name":"同名","value":2})];
        let plan = plan_records("x.records", &source, &local, "origin", Some(core), false).unwrap();
        assert_eq!(plan[0].decision, ItemDecision::Skip);
        assert!(plan[0].conflict);
        let decisions = BTreeMap::from([(
            ("x.records".into(), "source".into()),
            ConflictDecision::KeepBoth,
        )]);
        let core = MergeContext {
            decisions: &decisions,
            ..core
        };
        assert_eq!(
            plan_records("x.records", &source, &local, "origin", Some(core), false).unwrap()[0]
                .decision,
            ItemDecision::KeepBoth
        );
        assert!(plan_records("x.records", &source, &local, "origin", Some(core), true).is_err());
    }

    #[test]
    fn mapped_copy_is_idempotent_and_deleted_target_is_not_revived() {
        let mut lineage = ImportMap::default();
        lineage.record("origin", "x.records", "source", "copy", "package", "now");
        let sources = BTreeMap::new();
        let decisions = BTreeMap::new();
        let core = MergeContext {
            mode: ImportMode::Merge,
            lineage: &lineage,
            decisions: &decisions,
            source_records: &sources,
        };
        let source = vec![json!({"id":"source","name":"记录"})];
        let local = vec![json!({"id":"copy","name":"记录"})];
        assert_eq!(
            plan_records("x.records", &source, &local, "origin", Some(core), false).unwrap()[0]
                .decision,
            ItemDecision::Identical
        );
        assert_eq!(
            plan_records("x.records", &source, &[], "origin", Some(core), false).unwrap()[0]
                .decision,
            ItemDecision::RestorePrompt
        );
    }

    #[test]
    fn selection_preserves_snapshot_order_and_duplicate_records() {
        let records = vec![
            json!({"id":"b","value":1}),
            json!({"id":"a"}),
            json!({"id":"b","value":2}),
            json!({"id":"c"}),
        ];
        assert_eq!(select(records.clone(), &[]).unwrap(), records);
        assert_eq!(
            select(records.clone(), &["b".into(), "a".into(), "b".into()]).unwrap(),
            records[..3]
        );
        assert!(select(records, &["b".into(), "missing".into()]).is_err());
    }

    #[test]
    fn indexed_conflicts_preserve_first_match_and_live_mapping_priority() {
        let mut lineage = ImportMap::default();
        for target in ["deleted-first", "copy-first", "copy-last", "deleted-last"] {
            lineage.record("origin", "x.records", "mapped", target, "package", "now");
        }
        lineage.record("other", "x.records", "mapped", "wrong", "package", "now");
        lineage.record("origin", "y.records", "mapped", "wrong", "package", "now");
        let sources = BTreeMap::new();
        let decisions = BTreeMap::new();
        let core = MergeContext {
            mode: ImportMode::Merge,
            lineage: &lineage,
            decisions: &decisions,
            source_records: &sources,
        };
        let source = vec![
            json!({"id":"same","name":"identity","value":1}),
            json!({"id":"new","name":"duplicate","value":1}),
            json!({"id":"mapped","name":"mapped","value":1}),
        ];
        let local = vec![
            json!({"id":"same","name":"identity","value":2}),
            source[0].clone(),
            json!({"id":"label-first","name":"duplicate","value":2}),
            json!({"id":"label-last","name":"duplicate","value":1}),
            json!({"id":"copy-first","name":"mapped","value":2}),
            json!({"id":"copy-last","name":"mapped","value":1}),
            json!({"id":"wrong","name":"mapped","value":2}),
        ];
        let plan = plan_records("x.records", &source, &local, "origin", Some(core), false).unwrap();
        assert_eq!(plan[0].decision, ItemDecision::Skip);
        assert_eq!(plan[0].target_id.as_deref(), Some("same"));
        assert_eq!(plan[1].decision, ItemDecision::Skip);
        assert_eq!(plan[1].target_id.as_deref(), Some("label-first"));
        assert_eq!(plan[2].decision, ItemDecision::Identical);
        assert_eq!(plan[2].target_id.as_deref(), Some("copy-last"));
        for (record, expected) in source.iter().zip(&plan) {
            let single = plan_records(
                "x.records",
                std::slice::from_ref(record),
                &local,
                "origin",
                Some(core),
                false,
            )
            .unwrap();
            assert_eq!(single[0].decision, expected.decision);
            assert_eq!(single[0].target_id, expected.target_id);
            assert_eq!(single[0].conflict, expected.conflict);
        }
        // 有映射但目标均已删除时，不退回同名匹配而意外覆盖另一条记录。
        let plan = plan_records("x.records", &source, &local[6..], "origin", Some(core), false)
            .unwrap();
        assert_eq!(plan[2].decision, ItemDecision::RestorePrompt);
        assert!(plan[2].target_id.is_none());
    }

    #[test]
    fn identity_comparison_preserves_nested_ids_missing_fields_and_nulls() {
        let source = json!({"id":"source","data":{"id":"nested","items":[null,1]},"empty":null});
        let mut normalized = source.clone();
        normalized["id"] = json!("target");
        assert!(same_record_except_id(&source, &normalized));
        let mut nested_changed = normalized.clone();
        nested_changed["data"]["id"] = json!("changed");
        let mut missing = normalized.clone();
        missing.as_object_mut().unwrap().remove("empty");
        let mut different_key = missing.clone();
        different_key["another"] = Value::Null;
        let mut extra = normalized.clone();
        extra["extra"] = Value::Null;
        for target in [nested_changed, missing, different_key, extra] {
            assert_eq!(same_record_except_id(&source, &target), normalized == target);
            assert!(!same_record_except_id(&source, &target));
        }
    }

    #[test]
    fn selection_rejects_missing_ids_and_unknown_fields() {
        assert!(select(vec![json!({"id":"a"})], &["missing".into()]).is_err());
        assert!(fields(&json!({"id":"a","unexpected":1}), &["id"]).is_err());
    }
}
