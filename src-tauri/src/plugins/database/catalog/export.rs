//! 完整结果流式导出：独立只读会话、真实取消、临时文件成功后替换。
use crate::plugins::database::{
    drivers::{
        self,
        workspace::{self, WorkspaceConnection},
        CancelHandle, DbCancelState, DbState,
    },
    files::csv_value,
    models::{DbValue, ExecutionScope},
    secrets::{self, SecretsState},
    sql_analysis,
    store::{self, StoreState},
};
use mysql_async::prelude::Queryable;
use std::sync::{atomic::Ordering, Arc};
use tauri::State;
use tokio::io::AsyncWriteExt;

struct Registration<'a> {
    state: &'a DbCancelState,
    id: String,
    handle: CancelHandle,
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.handle.finished.store(true, Ordering::Release);
        if let Ok(mut map) = self.state.0.lock() {
            map.remove(&self.id);
        }
    }
}
async fn record(
    writer: &mut tokio::io::BufWriter<tokio::fs::File>,
    cells: &[String],
) -> Result<(), String> {
    if cells.iter().map(String::len).sum::<usize>() > 16 * 1024 * 1024 {
        return Err("单行导出超过 16 MiB，请改用数据库原生导出".into());
    }
    let mut encoder = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    encoder.write_record(cells).map_err(|e| e.to_string())?;
    let bytes = encoder.into_inner().map_err(|e| e.to_string())?;
    writer.write_all(&bytes).await.map_err(|e| e.to_string())
}

