//! FRP 可移植元数据与逐文件原文；导入只写空间管理副本，不碰外部配置目录或启动进程。

use crate::framework::data_transfer::{
    adapter::{self, DatasetAdapter, MergeTarget, StagingTarget},
    records,
    types::*,
};
use crate::framework::{paths, store::PluginDb};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const META: &str = "frp.profiles";
const CONTENT: &str = "frp.contents";
const MANAGED: &str = "data/frp/imported";

struct FrpAdapter;

/// 当前空间的导入配置目录；本机外部 profileDir 不影响此路径。
pub(super) fn managed_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(paths::data_dir(app)?.join("frp/imported"))
}

/// 对已导入管理副本优先使用空间目录，其他文件沿用用户配置目录。
pub(super) fn directory_for(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    super::profile::validate_file_name(name)?;
    let managed = managed_dir(app)?;
    if managed.join(name).is_file() {
        Ok(managed)
    } else {
        super::profile_dir(app)
    }
}

fn discovered_id(name: &str) -> Result<String, String> {
    let space = crate::framework::space::current_id()?;
    Ok(hex::encode(Sha256::digest(
        format!("{space}\0{name}").as_bytes(),
    )))
}

fn available_id(conn: &rusqlite::Connection, name: &str) -> Result<String, String> {
    let base = discovered_id(name)?;
    for index in 0..1000 {
        let id = if index == 0 {
            base.clone()
        } else {
            hex::encode(Sha256::digest(format!("{base}:{index}").as_bytes()))
        };
        let occupied: Option<String> = conn
            .query_row(
                "SELECT file_name FROM profile_meta WHERE uid=?1",
                [&id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if occupied.as_deref().is_none_or(|file| file == name) {
            return Ok(id);
        }
    }
    Err("FRP 档案身份冲突，请检查元数据库".into())
}

/// 文件名跨两个可见目录唯一，避免管理副本遮蔽用户文件。
pub(super) fn ensure_name_available(app: &AppHandle, name: &str) -> Result<(), String> {
    super::profile::validate_file_name(name)?;
    for directory in [managed_dir(app)?, super::profile_dir(app)?] {
        if directory
            .join(name)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Err("档案名称已存在，请使用其他名称".into());
        }
    }
    Ok(())
}

/// 正常列表发现文件时登记身份；导出与导入预览本身不写元数据。
pub(super) fn remember(app: &AppHandle, name: &str) -> Result<String, String> {
    PluginDb::open(app,"frp",super::MIGRATIONS)?.with_conn(|conn|{
        let id = available_id(conn, name)?;
        conn.execute("INSERT INTO profile_meta(file_name,remark,last_used_at,uid) VALUES(?1,'',0,?2) ON CONFLICT(file_name) DO NOTHING",rusqlite::params![name,id]).map_err(|e|e.to_string())?;
        let source:String=conn.query_row("SELECT source_name FROM profile_meta WHERE file_name=?1",[name],|row|row.get(0)).map_err(|e|e.to_string())?;
        Ok(if source.is_empty(){name.to_string()}else{source})
    })
}

fn inventory(app: &AppHandle) -> Result<Vec<Value>, String> {
    let db = PluginDb::open(app, "frp", super::MIGRATIONS)?;
    let mut names = std::collections::BTreeSet::new();
    for directory in [super::profile_dir(app)?, managed_dir(app)?] {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("读取 FRP 配置目录失败: {e}")),
        };
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map_err(|e| e.to_string())?.is_file()
                && super::profile::validate_file_name(&name).is_ok()
            {
                names.insert(name);
            }
        }
    }
    db.with_conn(|conn| {
        names
            .into_iter()
            .map(|name| {
                let metadata: Option<(String, String, String)> = conn
                    .query_row(
                        "SELECT uid,remark,source_name FROM profile_meta WHERE file_name=?1",
                        [&name],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                let (id, remark, source) = match metadata {
                    Some(metadata) => metadata,
                    None => (available_id(conn, &name)?, String::new(), String::new()),
                };
                Ok(json!({"id":id,"name":if source.is_empty(){name}else{source},"remark":remark}))
            })
            .collect()
    })
}

