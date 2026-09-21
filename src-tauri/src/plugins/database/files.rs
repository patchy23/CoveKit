//! SQL 文件与已加载结果导出；编码和大小限制在后端执行。
use crate::plugins::database::models::DbValue;

/// 读取 UTF-8 SQL 文件，拒绝超大文件而非先无限分配。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_sql_file_read(path: String) -> Result<String, String> {
    let log_started = std::time::Instant::now();
    let result: Result<String, String> = async {
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| format!("SQL 文件不可读: {e}"))?;
        if metadata.len() > 1024 * 1024 {
            return Err("SQL 文件超过 1 MiB，请拆分后打开".into());
        }
        let value = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("读取 UTF-8 SQL 文件失败: {e}"))?;
        Ok(value.trim_start_matches('\u{feff}').to_string())
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_sql_file_read elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_sql_file_read elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 保存 SQL 草稿文件，路径由公共文件对话框选择。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_sql_file_write(path: String, sql: String) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        if path.trim().is_empty() || sql.len() > 1024 * 1024 {
            return Err("SQL 路径为空或文件超过 1 MiB".into());
        }
        atomic_write(&path, sql.as_bytes()).await
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_sql_file_write elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_sql_file_write elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 在目标目录使用唯一临时文件，成功后替换；失败只清理本次创建的文件。
pub(crate) async fn atomic_write(path: &str, bytes: &[u8]) -> Result<(), String> {
    let destination = std::path::Path::new(path);
    let parent = destination.parent().ok_or("文件路径缺少目录")?;
    let temporary = parent.join(format!(".covekit-{}.tmp", uuid::Uuid::new_v4()));
    let outcome = async {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).await.map_err(|e| e.to_string())?;
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        tokio::fs::rename(&temporary, destination)
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    if outcome.is_err() {
        if let Err(error) = tokio::fs::remove_file(&temporary).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "清理未完成导出文件失败: {error_type}",
                    error_type = std::any::type_name_of_val(&error)
                );
            }
        }
    }
    outcome
}

/// 导出原值约定：NULL 为 \N；文本的前导反斜线加一层，便于本工具无损导入。
pub(crate) fn csv_value(value: &DbValue) -> String {
    match &value.value {
        None => "\\N".into(),
        Some(text) if text.starts_with('\\') => format!("\\{text}"),
        Some(text) => text.clone(),
    }
}

/// CSV 库对表头和数据使用同一编码器，不再手写逗号/换行转义。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_export_rows(
    path: String,
    columns: Vec<String>,
    rows: Vec<Vec<DbValue>>,
) -> Result<u64, String> {
    let log_started = std::time::Instant::now();
    let result: Result<u64, String> = async {
        if rows.len() > 100_000 || columns.len() > 2000 {
            return Err("导出规模超过已加载结果限制".into());
        }
        let count = rows.len() as u64;
        let bytes = tokio::task::spawn_blocking(move || {
            let mut writer = csv::WriterBuilder::new()
                .terminator(csv::Terminator::CRLF)
                .from_writer(Vec::new());
            writer.write_record(&columns).map_err(|e| e.to_string())?;
            for row in &rows {
                if row.len() != columns.len() {
                    return Err("导出行列数量不一致".to_string());
                }
                writer
                    .write_record(row.iter().map(csv_value))
                    .map_err(|e| e.to_string())?;
            }
            writer.into_inner().map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        atomic_write(&path, &bytes).await?;
        Ok(count)
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dbc_export_rows elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dbc_export_rows elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}
