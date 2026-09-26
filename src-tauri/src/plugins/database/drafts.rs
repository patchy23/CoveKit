//! SQL 草稿只保存文档和目标，不保存结果、密码或活动事务。
use super::{
    models::{QueryDraft, QueryDraftPosition},
    store::{self, StoreState},
};
use tauri::State;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashSet;

fn validate(drafts: &[QueryDraft]) -> Result<(), String> {
    if drafts.len() > 50 {
        return Err("最多恢复 50 个 SQL 草稿".into());
    }
    for draft in drafts {
        if draft.sql.len() > 1024 * 1024 || draft.label.len() > 1024 {
            return Err("单个 SQL 草稿不能超过 1 MiB，名称不能超过 1024 字节".into());
        }
    }
    Ok(())
}

fn check_bytes(bytes: usize) -> Result<(), String> {
    if bytes > 8 * 1024 * 1024 {
        return Err("SQL 草稿总量超过 8 MiB，请另存 SQL 文件后关闭部分页签".into());
    }
    Ok(())
}

fn encode(drafts: &[QueryDraft]) -> Result<String, String> {
    validate(drafts)?;
    let value = serde_json::to_string(drafts).map_err(|e| e.to_string())?;
    check_bytes(value.len())?;
    Ok(value)
}

fn load(conn: &Connection) -> Result<Vec<QueryDraft>, String> {
    let mut statement = conn
        .prepare("SELECT e.id,e.content,p.selection_from,p.selection_to,p.active
                  FROM query_draft_entries e LEFT JOIN query_draft_positions p ON p.id=e.id
                  ORDER BY e.position")
        .map_err(|e| e.to_string())?;
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    let mut drafts = Vec::new();
    let mut bytes = 2usize;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let content: String = row.get(1).map_err(|e| e.to_string())?;
        let position_from: Option<usize> = row.get(2).map_err(|e| e.to_string())?;
        let position = position_from.map(|from| {
            Ok::<_, rusqlite::Error>((from, row.get::<_, usize>(3)?, row.get::<_, bool>(4)?))
        }).transpose().map_err(|e| e.to_string())?;
        let content_bytes = match position {
            Some((from, to, active)) => (content.len() + position_bytes(from, to, active)).saturating_sub(7),
            None => content.len(),
        };
        bytes = bytes.saturating_add(content_bytes + usize::from(!drafts.is_empty()));
        check_bytes(bytes)?;
        let mut draft: QueryDraft = serde_json::from_str(&content)
            .map_err(|e| format!("草稿损坏，原记录已保留：{e}"))?;
        if let Some((from, to, active)) = position {
            draft.from = from;
            draft.to = to;
            draft.active = active;
        }
        draft.id = Some(row.get(0).map_err(|e| e.to_string())?);
        drafts.push(draft);
        validate(&drafts)?;
    }
    if !drafts.is_empty() {
        return Ok(drafts);
    }
    let content: Option<String> = conn
        .query_row("SELECT content FROM query_drafts WHERE id=1", [], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    let drafts = match content {
        Some(value) => serde_json::from_str(&value)
            .map_err(|e| format!("草稿损坏，原记录已保留：{e}"))?,
        None => Vec::new(),
    };
    encode(&drafts)?;
    Ok(drafts)
}

/// 调用方持有事务：正文只写变更文档，顺序与关闭文档在同一事务内提交。
fn save(conn: &Connection, mut drafts: Vec<QueryDraft>, order: Option<Vec<String>>, positions: Vec<QueryDraftPosition>) -> Result<(), String> {
    validate(&drafts)?;
    let order = match order {
        Some(order) => order,
        None => drafts.iter_mut().map(|draft| {
            draft.id.get_or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone()
        }).collect(),
    };
    let ids: HashSet<&str> = order.iter().map(String::as_str).collect();
    if order.len() > 50 || ids.len() != order.len() || ids.contains("") {
        return Err("SQL 草稿顺序包含重复、空标识或超过 50 个文档".into());
    }
    let existing = conn.prepare("SELECT id FROM query_draft_entries")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut changed = HashSet::new();
    for draft in &drafts {
        let id = draft.id.as_deref().ok_or("增量草稿缺少文档标识")?;
        if !ids.contains(id) || !changed.insert(id.to_string()) {
            return Err("增量草稿标识重复或不在当前文档中".into());
        }
    }
    if order.iter().any(|id| !existing.contains(id) && !changed.contains(id)) {
        return Err("SQL 草稿增量缺少文档正文，原记录已保留".into());
    }
    let mut positioned = HashSet::new();
    for position in &positions {
        if !ids.contains(position.id.as_str()) || changed.contains(&position.id)
            || !positioned.insert(position.id.as_str()) {
            return Err("SQL 草稿选区标识重复、与正文重复或不在当前文档中".into());
        }
    }
    for mut draft in drafts.drain(..) {
        let id = draft.id.take().ok_or("增量草稿缺少文档标识")?;
        let position = QueryDraftPosition { id, from: draft.from, to: draft.to, active: draft.active };
        // 正文内固定占位值共 7 字节；实际选区单独保存，预算仍按还原后的 JSON 计算。
        draft.from = 0;
        draft.to = 0;
        draft.active = false;
        // 标识独立存放，不挤占原 SQL JSON 预算，也不重复存进正文。
        let content = serde_json::to_string(&draft).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO query_draft_entries(id,position,content) VALUES(?1,0,?2)
             ON CONFLICT(id) DO UPDATE SET content=excluded.content WHERE content<>excluded.content",
            rusqlite::params![position.id, content],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO query_draft_positions(id,selection_from,selection_to,active,base_bytes) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET selection_from=excluded.selection_from,selection_to=excluded.selection_to,active=excluded.active,base_bytes=excluded.base_bytes",
            rusqlite::params![position.id, position.from, position.to, position.active, content.len()],
        ).map_err(|e| e.to_string())?;
    }
    for position in positions {
        let updated = conn.execute(
            "UPDATE query_draft_positions SET selection_from=?2,selection_to=?3,active=?4 WHERE id=?1",
            rusqlite::params![position.id, position.from, position.to, position.active],
        ).map_err(|e| e.to_string())?;
        if updated != 1 {
            return Err("SQL 草稿选区缺少完整保存基线，原记录已保留".into());
        }
    }
    for id in existing {
        if !ids.contains(id.as_str()) {
            conn.execute("DELETE FROM query_draft_positions WHERE id=?1", [&id])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM query_draft_entries WHERE id=?1", [&id])
                .map_err(|e| e.to_string())?;
        }
    }
    for (position, id) in order.iter().enumerate() {
        conn.execute("UPDATE query_draft_entries SET position=?2 WHERE id=?1 AND position<>?2", rusqlite::params![id, position])
            .map_err(|e| e.to_string())?;
    }
    let bytes: usize = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN p.id IS NULL THEN length(CAST(e.content AS BLOB))
            ELSE p.base_bytes + length(CAST(p.selection_from AS TEXT)) +
            length(CAST(p.selection_to AS TEXT)) + CASE WHEN p.active THEN 4 ELSE 5 END - 7 END),0)
         FROM query_draft_entries e LEFT JOIN query_draft_positions p ON p.id=e.id", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    check_bytes(bytes.saturating_add(order.len().saturating_sub(1)).saturating_add(2))?;
    // 旧集合仅在新集合完整提交时移除；失败由外层事务回滚。
    conn.execute("DELETE FROM query_drafts", []).map_err(|e| e.to_string())?;
    Ok(())
}

