//! SSH 数据集传输 · tests

#[cfg(test)]
mod cases {

    use super::super::apply::apply_merge_blocks;
    use super::super::apply::apply_to_staging;
    use super::super::apply::count_rows;
    use super::super::apply::rewrite_record;
    use super::super::catalog::describe;
    use super::super::catalog::export;
    use super::super::catalog::records_index;
    use super::super::dataset_order;
    use super::super::owns;
    use super::super::plan::overwrite_item;
    use super::super::plan::plan_import;
    use super::super::plan::plan_merge_items;
    use super::super::records::references;
    use super::super::records::validate;
    use super::super::CREDENTIAL_DATASET;
    use super::super::DATASET_BOOKMARKS;
    use super::super::DATASET_GROUPS;
    use super::super::DATASET_PROFILES;
    use super::super::DATASET_TUNNELS;
    use super::super::EDGE_CREDENTIAL;
    use super::super::EDGE_GROUP;
    use super::super::EDGE_PROFILE;
    use super::super::NOTE_NO_CREDENTIAL;
    use crate::framework::data_transfer::adapter::StagingTarget;
    use crate::framework::data_transfer::types::ConflictDecision;
    use crate::framework::data_transfer::types::IdMap;
    use crate::framework::data_transfer::types::ImportContext;
    use crate::framework::data_transfer::types::ImportMode;
    use crate::framework::data_transfer::types::ItemDecision;
    use crate::framework::data_transfer::types::MergeContext;
    use crate::plugins::ssh::models::ServerProfile;
    use crate::plugins::ssh::models::SshBookmark;
    use crate::plugins::ssh::models::SshGroup;
    use crate::plugins::ssh::models::TunnelConfig;
    use crate::plugins::ssh::store;
    use rusqlite::Connection;
    use serde_json::Value;
    use std::collections::BTreeMap;

    use crate::framework::data_transfer::lineage;
    use crate::plugins::ssh::models::{AuthMethod, TunnelType};

    /// 造一条档案（默认未绑定凭证、未分组）
    fn profile(id: &str) -> ServerProfile {
        ServerProfile {
            id: id.into(),
            name: format!("档案 {id}"),
            host: "10.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth_method: AuthMethod::Password,
            credential_ref: None,
            has_local_auth: false,
            group_id: None,
            remark: None,
            last_connected_at: None,
        }
    }

    /// 造一条隧道
    fn tunnel(id: &str, profile_id: &str) -> TunnelConfig {
        TunnelConfig {
            id: id.into(),
            profile_id: profile_id.into(),
            name: format!("隧道 {id}"),
            tunnel_type: TunnelType::Local,
            listen_host: "127.0.0.1".into(),
            listen_port: 8080,
            target_host: Some("127.0.0.1".into()),
            target_port: Some(80),
            auto_start: false,
        }
    }

    /// 导入判定：引用的凭证随包带来 → 会写入；没带来 → 待补全并说明缺什么
    #[test]
    fn plan_import_marks_missing_references() {
        use crate::framework::data_transfer::types::{CarriedBlock, ItemDecision};
        use std::collections::{BTreeMap, BTreeSet};

        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        let record = serde_json::to_value(&bound).expect("序列化档案");

        let carried = BTreeMap::from([(
            CREDENTIAL_DATASET.to_string(),
            CarriedBlock {
                record_count: 1,
                ids: Some(BTreeSet::from(["cred-1".to_string()])),
            },
        )]);
        let with_credential = ImportContext {
            source_space_id: "default".into(),
            carried: carried.clone(),
            merge: None,
        };
        let items =
            plan_import(DATASET_PROFILES, &[record.clone()], &with_credential).expect("判定成功");
        assert_eq!(items[0].decision, ItemDecision::Insert);
        assert_eq!(items[0].id, "p1");

        // 包内只声明未携带（键存在但 ids 为空集）→ 待补全
        let declared_only = ImportContext {
            source_space_id: "default".into(),
            carried: BTreeMap::from([(
                CREDENTIAL_DATASET.to_string(),
                CarriedBlock {
                    record_count: 1,
                    ids: None,
                },
            )]),
            merge: None,
        };
        let items = plan_import(DATASET_PROFILES, &[record], &declared_only).expect("判定成功");
        assert_eq!(items[0].decision, ItemDecision::PendingReference);
        assert!(
            items[0].note.as_deref().unwrap_or("").contains("cred-1"),
            "说明里要点名缺了哪条凭证：{:?}",
            items[0].note
        );
    }