fn include_contents(app: &AppHandle, items: &mut [Value]) -> Result<(), String> {
    let db = PluginDb::open(app, "frp", super::MIGRATIONS)?;
    let mut total = 0u64;
    for record in items {
        let name: Option<String> = db.with_conn(|conn| {
            conn.query_row(
                "SELECT file_name FROM profile_meta WHERE uid=?1",
                [records::string(record, "id")?],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
        })?;
        let name = name.as_deref().unwrap_or(records::string(record, "name")?);
        let path = directory_for(app, name)?.join(name);
        let metadata = path.symlink_metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("FRP 配置不是普通文件".into());
        }
        total = total.saturating_add(metadata.len());
        if metadata.len() > MAX_RECORD_BYTES as u64 || total > 32 * 1024 * 1024 {
            return Err("所选 FRP 配置超过数据包大小上限".into());
        }
        record["content"] = Value::String(
            std::fs::read_to_string(path).map_err(|e| format!("读取所选 FRP 配置失败: {e}"))?,
        );
    }
    Ok(())
}

fn local_contents(
    app: &AppHandle,
    items: &[Value],
    context: &ImportContext<'_>,
) -> Result<Vec<Value>, String> {
    let mut local = inventory(app)?;
    local.retain(|candidate| {
        items.iter().any(|item| {
            candidate["id"] == item["id"]
                || candidate["name"] == item["name"]
                || context.merge.is_some_and(|view| {
                    view.core.lineage.entries.iter().any(|entry| {
                        entry.source_space_id == context.source_space_id
                            && entry.dataset == CONTENT
                            && Some(entry.source_id.as_str()) == item["id"].as_str()
                            && Some(entry.target_id.as_str()) == candidate["id"].as_str()
                    })
                })
        })
    });
    include_contents(app, &mut local)?;
    Ok(local)
}

