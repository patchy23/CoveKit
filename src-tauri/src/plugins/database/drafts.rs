//! SQL 草稿只保存文档和目标，不保存结果、密码或活动事务。
use super::{
    models::QueryDraft,
    store::{self, StoreState},
};
use tauri::State;

fn encode(drafts: &[QueryDraft]) -> Result<String, String> {
    if drafts.len() > 50 {
        return Err("最多恢复 50 个 SQL 草稿".into());
    }
    for draft in drafts {
        if draft.sql.len() > 1024 * 1024 || draft.label.len() > 1024 {
            return Err("单个 SQL 草稿不能超过 1 MiB，名称不能超过 1024 字节".into());
        }
    }
    let value = serde_json::to_string(drafts).map_err(|e| e.to_string())?;
    if value.len() > 8 * 1024 * 1024 {
        return Err("SQL 草稿总量超过 8 MiB，请另存 SQL 文件后关闭部分页签".into());
    }
    Ok(value)
}
#[tauri::command(rename_all = "camelCase")]
/// 读取可恢复 SQL 文档，不恢复结果、连接或事务。
pub async fn dbc_drafts(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
) -> Result<Vec<QueryDraft>, String> {
    store::db(&app, &store_state)?.with_conn(|conn| {
        use rusqlite::OptionalExtension;
        let content: Option<String> = conn
            .query_row("SELECT content FROM query_drafts WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        let drafts = match content {
            Some(value) => {
                serde_json::from_str(&value).map_err(|e| format!("草稿损坏，原记录已保留：{e}"))?
            }
            None => Vec::new(),
        };
        encode(&drafts)?;
        Ok(drafts)
    })
}
#[tauri::command(rename_all = "camelCase")]
/// 验证草稿预算后原子替换当前草稿集合。
pub async fn dbc_drafts_save(
    app: tauri::AppHandle,
    store_state: State<'_, StoreState>,
    drafts: Vec<QueryDraft>,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
    let content = encode(&drafts)?;
    store::db(&app, &store_state)?.with_conn(|conn| {
        conn.execute("INSERT INTO query_drafts(id,content) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET content=excluded.content", [&content]).map_err(|e|e.to_string())?;
        Ok(())
    })
    }.await;
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
    #[test]
    fn rejects_oversize_drafts_without_truncating_sql() {
        let draft: QueryDraft = serde_json::from_value(serde_json::json!({"sql":"x".repeat(1024*1024+1),"label":"a","connectionId":"c","database":"d","schema":"s","from":0,"to":0,"dirty":true,"active":true})).unwrap();
        assert!(encode(&[draft]).is_err());
        assert_eq!(encode(&[]).unwrap(), "[]");
    }
}