/// 只接受一条静态判定只读的语句，重新执行原 SQL，不受网格行数上限影响。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_export_query(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    cancel_state: State<'_, DbCancelState>,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, SecretsState>,
    conn_id: String,
    scope: ExecutionScope,
    sql: String,
    path: String,
    request_id: String,
) -> Result<u64, String> {
    let log_started = std::time::Instant::now();
    let result: Result<u64, String> = async {
        let mut entry = state.entry(&conn_id)?;
        entry.config = store::list_connections(&app, &store_state)?
            .into_iter()
            .find(|c| c.id == conn_id)
            .ok_or("连接配置已删除")?;
        if sql_analysis::split(entry.config.db_type, &sql)?.len() != 1
            || sql_analysis::assess(entry.config.db_type, &sql)?.0
        {
            return Err("完整导出仅接受一条只读查询，不执行脚本或写入语句".into());
        }
        if entry.config.db_type.is_agent() || entry.config.db_type.is_redis() {
            return Err("此驱动暂不支持 SQL 结果流式导出".into());
        }
        entry.config.readonly = true;
        let mut handle = CancelHandle::pending();
        handle.connection_id = conn_id.clone();
        {
            let mut registry = cancel_state.0.lock().map_err(|e| e.to_string())?;
            if request_id.is_empty() || registry.contains_key(&request_id) {
                return Err("导出请求身份无效".into());
            }
            registry.insert(request_id.clone(), handle.clone());
        }
        let registration = Registration {
            state: &cancel_state,
            id: request_id.clone(),
            handle: handle.clone(),
        };
        let password = secrets::secret_get(&app, &secrets_state, &conn_id)?;
        let mut session = workspace::open(&entry, scope, &password).await?;
        match &session.connection {
            WorkspaceConnection::Mysql(conn, pool) => {
                handle.mysql_thread_id = Some(conn.id());
                handle.mysql_pool = Some(pool.clone());
            }
            WorkspaceConnection::Postgres(client) => {
                handle.pg_cancel = Some(client.cancel_token());
                handle.pg_ssl = entry.config.ssl;
            }
            WorkspaceConnection::Sqlite(conn) => {
                handle.sqlite = Some(Arc::new(
                    conn.lock()
                        .map_err(|e| e.to_string())?
                        .get_interrupt_handle(),
                ));
            }
            _ => return Err("驱动不支持流式导出".into()),
        }
        cancel_state
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(request_id, handle.clone());
        let destination = std::path::PathBuf::from(path);
        let parent = destination.parent().ok_or("导出路径缺少目录")?;
        let temporary = parent.join(format!(".covekit-export-{}.tmp", uuid::Uuid::new_v4()));
        let outcome = write_rows(&mut session.connection, &sql, &temporary, &handle).await;
        let gate = handle.gate.lock().await;
        handle.finished.store(true, Ordering::Release);
        let outcome = if handle.aborted.load(Ordering::Acquire) {
            Err("DB_CANCELLED: 导出已取消，目标文件未改动".into())
        } else {
            outcome
        };
        drop(gate);
        let cleanup = workspace::close(session).await;
        let outcome = match outcome {
            Ok(count) => cleanup.map(|_| count),
            Err(error) => Err(error),
        };
        let result = match outcome {
            Ok(count) => tokio::fs::rename(&temporary, &destination)
                .await
                .map(|_| count)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error),
        };
        if result.is_err() {
            if let Err(error) = tokio::fs::remove_file(&temporary).await {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::error!(
                        "未完成导出文件清理失败：{error_type}",
                        error_type = std::any::type_name_of_val(&error)
                    );
                }
            }
        }
        drop(registration);
        result
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_export_query elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_export_query elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 将完整结果逐行编码；用于独立导出任务和驱动契约测试。
pub(crate) async fn write_rows(
    connection: &mut WorkspaceConnection,
    sql: &str,
    temporary: &std::path::Path,
    handle: &CancelHandle,
) -> Result<u64, String> {
    if handle.aborted.load(Ordering::Acquire) {
        return Err("DB_CANCELLED: 导出已取消".into());
    }
    if let WorkspaceConnection::Sqlite(conn) = &*connection {
        let conn = Arc::clone(conn);
        let path = temporary.to_path_buf();
        let sql = sql.to_string();
        let aborted = Arc::clone(&handle.aborted);
        return tokio::task::spawn_blocking(move || {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let mut statement = guard.prepare(&sql).map_err(|e| e.to_string())?;
            if !statement.readonly() || statement.column_count() == 0 {
                return Err("导出需要只读结果集".into());
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            let mut writer = csv::WriterBuilder::new()
                .terminator(csv::Terminator::CRLF)
                .from_writer(file);
            writer
                .write_record(statement.column_names())
                .map_err(|e| e.to_string())?;
            let column_count = statement.column_count();
            let mut rows = statement.query([]).map_err(|e| e.to_string())?;
            let mut count = 0;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                if aborted.load(Ordering::Acquire) {
                    return Err("DB_CANCELLED: 导出已取消".into());
                }
                let mut cells = Vec::with_capacity(column_count);
                for i in 0..column_count {
                    use rusqlite::types::ValueRef;
                    let value = match row.get_ref(i).map_err(|e| e.to_string())? {
                        ValueRef::Null => DbValue::null(),
                        ValueRef::Integer(n) => DbValue::text("integer", n.to_string()),
                        ValueRef::Real(n) => DbValue::text("float", n.to_string()),
                        ValueRef::Text(t) => DbValue::text(
                            "text",
                            std::str::from_utf8(t)
                                .map_err(|e| e.to_string())?
                                .to_string(),
                        ),
                        ValueRef::Blob(b) => DbValue::binary(b),
                    };
                    cells.push(csv_value(&value));
                }
                if cells.iter().map(String::len).sum::<usize>() > 16 * 1024 * 1024 {
                    return Err("单行导出超过 16 MiB".into());
                }
                writer.write_record(cells).map_err(|e| e.to_string())?;
                count += 1;
            }
            writer.flush().map_err(|e| e.to_string())?;
            writer.get_ref().sync_all().map_err(|e| e.to_string())?;
            Ok(count)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|e| e.to_string())?;
    let mut writer = tokio::io::BufWriter::with_capacity(256 * 1024, file);
    let mut count = 0;
    match connection {
        WorkspaceConnection::Mysql(conn, _) => {
            let mut query = conn.query_iter(sql).await.map_err(|e| e.to_string())?;
            let columns = query.columns_ref();
            if columns.is_empty() {
                return Err("导出语句没有结果列".into());
            }
            let names: Vec<_> = columns.iter().map(|c| c.name_str().to_string()).collect();
            let native: Vec<_> = columns
                .iter()
                .map(|c| format!("{:?}", c.column_type()))
                .collect();
            let binary: Vec<_> = columns.iter().map(|c| c.character_set() == 63).collect();
            record(&mut writer, &names).await?;
            while let Some(row) = query.next().await.map_err(|e| e.to_string())? {
                if handle.aborted.load(Ordering::Acquire) {
                    return Err("DB_CANCELLED: 导出已取消".into());
                }
                let mut cells = Vec::with_capacity(row.len());
                for i in 0..row.len() {
                    let value = row
                        .get::<mysql_async::Value, usize>(i)
                        .ok_or("结果缺少单元格")?;
                    cells.push(csv_value(&drivers::mysql::mysql_value(
                        value, binary[i], &native[i],
                    )));
                }
                record(&mut writer, &cells).await?;
                count += 1;
            }
            query.drop_result().await.map_err(|e| e.to_string())?;
        }
        WorkspaceConnection::Postgres(client) => {
            use futures_util::TryStreamExt;
            let prepared = client.prepare(&sql).await.map_err(|e| e.to_string())?;
            if prepared.columns().is_empty() {
                return Err("导出语句没有结果列".into());
            }
            let types: Vec<_> = prepared
                .columns()
                .iter()
                .map(|c| c.type_().name().to_string())
                .collect();
            let stream = client
                .simple_query_raw(&sql)
                .await
                .map_err(|e| e.to_string())?;
            futures_util::pin_mut!(stream);
            while let Some(message) = stream.try_next().await.map_err(|e| e.to_string())? {
                if handle.aborted.load(Ordering::Acquire) {
                    return Err("DB_CANCELLED: 导出已取消".into());
                }
                match message {
                    tokio_postgres::SimpleQueryMessage::RowDescription(columns) => {
                        record(
                            &mut writer,
                            &columns
                                .iter()
                                .map(|c| c.name().to_string())
                                .collect::<Vec<_>>(),
                        )
                        .await?
                    }
                    tokio_postgres::SimpleQueryMessage::Row(row) => {
                        let mut cells = Vec::with_capacity(row.len());
                        for i in 0..row.len() {
                            cells.push(match row.try_get(i).map_err(|e| e.to_string())? {
                                None => csv_value(&DbValue::null()),
                                Some(text) => {
                                    let text = if types.get(i).is_some_and(|t| t == "bytea") {
                                        text.strip_prefix("\\x").unwrap_or(text)
                                    } else {
                                        text
                                    };
                                    csv_value(&DbValue::text("text", text.to_string()))
                                }
                            });
                        }
                        record(&mut writer, &cells).await?;
                        count += 1;
                    }
                    _ => {}
                }
            }
        }
        _ => return Err("驱动不支持流式导出".into()),
    }
    writer.flush().await.map_err(|e| e.to_string())?;
    writer
        .get_ref()
        .sync_all()
        .await
        .map_err(|e| e.to_string())?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sqlite_export_streams_all_rows_with_lossless_csv_values() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let mut connection = WorkspaceConnection::Sqlite(Arc::new(std::sync::Mutex::new(conn)));
        let path =
            std::env::temp_dir().join(format!("covekit-export-test-{}.csv", uuid::Uuid::new_v4()));
        let handle = CancelHandle::pending();
        let sql = "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10001) SELECT i AS \"id,序号\", NULL AS n, 'NULL' AS t, '' AS e, X'00ff' AS b, 'a,b' AS quoted FROM n";
        let count = write_rows(&mut connection, sql, &path, &handle)
            .await
            .unwrap();
        assert_eq!(count, 10001, "完整导出不受网格 1000 行限制");
        let bytes = tokio::fs::read(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        let mut csv = csv::Reader::from_reader(bytes.as_slice());
        assert_eq!(csv.headers().unwrap().get(0), Some("id,序号"));
        let rows: Vec<_> = csv.records().map(Result::unwrap).collect();
        assert_eq!(rows.len(), 10001);
        assert_eq!(rows[0].get(1), Some("\\N"));
        assert_eq!(rows[0].get(2), Some("NULL"));
        assert_eq!(rows[0].get(3), Some(""));
        assert_eq!(rows[0].get(4), Some("00ff"));
        assert_eq!(rows.last().unwrap().get(0), Some("10001"));
        handle.aborted.store(true, Ordering::Release);
        assert!(write_rows(&mut connection, sql, &path, &handle)
            .await
            .unwrap_err()
            .contains("DB_CANCELLED"));
        assert!(!path.exists(), "执行前取消不得创建文件");
    }
}