fn validate(dataset: &str, records: &[Value]) -> Result<(), String> {
    if !matches!(dataset, META | CONTENT) {
        return Err("未知 FRP 数据集".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for record in records {
        records::fields(
            record,
            if dataset == CONTENT {
                &["id", "name", "remark", "content"]
            } else {
                &["id", "name", "remark"]
            },
        )?;
        let id = records::string(record, "id")?;
        if id.is_empty() || !ids.insert(id) {
            return Err("FRP 身份为空或重复".into());
        }
        super::profile::validate_file_name(records::string(record, "name")?)?;
        records::string(record, "remark")?;
        if dataset == CONTENT {
            records::string(record, "content")?;
        }
    }
    Ok(())
}

fn apply(
    root: &Path,
    blocks: &[(String, Vec<Value>)],
    target: Option<&MergeTarget<'_>>,
) -> Result<BTreeMap<String, usize>, String> {
    let db = PluginDb::open_at(&root.join("data/frp.db"), super::MIGRATIONS)?;
    let directory = root.join(MANAGED);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut counts = BTreeMap::new();
    db.with_transaction(|conn| {
        if target.is_some_and(|target| target.mode == ImportMode::Overwrite) {
            clear_selected(conn, &directory, blocks)?;
        }
        for (dataset,items) in blocks {
            validate(dataset,items)?;
            let mut count=0;
            for record in items {
                let id=records::string(record,"id")?;
                if target.is_some_and(|target|!records::writable(target,dataset,id)) { continue; }
                let target_id=target.and_then(|target|target.id_map.get(&(META.into(),id.into())).or_else(||target.id_map.get(&(dataset.clone(),id.into())))).map(String::as_str).unwrap_or(id);
                // 包内身份不拼路径：固定摘要文件名，任何来源字符串都只能落到管理目录。
                let existing: Option<String> = conn.query_row("SELECT file_name FROM profile_meta WHERE uid=?1", [target_id], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
                let name = existing.filter(|name| super::profile::validate_file_name(name).is_ok() && directory.join(name).is_file()).unwrap_or_else(|| format!("import-{}.toml",hex::encode(Sha256::digest(target_id.as_bytes()))));
                conn.execute("INSERT INTO profile_meta(file_name,remark,last_used_at,uid,source_name) VALUES(?1,?2,0,?3,?4) ON CONFLICT(uid) DO UPDATE SET file_name=excluded.file_name,remark=excluded.remark,source_name=excluded.source_name",rusqlite::params![name,records::string(record,"remark")?,target_id,records::string(record,"name")?]).map_err(|e|e.to_string())?;
                if dataset==META && !directory.join(&name).exists() {
                    crate::framework::secure_store::replace_file(&directory.join(&name), "# 请补充导入的配置内容后再启动 frpc。\n".as_bytes())?;
                }
                if dataset==CONTENT {
                    let path=directory.join(&name);
                    if path.symlink_metadata().is_ok_and(|meta|meta.file_type().is_symlink()) { return Err("FRP 管理副本不能是符号链接".into()); }
                    crate::framework::secure_store::replace_file(&path,records::string(record,"content")?.as_bytes())?;
                }
                count+=1;
            }
            counts.insert(dataset.clone(),count);
        }
        Ok(())
    })?;
    Ok(counts)
}

fn clear_selected(
    conn: &rusqlite::Connection,
    directory: &Path,
    blocks: &[(String, Vec<Value>)],
) -> Result<(), String> {
    // 内容覆盖只清空间管理副本；外部用户文件永远不删除。
    if blocks.iter().any(|(dataset, _)| dataset == CONTENT) {
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "FRP 文件名无效")?;
            if super::profile::validate_file_name(&name).is_err() {
                continue;
            }
            std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM profile_meta WHERE file_name=?1", [&name])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM profile_client WHERE file_name=?1", [&name])
                .map_err(|e| e.to_string())?;
        }
    }
    // 仅元数据覆盖不能删除未选择的文件内容或改变本机客户端绑定。
    if blocks.iter().any(|(dataset, _)| dataset == META) {
        conn.execute("UPDATE profile_meta SET remark=''", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl DatasetAdapter for FrpAdapter {
    fn owner(&self) -> &'static str {
        "frp"
    }
    fn identity_dataset(&self, dataset: &str) -> String {
        if dataset == CONTENT {
            META.into()
        } else {
            dataset.into()
        }
    }
    fn storage_files(&self) -> Vec<String> {
        vec!["data/frp.db".into(), MANAGED.into()]
    }
    fn describe_datasets(&self, app: &AppHandle) -> Result<Vec<DatasetDescriptor>, String> {
        let records = inventory(app)?;
        let metadata = records::descriptor("frp", META, "FRP 档案元数据", &records)?;
        let mut contents = records::descriptor("frp", CONTENT, "FRP 配置文件内容", &records)?;
        contents.pulls.push(DatasetPull {
            kind: "frpProfile".into(),
            dataset: META.into(),
        });
        for entry in &mut contents.entries {
            entry.dependencies.push(DependencyEdge {
                kind: "frpProfile".into(),
                from_id: entry.id.clone(),
                to_id: entry.id.clone(),
            });
        }
        Ok(vec![metadata, contents])
    }
    fn export_records(
        &self,
        app: &AppHandle,
        dataset: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, String> {
        // 只读用户选中的文件原文，不递归读取外部目录。
        if dataset == META {
            return records::select(inventory(app)?, ids);
        }
        if dataset != CONTENT {
            return Err("未知 FRP 数据集".into());
        }
        let mut selected = records::select(inventory(app)?, ids)?;
        include_contents(app, &mut selected)?;
        Ok(selected)
    }
    fn validate_records(&self, dataset: &str, records: &[Value]) -> Result<(), String> {
        validate(dataset, records)
    }
    fn enumerate_references(
        &self,
        dataset: &str,
        records: &[Value],
    ) -> Result<Vec<DependencyEdge>, String> {
        if dataset != CONTENT {
            return Ok(Vec::new());
        }
        records
            .iter()
            .map(|record| {
                Ok(DependencyEdge {
                    kind: "frpProfile".into(),
                    from_id: records::string(record, "id")?.into(),
                    to_id: records::string(record, "id")?.into(),
                })
            })
            .collect()
    }
    fn plan_import(
        &self,
        dataset: &str,
        items: &[Value],
        context: &ImportContext<'_>,
    ) -> Result<Vec<ImportPlanItem>, String> {
        validate(dataset, items)?;
        if dataset == CONTENT
            && items
                .iter()
                .any(|item| !context.carries_id(META, records::string(item, "id").unwrap_or("")))
        {
            return Err("FRP 配置内容必须同时选择对应档案元数据".into());
        }
        let local = match context.merge {
            Some(view) => {
                if dataset == CONTENT {
                    local_contents(view.app, items, context)?
                } else {
                    inventory(view.app)?
                }
            }
            None => Vec::new(),
        };
        let mut planned = records::plan(dataset, items, &local, context, false)?;
        if dataset == META {
            if let Some(view) = context.merge {
                if let Some(contents) = view.core.source_records.get(CONTENT) {
                    let source: Vec<Value> = contents
                        .iter()
                        .filter(|record| {
                            context.carries_id(CONTENT, record["id"].as_str().unwrap_or(""))
                        })
                        .cloned()
                        .collect();
                    let content_plan = records::plan(
                        CONTENT,
                        &source,
                        &local_contents(view.app, &source, context)?,
                        context,
                        false,
                    )?;
                    for item in &mut planned {
                        if let Some(content) =
                            content_plan.iter().find(|content| content.id == item.id)
                        {
                            item.decision = content.decision;
                            item.target_id = content.target_id.clone();
                            item.conflict = false;
                        }
                    }
                }
            }
        }
        for item in &mut planned {
            item.note = Some(
                if dataset == META {
                    "仅导入元数据；配置内容未选择时需补充 TOML"
                } else {
                    "保存为空间管理副本，不启动 frpc；覆盖只清管理副本，保留外部文件；客户端需在本机选择"
                }
                .into(),
            );
        }
        Ok(planned)
    }
    fn apply_to_staging(
        &self,
        dataset: &str,
        items: &[Value],
        target: &StagingTarget,
    ) -> Result<usize, String> {
        apply(&target.root, &[(dataset.into(), items.to_vec())], None)
            .map(|counts| counts.get(dataset).copied().unwrap_or(0))
    }
    fn apply_merge(
        &self,
        blocks: &[(String, Vec<Value>)],
        target: &MergeTarget<'_>,
    ) -> Result<BTreeMap<String, usize>, String> {
        apply(target.space_root, blocks, Some(target))
    }
}

/// 装配 FRP 数据适配器。
pub(super) fn register() {
    static ADAPTER: FrpAdapter = FrpAdapter;
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_files_stay_managed_and_metadata_does_not_replace_content() {
        let root =
            std::env::temp_dir().join(format!("patchybox-frp-transfer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("data/frp/profiles")).unwrap();
        let original = root.join("data/frp/profiles/example.toml");
        std::fs::write(&original, "original").unwrap();
        let content = json!({"id":"source", "name":"example.toml", "remark":"示例", "content":"serverAddr = 'example.invalid'"});
        let metadata = json!({"id":"source", "name":"example.toml", "remark":"更新备注"});
        apply(&root, &[(CONTENT.into(), vec![content.clone()])], None).unwrap();
        apply(&root, &[(META.into(), vec![metadata])], None).unwrap();
        let files: Vec<_> = std::fs::read_dir(root.join(MANAGED))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&files[0]).unwrap(),
            content["content"].as_str().unwrap()
        );
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "original");
        let db = PluginDb::open_at(&root.join("data/frp.db"), super::super::MIGRATIONS).unwrap();
        db.with_transaction(|conn| {
            clear_selected(conn, &root.join(MANAGED), &[(CONTENT.into(), vec![])])
        })
        .unwrap();
        assert!(!files[0].exists());
        assert_eq!(std::fs::read_to_string(original).unwrap(), "original");
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_paths_and_duplicate_identities() {
        let record = json!({"id":"one", "name":"../outside.toml", "remark":""});
        assert!(validate(META, &[record]).is_err());
        let record = json!({"id":"one", "name":"safe.toml", "remark":""});
        assert!(validate(META, &[record.clone(), record]).is_err());
    }
}