    /// 写入暂存库：建库、写入、读回计数一致，且 id 原样保留
    #[test]
    fn apply_to_staging_writes_readable_db() {
        let root = std::env::temp_dir().join(format!("pb-ssh-staging-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data")).expect("建暂存数据目录");
        let target = StagingTarget {
            root: root.clone(),
            space_id: "11111111-1111-4111-8111-111111111111".into(),
            keyring: crate::framework::secure_store::ScopedKeyringStore::new(
                "com.patchyx.covekit.tests",
            ),
        };

        let group = SshGroup {
            id: "g1".into(),
            name: "生产".into(),
            sort_order: 1,
        };
        let mut bound = profile("p1");
        bound.group_id = Some("g1".into());
        bound.credential_ref = Some("cred-1".into());
        let tunnel = tunnel("t1", "p1");
        let bookmark = SshBookmark {
            id: "b1".into(),
            profile_id: "p1".into(),
            name: "日志".into(),
            path: "/var/log".into(),
            sort: 1,
        };

        let written = |dataset: &str, value: &Value| -> usize {
            apply_to_staging(dataset, std::slice::from_ref(value), &target).expect("写入成功")
        };
        assert_eq!(
            written(
                DATASET_GROUPS,
                &serde_json::to_value(&group).expect("序列化分组")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_PROFILES,
                &serde_json::to_value(&bound).expect("序列化档案")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_TUNNELS,
                &serde_json::to_value(&tunnel).expect("序列化隧道")
            ),
            1
        );
        assert_eq!(
            written(
                DATASET_BOOKMARKS,
                &serde_json::to_value(&bookmark).expect("序列化书签")
            ),
            1
        );

        // 读回：条数一致、id 原样保留（导入必须保留传输标识）
        let conn = Connection::open(root.join("data").join("ssh.db")).expect("打开暂存库");
        let profiles = store::profiles::list_profiles(&conn).expect("读档案");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "p1");
        assert_eq!(profiles[0].credential_ref.as_deref(), Some("cred-1"));
        assert_eq!(
            store::profiles::list_groups(&conn).expect("读分组").len(),
            1
        );
        assert_eq!(
            store::tunnels::list_tunnels(&conn, "p1").expect("读隧道")[0].id,
            "t1"
        );
        let bookmarks = store::bookmarks::list_bookmarks(&conn, "p1").expect("读书签");
        assert_eq!(bookmarks[0].id, "b1");
        assert_eq!(bookmarks[0].path, "/var/log");
        assert_eq!(
            count_rows(&root.join("data").join("ssh.db"), DATASET_BOOKMARKS).expect("计数"),
            1
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 目录：档案可勾选，分组/隧道/书签不可勾选且条目数与记录数一致
    #[test]
    fn describe_marks_selectable_dataset_and_follows() {
        let conn = store::open_memory();
        store::profiles::upsert_group(
            &conn,
            &SshGroup {
                id: "g1".into(),
                name: "生产".into(),
                sort_order: 1,
            },
        )
        .expect("插入分组");
        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        bound.group_id = Some("g1".into());
        store::profiles::upsert_profile(&conn, &bound, 1).expect("插入档案");
        store::profiles::upsert_profile(&conn, &profile("p2"), 2).expect("插入档案");
        store::tunnels::upsert_tunnel(&conn, &tunnel("t1", "p1"), 3).expect("插入隧道");
        store::bookmarks::add_bookmark(&conn, "p1", "日志", "/var/log").expect("插入书签");

        let descriptors = describe(&conn).expect("目录生成");
        let by_name = |name: &str| {
            descriptors
                .iter()
                .find(|item| item.name == name)
                .expect("数据集存在")
        };
        let profiles = by_name(DATASET_PROFILES);
        assert!(profiles.selectable);
        assert_eq!(profiles.record_count, 2);
        let entry = profiles
            .entries
            .iter()
            .find(|item| item.id == "p1")
            .expect("档案条目存在");
        assert_eq!(entry.detail, "root@10.0.0.1:22");
        assert!(entry.note.is_none());
        let kinds: Vec<&str> = entry
            .dependencies
            .iter()
            .map(|item| item.kind.as_str())
            .collect();
        assert_eq!(kinds, vec!["credential", "group", "tunnel", "bookmark"]);
        assert_eq!(entry.dependencies[0].from_id, "p1");
        assert_eq!(entry.dependencies[0].to_id, "cred-1");

        // 未绑定凭证的档案在预览里标「待补全」
        let plain = profiles
            .entries
            .iter()
            .find(|item| item.id == "p2")
            .expect("档案条目存在");
        assert_eq!(plain.note.as_deref(), Some(NOTE_NO_CREDENTIAL));
        assert!(plain.dependencies.is_empty());

        assert!(!by_name(DATASET_GROUPS).selectable);
        assert_eq!(by_name(DATASET_GROUPS).record_count, 1);
        assert_eq!(by_name(DATASET_TUNNELS).record_count, 1);
        assert_eq!(by_name(DATASET_BOOKMARKS).record_count, 1);
        // 隧道条目的 detail 是监听地址，书签是远程路径
        assert_eq!(by_name(DATASET_TUNNELS).entries[0].detail, "127.0.0.1:8080");
        assert_eq!(by_name(DATASET_BOOKMARKS).entries[0].detail, "/var/log");
    }

    /// 导出按 id 顺序取记录；缺 id 报错而不是少带
    #[test]
    fn export_follows_id_order_and_rejects_missing() {
        let conn = store::open_memory();
        store::profiles::upsert_profile(&conn, &profile("p1"), 1).expect("插入档案");
        store::profiles::upsert_profile(&conn, &profile("p2"), 2).expect("插入档案");
        store::tunnels::upsert_tunnel(&conn, &tunnel("t1", "p1"), 3).expect("插入隧道");

        let records =
            export(&conn, DATASET_PROFILES, &["p2".into(), "p1".into()]).expect("导出成功");
        assert_eq!(records[0]["id"], serde_json::json!("p2"));
        assert_eq!(records[0]["name"], serde_json::json!("档案 p2"));
        assert_eq!(records[1]["id"], serde_json::json!("p1"));

        // 子记录表跨档案按 id 取
        let tunnels = export(&conn, DATASET_TUNNELS, &["t1".into()]).expect("导出成功");
        assert_eq!(tunnels[0]["profileId"], serde_json::json!("p1"));
        assert_eq!(tunnels[0]["listenPort"], serde_json::json!(8080));

        let missing = export(&conn, DATASET_PROFILES, &["nope".into()]).unwrap_err();
        assert!(missing.contains("不存在记录 nope"), "{missing}");
        // 未绑定凭证的档案可选字段不落空字段（序列化即契约，与前端一致）
        assert!(records[0].get("credentialRef").is_none());
    }

    /// 记录体检：缺主机 / 端口 0 / 缺 id 都被拒，合法记录通过
    #[test]
    fn validate_rejects_incomplete_records() {
        let good = serde_json::to_value(profile("p1")).expect("序列化");
        assert!(validate(DATASET_PROFILES, &[good.clone()]).is_ok());

        let mut no_host = good.clone();
        no_host["host"] = serde_json::json!("  ");
        assert!(validate(DATASET_PROFILES, &[no_host])
            .unwrap_err()
            .contains("缺少主机地址"));

        let mut zero_port = good.clone();
        zero_port["port"] = serde_json::json!(0);
        assert!(validate(DATASET_PROFILES, &[zero_port])
            .unwrap_err()
            .contains("端口非法"));

        let mut no_id = good.clone();
        no_id["id"] = serde_json::json!("");
        assert!(validate(DATASET_PROFILES, &[no_id])
            .unwrap_err()
            .contains("缺少 id"));

        // 结构不认识（认证方式写错）直接报错，不做兜底
        let mut bad_auth = good;
        bad_auth["authMethod"] = serde_json::json!("kerberos");
        assert!(validate(DATASET_PROFILES, &[bad_auth])
            .unwrap_err()
            .contains("结构不认识"));

        let mut bad_bookmark = serde_json::to_value(SshBookmark {
            id: "b1".into(),
            profile_id: "p1".into(),
            name: "日志".into(),
            path: "  ".into(),
            sort: 0,
        })
        .expect("序列化");
        bad_bookmark["path"] = serde_json::json!("");
        assert!(validate(DATASET_BOOKMARKS, &[bad_bookmark])
            .unwrap_err()
            .contains("缺少远程路径"));

        assert!(validate("ssh.known_hosts", &[]).is_err());
    }

    /// 清单依赖边：档案出凭证/分组边，隧道与书签出所属档案边
    #[test]
    fn references_expose_record_level_edges() {
        let mut bound = profile("p1");
        bound.credential_ref = Some("cred-1".into());
        bound.group_id = Some("g1".into());
        let records = vec![serde_json::to_value(&bound).expect("序列化")];
        let edges = references(DATASET_PROFILES, &records).expect("边生成");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].kind, EDGE_CREDENTIAL);
        assert_eq!(edges[0].from_id, "p1");
        assert_eq!(edges[1].kind, EDGE_GROUP);
        assert_eq!(edges[1].to_id, "g1");

