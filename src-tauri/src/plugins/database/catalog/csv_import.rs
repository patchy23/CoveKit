//! CSV 导入预览和单事务写入；有界解析、显式列映射、文件指纹和逐行错误。
use super::{
    bound::{BoundConnection, BoundTask},
    metadata::columns_for_entry,
    mutation::mutation_sql,
    table::qualified,
};
use crate::plugins::database::{
    drivers::{DbCancelState, DbState},
    models::{CsvMapping, CsvPreview, DbValue, TableChange},
    secrets::SecretsState,
    store::{self, StoreState},
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tauri::State;

/// 文件上限与行上限在分配和解析阶段都检查；所有文本按 UTF-8 处理。
async fn read_csv(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>, String), String> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("CSV 无法打开：{e}"))?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("CSV 超过 16 MiB，请拆分后导入".into());
    }
    tokio::task::spawn_blocking(move || {
        let fingerprint = hex::encode(Sha256::digest(&bytes));
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| "CSV 必须为 UTF-8 编码")?
            .trim_start_matches('\u{feff}');
        let mut reader = csv::ReaderBuilder::new().from_reader(text.as_bytes());
        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(str::to_string)
            .collect();
        if headers.is_empty() || headers.len() > 512 {
            return Err("CSV 列数须为 1 至 512".into());
        }
        let mut rows = Vec::new();
        for (index, record) in reader.records().enumerate() {
            if index >= 10_000 {
                return Err("一次导入最多 10000 行，请拆分文件".into());
            }
            let record = record.map_err(|e| format!("CSV 第 {} 条记录格式错误：{e}", index + 1))?;
            rows.push(record.iter().map(str::to_string).collect());
        }
        Ok((headers, rows, fingerprint))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command(rename_all = "camelCase")]
/// 读取受限 CSV，返回列映射预览与内容指纹，不写入目标数据库。
pub async fn dbc_csv_preview(path: String) -> Result<CsvPreview, String> {
    let (columns, mut rows, fingerprint) = read_csv(&path).await?;
    let total = rows.len() as u64;
    rows.truncate(20);
    Ok(CsvPreview {
        columns,
        rows,
        total,
        fingerprint,
    })
}
fn value(text: &str, native: &str) -> DbValue {
    if text == "\\N" {
        return DbValue::null();
    }
    let text = text
        .strip_prefix("\\\\")
        .map(|s| format!("\\{s}"))
        .unwrap_or_else(|| text.to_string());
    let native = native.to_lowercase();
    let kind = if native.contains("int") {
        "integer"
    } else if native.contains("blob") || native.contains("binary") || native == "bytea" {
        "binary"
    } else if native == "boolean" || native == "bool" {
        "boolean"
    } else if native.contains("float") || native == "real" || native.contains("double") {
        "float"
    } else {
        "text"
    };
    DbValue::text(kind, text)
}
/// 整个文件一个独立事务；取消/行错误回滚，提交失败明确结果未知。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_csv_import(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, SecretsState>,
    cancel_state: State<'_, DbCancelState>,
    conn_id: String,
    database: Option<String>,
    schema: Option<String>,
    table: String,
    path: String,
    fingerprint: String,
    mapping: Vec<CsvMapping>,
    request_id: String,
) -> Result<u64, String> {
    let config = store::list_connections(&app, &store_state)?
        .into_iter()
        .find(|c| c.id == conn_id)
        .ok_or("连接已删除")?;
    if config.readonly {
        return Err("DB_READ_ONLY: 当前连接只读".into());
    }
    let mut task = BoundTask::register(&cancel_state, request_id, &conn_id)?;
    let (headers, rows, current) = read_csv(&path).await?;
    if fingerprint != current {
        return Err("CSV 文件在预览后已变化，请重新预览确认".into());
    }
    if mapping.is_empty() || rows.is_empty() {
        return Err("没有待导入的列或数据".into());
    }
    let entry =
        super::scoped_session(&app, &state, &secrets_state, &conn_id, database.as_deref()).await?;
    let columns = columns_for_entry(&entry, schema.clone(), table.clone()).await?;
    let mut seen = std::collections::HashSet::new();
    for item in &mapping {
        if item.source >= headers.len()
            || !columns.iter().any(|c| c.name == item.column)
            || !seen.insert(&item.column)
        {
            return Err("列映射无效或目标列重复，请重新选择".into());
        }
    }
    let name = qualified(&entry, schema.as_deref(), &table);
    let mut conn = BoundConnection::open(&entry).await?;
    task.bind(&conn, &entry).await?;
    if !state.is_current(&entry)? {
        return Err("连接已变化，导入未执行".into());
    }
    task.query(&mut conn, "BEGIN", &[]).await?;
    let started = std::time::Instant::now();
    let result = async {
        let mut count = 0;
        for (index, row) in rows.iter().enumerate() {
            task.check()?;
            if started.elapsed().as_secs() >= 120 {
                return Err("导入超过 120 秒，整批回滚；请拆分文件后重试".into());
            }
            let values = mapping
                .iter()
                .map(|item| {
                    let column = columns
                        .iter()
                        .find(|c| c.name == item.column)
                        .ok_or("列结构已变化")?;
                    Ok((
                        item.column.clone(),
                        value(&row[item.source], &column.data_type),
                    ))
                })
                .collect::<Result<HashMap<_, _>, String>>()?;
            let change = TableChange {
                action: "insert".into(),
                original: HashMap::new(),
                values,
            };
            let (sql, params) = mutation_sql(entry.config.db_type, &name, &columns, &change)?;
            let result = task
                .query(&mut conn, &sql, &params)
                .await
                .map_err(|e| format!("第 {} 条数据导入失败：{e}", index + 1))?;
            if result.rows_affected != 1 {
                return Err(format!("第 {} 条数据影响行数异常", index + 1));
            }
            count += 1;
        }
        task.check()?;
        Ok::<u64, String>(count)
    }
    .await;
    let result = match result {
        Ok(count) => task.before_commit().await.map(|()| count),
        Err(error) => Err(error),
    };
    match result {
        Ok(count) => {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                conn.query("COMMIT", &[], 1),
            )
            .await
            .map_err(|_| "DB_OUTCOME_UNKNOWN: 提交超时，请核对导入结果")?
            .map_err(|e| format!("DB_OUTCOME_UNKNOWN: 提交状态无法确认，请核对数据再操作：{e}"))?;
            Ok(count)
        }
        Err(error) => {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                conn.query("ROLLBACK", &[], 1),
            )
            .await
            .map_err(|_| "DB_OUTCOME_UNKNOWN: 导入回滚超时，请核对数据")?
            .map_err(|e| format!("{error}；回滚失败：{e}"))?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preview_preserves_quoted_newlines_null_escape_and_file_fingerprint() {
        let path =
            std::env::temp_dir().join(format!("covekit-import-test-{}.csv", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, "id,text\r\n1,\"中文,\n多行\"\r\n2,\\N\r\n")
            .await
            .unwrap();
        let name = path.to_str().unwrap();
        let first = dbc_csv_preview(name.into()).await.unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.rows[0][1], "中文,\n多行");
        assert_eq!(value(&first.rows[1][1], "text").kind, "null");
        assert_eq!(value("\\\\N", "text").value.as_deref(), Some("\\N"));
        tokio::fs::write(&path, "id,text\n1,modified\n")
            .await
            .unwrap();
        let changed = dbc_csv_preview(name.into()).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_ne!(first.fingerprint, changed.fingerprint);
    }
}
