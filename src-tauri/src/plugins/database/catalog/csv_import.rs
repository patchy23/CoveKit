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

/// 一次读取并固定待导入字节；预览指纹和后续分批解析使用同一份内容。
struct CsvSource {
    reader: csv::Reader<std::io::Cursor<Vec<u8>>>,
    headers: Vec<String>,
    fingerprint: String,
    count: u64,
    byte_offset: u64,
    cancelled: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

fn check_csv_cancel(
    cancelled: &Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<(), String> {
    if cancelled.as_ref().is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire)) {
        Err("DB_CANCELLED: 操作已取消，未提交修改将回滚".into())
    } else {
        Ok(())
    }
}

impl CsvSource {
    async fn open(
        path: &str,
        cancelled: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<Self, String> {
        use tokio::io::AsyncReadExt;
        check_csv_cancel(&cancelled)?;
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| format!("CSV 无法打开：{e}"))?;
        let mut input = file.take(16 * 1024 * 1024 + 1);
        let mut bytes = Vec::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            check_csv_cancel(&cancelled)?;
            let length = input.read(&mut buffer).await.map_err(|e| e.to_string())?;
            if length == 0 {
                break;
            }
            if bytes.len() + length > 16 * 1024 * 1024 {
                return Err("CSV 超过 16 MiB，请拆分后导入".into());
            }
            bytes.extend_from_slice(&buffer[..length]);
        }
        tokio::task::spawn_blocking(move || {
            check_csv_cancel(&cancelled)?;
            let fingerprint = hex::encode(Sha256::digest(&bytes));
            let text = std::str::from_utf8(&bytes).map_err(|_| "CSV 必须为 UTF-8 编码")?;
            let offset = text.len() - text.trim_start_matches('\u{feff}').len();
            let mut cursor = std::io::Cursor::new(bytes);
            // offset 受既有 16 MiB 文件上限约束。
            cursor.set_position(offset as u64);
            let mut reader = csv::ReaderBuilder::new().from_reader(cursor);
            let headers: Vec<String> = reader.headers().map_err(|e| e.to_string())?
                .iter().map(str::to_string).collect();
            if headers.is_empty() || headers.len() > 512 {
                return Err("CSV 列数须为 1 至 512".into());
            }
            Ok(Self { reader, headers, fingerprint, count: 0, byte_offset: offset as u64, cancelled })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    fn read_record(&mut self) -> Result<Option<csv::StringRecord>, String> {
        check_csv_cancel(&self.cancelled)?;
        let mut record = csv::StringRecord::new();
        let found = self.reader.read_record(&mut record)
            .map_err(|e| format!("CSV 第 {} 条记录格式错误：{e}", self.count + 1))?;
        if !found {
            return Ok(None);
        }
        if self.count >= 10_000 {
            return Err("一次导入最多 10000 行，请拆分文件".into());
        }
        self.count += 1;
        Ok(Some(record))
    }

    /// 写入前验证完整快照，保留原来坏文件不会发出任何 INSERT 的行为。
    /// 第二遍只解析同一份内存字节，不重读文件，也不保留全文件的单元格对象。
    async fn validated(mut self) -> Result<Self, String> {
        tokio::task::spawn_blocking(move || {
            let first_record = self.reader.position().clone();
            while self.read_record()?.is_some() {}
            self.reader.seek_raw(
                std::io::SeekFrom::Start(first_record.byte() + self.byte_offset),
                first_record,
            ).map_err(|e| format!("CSV 无法开始导入：{e}"))?;
            self.count = 0;
            Ok(self)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// 批次只约束暂存，不限制行或字段；超大合法单行仍完整交付。
    /// 解析留在阻塞池，调用者停止请求下一批后没有后台生产者或待发送队列。
    async fn next_batch(mut self) -> Result<(Self, Vec<csv::StringRecord>), String> {
        tokio::task::spawn_blocking(move || {
            let mut batch = Vec::new();
            let mut bytes = 0;
            while batch.len() < 128 && bytes < 64 * 1024 {
                let Some(record) = self.read_record()? else {
                    break;
                };
                bytes += record.as_slice().len();
                batch.push(record);
            }
            Ok((self, batch))
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

/// 预览仍校验完整文件，只为展示行生成逐单元格字符串。
async fn read_csv(
    path: &str,
    retained_rows: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>, String, u64), String> {
    let mut source = CsvSource::open(path, None).await?;
    // 预览没有数据库消费等待，整轮在同一个阻塞任务校验，避免逐批跨线程调度。
    tokio::task::spawn_blocking(move || {
        let mut rows = Vec::new();
        while let Some(record) = source.read_record()? {
            if rows.len() < retained_rows {
                rows.push(record.iter().map(str::to_string).collect());
            }
        }
        Ok((source.headers, rows, source.fingerprint, source.count))
    })
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command(rename_all = "camelCase")]
/// 读取受限 CSV，返回列映射预览与内容指纹，不写入目标数据库。
pub async fn dbc_csv_preview(path: String) -> Result<CsvPreview, String> {
    let (columns, rows, fingerprint, total) = read_csv(&path, 20).await?;
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
    let log_started = std::time::Instant::now();
    let result: Result<u64, String> = async {
        let config = store::list_connections(&app, &store_state)?
            .into_iter()
            .find(|c| c.id == conn_id)
            .ok_or("连接已删除")?;
        if config.readonly {
            return Err("DB_READ_ONLY: 当前连接只读".into());
        }
        let mut task = BoundTask::register(&cancel_state, request_id, &conn_id)?;
        let source = CsvSource::open(&path, Some(task.handle.aborted.clone())).await?;
        if fingerprint != source.fingerprint {
            return Err("CSV 文件在预览后已变化，请重新预览确认".into());
        }
        let source = source.validated().await?;
        let (mut source, mut rows) = source.next_batch().await?;
        if mapping.is_empty() || rows.is_empty() {
            return Err("没有待导入的列或数据".into());
        }
        let entry =
            super::scoped_session(&app, &state, &secrets_state, &conn_id, database.as_deref())
                .await?;
        let columns = columns_for_entry(&entry, schema.clone(), table.clone()).await?;
        let mut seen = std::collections::HashSet::new();
        for item in &mapping {
            if item.source >= source.headers.len()
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
            loop {
                for row in rows.drain(..) {
                    let index = count;
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
                let (next, batch) = source.next_batch().await?;
                source = next;
                rows = batch;
                if rows.is_empty() {
                    break;
                }
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
                .map_err(|e| {
                    format!("DB_OUTCOME_UNKNOWN: 提交状态无法确认，请核对数据再操作：{e}")
                })?;
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
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_csv_import elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_csv_import elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 解析分批保留，不为完整文件分配逐单元格 String；取消不再解析下一批。
    #[tokio::test]
    async fn import_batches_preserve_content_and_stop_on_cancel() {
        use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
        let path = std::env::temp_dir().join(format!("covekit-csv-batch-{}.csv", uuid::Uuid::new_v4()));
        let text = format!("\u{feff}id,text\n{}", (0..1000).map(|i| format!("{i},\"中文,\\N\"\n")).collect::<String>());
        tokio::fs::write(&path, &text).await.unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let source = CsvSource::open(path.to_str().unwrap(), Some(cancel.clone())).await.unwrap();
        let mut source = source.validated().await.unwrap();
        assert_eq!(source.fingerprint, hex::encode(Sha256::digest(text.as_bytes())));
        // 文件后续改变不影响已读取的固定字节和对应指纹。
        tokio::fs::write(&path, "changed\n").await.unwrap();
        let mut count = 0;
        loop {
            let (next, batch) = source.next_batch().await.unwrap();
            source = next;
            if batch.is_empty() { break; }
            assert!(batch.len() <= 128);
            for row in batch {
                assert_eq!(&row[0], count.to_string());
                assert_eq!(&row[1], "中文,\\N");
                count += 1;
            }
        }
        assert_eq!(count, 1000);
        cancel.store(true, Ordering::Release);
        assert!(matches!(source.next_batch().await, Err(error) if error.starts_with("DB_CANCELLED")));
        tokio::fs::remove_file(path).await.unwrap();
    }

    /// 后部坏行在后续批次返回错误，导入方据此回滚同一事务。
    #[tokio::test]
    async fn import_reports_errors_after_the_first_batch() {
        let path = std::env::temp_dir().join(format!("covekit-csv-tail-{}.csv", uuid::Uuid::new_v4()));
        let text = format!("a,b\n{}bad,extra,column\n", "1,2\n".repeat(128));
        tokio::fs::write(&path, text).await.unwrap();
        let source = CsvSource::open(path.to_str().unwrap(), None).await.unwrap();
        let (source, batch) = source.next_batch().await.unwrap();
        assert_eq!(batch.len(), 128);
        assert!(matches!(source.next_batch().await, Err(error) if error.contains("第 129 条")));
        let source = CsvSource::open(path.to_str().unwrap(), None).await.unwrap();
        assert!(matches!(source.validated().await, Err(error) if error.contains("第 129 条")));
        tokio::fs::remove_file(path).await.unwrap();
    }

    /// 只保留展示行，但计数、指纹和后续记录校验仍覆盖完整文件。
    #[tokio::test]
    async fn preview_keeps_twenty_rows_and_validates_the_tail() {
        let path = std::env::temp_dir().join(format!("covekit-preview-{}.csv", uuid::Uuid::new_v4()));
        let mut text = String::from("id,text\n");
        for index in 0..100 {
            text.push_str(&format!("{index},value\n"));
        }
        tokio::fs::write(&path, &text).await.unwrap();
        let name = path.to_str().unwrap();
        let preview = dbc_csv_preview(name.into()).await.unwrap();
        assert_eq!(preview.total, 100);
        assert_eq!(preview.rows.len(), 20);
        assert_eq!(preview.rows[19][0], "19");
        let (_, imported, fingerprint, total) = read_csv(name, 10_000).await.unwrap();
        assert_eq!(imported.len(), 100);
        assert_eq!(total, preview.total);
        assert_eq!(fingerprint, preview.fingerprint);
        text.push_str("bad,extra,column\n");
        tokio::fs::write(&path, text).await.unwrap();
        let error = dbc_csv_preview(name.into()).await.unwrap_err();
        assert!(error.contains("第 101 条"));
        tokio::fs::remove_file(path).await.unwrap();
    }
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