        let tunnel_records = vec![serde_json::to_value(tunnel("t1", "p1")).expect("序列化")];
        let tunnel_edges = references(DATASET_TUNNELS, &tunnel_records).expect("边生成");
        assert_eq!(tunnel_edges.len(), 1);
        assert_eq!(tunnel_edges[0].kind, EDGE_PROFILE);
        assert_eq!(tunnel_edges[0].from_id, "t1");
        assert_eq!(tunnel_edges[0].to_id, "p1");

        assert!(references(DATASET_GROUPS, &[]).expect("边生成").is_empty());
    }

    /// 归属判断与落库顺序：分组在档案前、子记录在档案后
    #[test]
    fn owns_and_order_cover_four_datasets() {
        assert!(owns(DATASET_TUNNELS));
        assert!(!owns("ssh.known_hosts"));
        assert_eq!(
            dataset_order().to_vec(),
            vec![
                DATASET_GROUPS,
                DATASET_PROFILES,
                DATASET_TUNNELS,
                DATASET_BOOKMARKS
            ]
        );
    }

    /* ── 合并导入判定（L3）── */

    /// 把档案写进内存库并读回逻辑记录索引（合并判定的「当前空间现状」）
    fn live_index(profiles: &[ServerProfile]) -> BTreeMap<String, Value> {
        let conn = store::open_memory();
        for profile in profiles {
            store::profiles::upsert_profile(&conn, profile, 1_700_000_000_000).expect("写入内存库");
        }
        records_index(&conn, DATASET_PROFILES).expect("读回逻辑记录")
    }

    /// 合并判定上下文夹具
    fn merge_core<'a>(
        lineage: &'a lineage::ImportMap,
        decisions: &'a BTreeMap<(String, String), ConflictDecision>,
        source_records: &'a BTreeMap<String, Vec<Value>>,
    ) -> MergeContext<'a> {
        MergeContext {
            mode: ImportMode::Merge,
            lineage,
            decisions,
            source_records,
        }
    }

    /// 空上下文（合并判定不带包内携带信息时用）
    fn bare_context() -> ImportContext<'static> {
        ImportContext::default()
    }

    /// 全新记录 → 插入
    #[test]
    fn merge_insert_when_no_hit() {
        let lineage = lineage::ImportMap::default();
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let core = merge_core(&lineage, &decisions, &sources);
        let live = live_index(&[profile("p-local")]);
        let record = serde_json::to_value(profile("p-new")).expect("序列化");
        let items = plan_merge_items(
            DATASET_PROFILES,
            &[record],
            &bare_context(),
            &core,
            &live,
            &BTreeMap::from([("档案 p-local".to_string(), "p-local".to_string())]),
        )
        .expect("判定");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].decision, ItemDecision::Insert);
        assert_eq!(items[0].target_id, None);
    }

    /// 主键命中且内容一致 → 已识别跳过
    #[test]
    fn merge_identical_by_id() {
        let lineage = lineage::ImportMap::default();
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let core = merge_core(&lineage, &decisions, &sources);
        let live = live_index(&[profile("p1")]);
        let record = serde_json::to_value(profile("p1")).expect("序列化");
        let items = plan_merge_items(
            DATASET_PROFILES,
            &[record],
            &bare_context(),
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Identical);
        assert_eq!(items[0].target_id.as_deref(), Some("p1"));
    }

    /// 凭证引用不参与同一性：包内档案带 credentialRef、本地为空仍算一致
    #[test]
    fn merge_identity_ignores_credential_ref() {
        let lineage = lineage::ImportMap::default();
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let core = merge_core(&lineage, &decisions, &sources);
        let live = live_index(&[profile("p1")]);
        let mut packaged = profile("p1");
        packaged.credential_ref = Some("cred-x".into());
        // 携带信息里带上该凭证，避免「待补录」噪声干扰断言
        let mut context = bare_context();
        context.carried.insert(
            CREDENTIAL_DATASET.to_string(),
            crate::framework::data_transfer::types::CarriedBlock {
                record_count: 1,
                ids: Some(["cred-x".to_string()].into_iter().collect()),
            },
        );
        let record = serde_json::to_value(packaged).expect("序列化");
        let items = plan_merge_items(
            DATASET_PROFILES,
            &[record],
            &context,
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Identical);
    }

    /// 主键命中但内容不同 → 默认保留本地；决策采用导入 → 替换且落在本地 id 上
    #[test]
    fn merge_conflict_default_keep_local_and_use_imported() {
        let lineage = lineage::ImportMap::default();
        let mut sources = BTreeMap::new();
        let mut local = profile("p1");
        local.host = "10.0.0.9".into();
        let live = live_index(&[local]);
        let mut packaged = serde_json::to_value(profile("p1")).expect("序列化");
        sources.insert(DATASET_PROFILES.to_string(), vec![packaged.clone()]);

        // 无决策 → 保留本地
        let decisions = BTreeMap::new();
        let core = merge_core(&lineage, &decisions, &sources);
        let items = plan_merge_items(
            DATASET_PROFILES,
            std::slice::from_ref(&packaged),
            &bare_context(),
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Skip);
        assert_eq!(items[0].target_id.as_deref(), Some("p1"));

        // 决策 = 采用导入 → 替换本地记录
        let decisions = BTreeMap::from([(
            (DATASET_PROFILES.to_string(), "p1".to_string()),
            ConflictDecision::UseImported,
        )]);
        let core = merge_core(&lineage, &decisions, &sources);
        let items = plan_merge_items(
            DATASET_PROFILES,
            std::slice::from_ref(&packaged),
            &bare_context(),
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Replace);
        assert_eq!(items[0].target_id.as_deref(), Some("p1"));

        // 决策 = 保留两份 → 目标 id 留空（由框架新分配）
        let decisions = BTreeMap::from([(
            (DATASET_PROFILES.to_string(), "p1".to_string()),
            ConflictDecision::KeepBoth,
        )]);
        let core = merge_core(&lineage, &decisions, &sources);
        packaged
            .as_object_mut()
            .expect("对象")
            .remove("credentialRef");
        let items = plan_merge_items(
            DATASET_PROFILES,
            std::slice::from_ref(&packaged),
            &bare_context(),
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::KeepBoth);
        assert_eq!(items[0].target_id, None);
    }

    /// 业务键命中（同名不同 id）→ 冲突
    #[test]
    fn merge_conflict_by_business_key() {
        let lineage = lineage::ImportMap::default();
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let core = merge_core(&lineage, &decisions, &sources);
        // 本地有「档案 p2」（id 是 p-local）；包里的 p2 同名不同 id
        let live = live_index(&[ServerProfile {
            id: "p-local".into(),
            ..profile("p2")
        }]);
        let by_key = BTreeMap::from([("档案 p2".to_string(), "p-local".to_string())]);
        let record = serde_json::to_value(profile("p2")).expect("序列化");
        let items = plan_merge_items(
            DATASET_PROFILES,
            &[record],
            &bare_context(),
            &core,
            &live,
            &by_key,
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Skip);
        assert_eq!(items[0].target_id.as_deref(), Some("p-local"));
        assert!(items[0].note.as_deref().unwrap_or("").contains("同名"));
    }

    /// 映射命中且一致 → 已识别；映射目标已删 → 复活确认
    #[test]
    fn merge_lineage_hit_and_restore_prompt() {
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let lineage = lineage::ImportMap {
            version: 1,
            entries: vec![lineage::LineageEntry {
                source_space_id: "source-a".into(),
                dataset: DATASET_PROFILES.into(),
                source_id: "p1".into(),
                target_id: "p1".into(),
                package_ids: vec![],
                last_seen_at: String::new(),
            }],
        };
        let core = merge_core(&lineage, &decisions, &sources);
        let mut context = bare_context();
        context.source_space_id = "source-a".into();

        // 目标在本地且一致 → 已识别
        let live = live_index(&[profile("p1")]);
        let record = serde_json::to_value(profile("p1")).expect("序列化");
        let items = plan_merge_items(
            DATASET_PROFILES,
            std::slice::from_ref(&record),
            &context,
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::Identical);

        // 目标已不在本地 → 默认不复活
        let live = BTreeMap::new();
        let items = plan_merge_items(
            DATASET_PROFILES,
            std::slice::from_ref(&record),
            &context,
            &core,
            &live,
            &BTreeMap::new(),
        )
        .expect("判定");
        assert_eq!(items[0].decision, ItemDecision::RestorePrompt);
        assert!(items[0].note.as_deref().unwrap_or("").contains("不复活"));
    }

    /// 覆盖模式：不做逐条冲突判定，全部按「新增写入」（提交时先清空）
    #[test]
    fn overwrite_marks_everything_insert() {
        let lineage = lineage::ImportMap::default();
        let decisions = BTreeMap::new();
        let sources = BTreeMap::new();
        let core = MergeContext {
            mode: ImportMode::Overwrite,
            ..merge_core(&lineage, &decisions, &sources)
        };
        // 覆盖模式在 plan_import_live 里短路，这里直接验证 overwrite_item 的判定
        let record = serde_json::to_value(profile("p1")).expect("序列化");
        let item = overwrite_item(DATASET_PROFILES, &record, &bare_context()).expect("判定");
        assert_eq!(item.decision, ItemDecision::Insert);
        assert_eq!(core.mode, ImportMode::Overwrite);
    }

    /* ── 合并/覆盖写入（L3）── */

    /// 单条写入的决策与 id 映射夹具
    fn write_fixture(
        entries: &[(&str, ItemDecision)],
    ) -> (BTreeMap<(String, String), ItemDecision>, IdMap) {
        let decisions = entries
            .iter()
            .map(|(id, decision)| ((DATASET_PROFILES.to_string(), (*id).to_string()), *decision))
            .collect();
        let id_map = entries
            .iter()
            .map(|(id, _)| {
                (
                    (DATASET_PROFILES.to_string(), (*id).to_string()),
                    (*id).to_string(),
                )
            })
            .collect();
        (decisions, id_map)
    }

    /// 故障注入：中途坏记录 → 事务整体回滚，当前空间不留半拉数据
    #[test]
    fn merge_write_rolls_back_on_failure() {
        let mut conn = store::open_memory();
        store::profiles::upsert_profile(&conn, &profile("p-local"), 1).expect("预置本地数据");
        let good = serde_json::to_value(profile("p-new")).expect("序列化");
        // 坏记录：缺字段，反序列化必然失败
        let bad = serde_json::json!({ "id": "p-bad" });
        let (decisions, id_map) = write_fixture(&[
            ("p-new", ItemDecision::Insert),
            ("p-bad", ItemDecision::Insert),
        ]);
        let blocks = vec![(DATASET_PROFILES.to_string(), vec![good, bad])];

        let tx = conn.transaction().expect("开事务");
        let result = apply_merge_blocks(&tx, &blocks, ImportMode::Merge, &id_map, &decisions);
        assert!(result.is_err(), "坏记录必须报错");
        drop(tx); // 不提交即回滚

        let index = records_index(&conn, DATASET_PROFILES).expect("读回");
        assert!(index.contains_key("p-local"), "本地数据不能被动");
        assert!(!index.contains_key("p-new"), "失败前的写入必须回滚");
    }

    /// 覆盖模式：先清空本地再整体写入
    #[test]
    fn overwrite_clears_then_writes() {
        let conn = store::open_memory();
        store::profiles::upsert_profile(&conn, &profile("p-local"), 1).expect("预置本地数据");
        let record = serde_json::to_value(profile("p-new")).expect("序列化");
        let (_, id_map) = write_fixture(&[("p-new", ItemDecision::Insert)]);
        let blocks = vec![(DATASET_PROFILES.to_string(), vec![record])];
        let counts = apply_merge_blocks(
            &conn,
            &blocks,
            ImportMode::Overwrite,
            &id_map,
            &BTreeMap::new(),
        )
        .expect("覆盖写入");
        assert_eq!(counts.get(DATASET_PROFILES), Some(&1));
        let index = records_index(&conn, DATASET_PROFILES).expect("读回");
        assert!(index.contains_key("p-new"));
        assert!(!index.contains_key("p-local"), "覆盖要先清空本地数据");
    }

    /// 跳过类决策不写入（Identical/Skip/RestorePrompt 一律不动）
    #[test]
    fn merge_skips_non_write_decisions() {
        let conn = store::open_memory();
        let records: Vec<Value> = ["a", "b", "c"]
            .iter()
            .map(|id| serde_json::to_value(profile(id)).expect("序列化"))
            .collect();
        let (decisions, id_map) = write_fixture(&[
            ("a", ItemDecision::Identical),
            ("b", ItemDecision::Skip),
            ("c", ItemDecision::RestorePrompt),
        ]);
        let blocks = vec![(DATASET_PROFILES.to_string(), records)];
        let counts = apply_merge_blocks(&conn, &blocks, ImportMode::Merge, &id_map, &decisions)
            .expect("写入");
        assert_eq!(counts.get(DATASET_PROFILES), Some(&0), "跳过类决策零写入");
        assert!(records_index(&conn, DATASET_PROFILES)
            .expect("读回")
            .is_empty());
    }

    /// 引用改写：分组映射、凭证置空、隧道档案映射；档案未映射则报错不留悬空引用
    #[test]
    fn rewrite_remaps_references_and_clears_credential() {
        let mut packaged = profile("p1");
        packaged.credential_ref = Some("cred-1".into());
        packaged.group_id = Some("g1".into());
        let record = serde_json::to_value(packaged).expect("序列化");
        let id_map: IdMap = BTreeMap::from([(
            (DATASET_GROUPS.to_string(), "g1".to_string()),
            "g-local".to_string(),
        )]);
        let out = rewrite_record(DATASET_PROFILES, &record, "p-new", &id_map).expect("改写");
        assert_eq!(out["id"], serde_json::json!("p-new"));
        assert_eq!(
            out["credentialRef"],
            serde_json::Value::Null,
            "凭证引用必须置空"
        );
        assert_eq!(out["groupId"], serde_json::json!("g-local"));

        let tunnel = serde_json::to_value(tunnel("t1", "p1")).expect("序列化");
        let id_map: IdMap = BTreeMap::from([(
            (DATASET_PROFILES.to_string(), "p1".to_string()),
            "p-new".to_string(),
        )]);
        let out = rewrite_record(DATASET_TUNNELS, &tunnel, "t-new", &id_map).expect("改写");
        assert_eq!(out["profileId"], serde_json::json!("p-new"));

        let error = rewrite_record(DATASET_TUNNELS, &tunnel, "t-new", &BTreeMap::new())
            .expect_err("档案未映射必须报错");
        assert!(error.contains("没有目标 id"), "错误文案: {error}");
    }
}