fn position_bytes(from: usize, to: usize, active: bool) -> usize {
    // JSON 数值与布尔值的长度，与 SQL 汇总预算公式一致。
    from.to_string().len() + to.to_string().len() + if active { 4 } else { 5 }
}
#[tauri::command(rename_all = "camelCase")]
/// 读取可恢复 SQL 文档，不恢复结果、连接或事务。
pub async fn dbc_drafts(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
) -> Result<Vec<QueryDraft>, String> {
    let db = store::db(&app, &store_state)?;
    tokio::task::spawn_blocking(move || db.with_conn(load))
    .await
    .map_err(|e| format!("读取 SQL 草稿任务失败：{e}"))?
}
#[tauri::command(rename_all = "camelCase")]
/// 原子保存变更文档与完整顺序；缺省顺序兼容旧版全量调用。
pub async fn dbc_drafts_save(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    drafts: Vec<QueryDraft>,
    order: Option<Vec<String>>,
    positions: Option<Vec<QueryDraftPosition>>,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        let db = store::db(&app, &store_state)?;
        tokio::task::spawn_blocking(move || db.with_transaction(|conn| save(conn, drafts, order, positions.unwrap_or_default())))
        .await
        .map_err(|e| format!("保存 SQL 草稿任务失败：{e}"))?
    }
    .await;
    match &result {
        Ok(_value) => log::debug!(
            "操作完成 operation=dbc_drafts_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=dbc_drafts_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;

    fn draft(id: &str, sql: &str) -> QueryDraft {
        serde_json::from_value(serde_json::json!({"id":id,"sql":sql,"label":"a","connectionId":"c","database":"d","schema":"s","from":0,"to":0,"dirty":true,"active":true})).unwrap()
    }

    fn commit(conn: &Connection, drafts: Vec<QueryDraft>, order: Vec<&str>) -> Result<(), String> {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        save(&tx, drafts, Some(order.into_iter().map(str::to_string).collect()), vec![])?;
        tx.commit().map_err(|e| e.to_string())
    }

    #[test]
    fn selection_updates_do_not_rewrite_sql_and_removal_clears_positions() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        commit(&conn, vec![draft("a", &"x".repeat(70000))], vec!["a"]).unwrap();
        conn.execute_batch("CREATE TRIGGER preserve_body BEFORE UPDATE OF content ON query_draft_entries BEGIN SELECT RAISE(ABORT,'选区不应重写正文'); END;").unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        save(&tx, vec![], Some(vec!["a".into()]), vec![QueryDraftPosition {
            id: "a".into(), from: 120, to: 130, active: true,
        }]).unwrap();
        tx.commit().unwrap();
        let restored = load(&conn).unwrap();
        assert_eq!(restored[0].sql.len(), 70000);
        assert_eq!((restored[0].from, restored[0].to, restored[0].active), (120, 130, true));
        commit(&conn, vec![], vec![]).unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM query_draft_positions", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn selection_budget_matches_original_serialized_document_and_failures_roll_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        let original = draft("a", "sql");
        commit(&conn, vec![original.clone()], vec!["a"]).unwrap();
        // active=true 比固定 false 少一字节，避免零选区减法下溢。
        assert!(load(&conn).unwrap()[0].active);
        let before: String = conn.query_row("SELECT content FROM query_draft_entries", [], |r| r.get(0)).unwrap();
        let mut updated = original;
        updated.id = None;
        updated.from = 1234;
        updated.to = 5678;
        assert_eq!(before.len() + position_bytes(updated.from, updated.to, updated.active) - 7, serde_json::to_string(&updated).unwrap().len());
        {
            let tx = conn.unchecked_transaction().unwrap();
            assert!(save(&tx, vec![draft("b", "new")], Some(vec!["a".into(), "b".into()]), vec![
                QueryDraftPosition { id: "a".into(), from: 99, to: 99, active: false },
                QueryDraftPosition { id: "a".into(), from: 1, to: 1, active: true },
            ]).is_err());
        }
        assert_eq!(load(&conn).unwrap().len(), 1);
        assert_eq!(load(&conn).unwrap()[0].from, 0);
    }

    #[test]
    fn older_entry_selection_survives_new_table_and_first_full_save() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        let mut old = draft("a", "legacy sql");
        old.id = None;
        old.from = 2;
        old.to = 5;
        conn.execute("INSERT INTO query_draft_entries VALUES('a',0,?1)", [serde_json::to_string(&old).unwrap()]).unwrap();
        let restored = load(&conn).unwrap();
        assert_eq!((restored[0].from, restored[0].to, restored[0].active), (2, 5, true));
        // 缺少选区基线时拒绝部分更新；同事务内已写的新正文也必须回滚。
        {
            let tx = conn.unchecked_transaction().unwrap();
            assert!(save(&tx, vec![draft("b", "temporary")], Some(vec!["a".into(), "b".into()]), vec![
                QueryDraftPosition { id: "a".into(), from: 7, to: 7, active: false },
            ]).is_err());
        }
        assert_eq!(load(&conn).unwrap().len(), 1);
        commit(&conn, restored, vec!["a"]).unwrap();
        let updated = load(&conn).unwrap();
        assert_eq!((updated[0].from, updated[0].to, updated[0].active), (2, 5, true));
        assert_eq!(updated[0].sql, "legacy sql");
    }

    #[test]
    fn delta_preserves_other_documents_and_commits_removal_and_order() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        commit(&conn, vec![draft("a", "old a"), draft("b", "old b")], vec!["a", "b"]).unwrap();
        conn.execute_batch("CREATE TRIGGER preserve_a BEFORE UPDATE OF content ON query_draft_entries WHEN OLD.id='a' BEGIN SELECT RAISE(ABORT,'不应重写未修改正文'); END;").unwrap();
        commit(&conn, vec![draft("b", "new b")], vec!["b", "a"]).unwrap();
        let result = load(&conn).unwrap();
        assert_eq!(result.iter().map(|draft| draft.sql.as_str()).collect::<Vec<_>>(), ["new b", "old a"]);
        assert!(commit(&conn, vec![draft("b", "wrong")], vec!["b", "missing"]).is_err());
        assert_eq!(load(&conn).unwrap()[0].sql, "new b");
        commit(&conn, vec![], vec!["a"]).unwrap();
        assert_eq!(load(&conn).unwrap().len(), 1);
        commit(&conn, vec![], vec![]).unwrap();
        assert!(load(&conn).unwrap().is_empty());
    }

    #[test]
    fn legacy_snapshot_is_preserved_until_successful_replacement() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        let mut old = draft("unused", "legacy sql");
        old.id = None;
        conn.execute("INSERT INTO query_drafts VALUES(1,?1)", [encode(&[old]).unwrap()]).unwrap();
        assert_eq!(load(&conn).unwrap()[0].sql, "legacy sql");
        assert!(commit(&conn, vec![], vec!["missing"]).is_err());
        assert_eq!(load(&conn).unwrap()[0].sql, "legacy sql");
        commit(&conn, vec![draft("new", "new sql")], vec!["new"]).unwrap();
        assert_eq!(load(&conn).unwrap()[0].id.as_deref(), Some("new"));
        let old_count: i64 = conn.query_row("SELECT COUNT(*) FROM query_drafts", [], |row| row.get(0)).unwrap();
        assert_eq!(old_count, 0);
    }

    #[test]
    fn total_budget_failure_rolls_back_changed_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, store::MIGRATIONS).unwrap();
        commit(&conn, vec![draft("a", "original")], vec!["a"]).unwrap();
        let ids: Vec<String> = (0..9).map(|index| index.to_string()).collect();
        let drafts = ids.iter().map(|id| draft(id, &"x".repeat(1024 * 1024))).collect();
        assert!(commit(&conn, drafts, ids.iter().map(String::as_str).collect()).is_err());
        assert_eq!(load(&conn).unwrap()[0].sql, "original");
    }
    #[test]
    fn rejects_oversize_drafts_without_truncating_sql() {
        let draft: QueryDraft = serde_json::from_value(serde_json::json!({"sql":"x".repeat(1024*1024+1),"label":"a","connectionId":"c","database":"d","schema":"s","from":0,"to":0,"dirty":true,"active":true})).unwrap();
        assert!(encode(&[draft]).is_err());
        assert_eq!(encode(&[]).unwrap(), "[]");
    }
}
