//! 单表写入：真实主键、原值并发校验、参数绑定、整批事务。
use super::{
    bound::{BoundConnection, BoundTask},
    metadata::columns_for_entry,
    table::{qualified, quote},
    table_sql::placeholder,
};
use crate::plugins::database::{
    drivers::DbState,
    models::{DbColumnInfo, DbType, DbValue, TableChange},
    secrets::SecretsState,
    store::{self, StoreState},
};
use tauri::State;

/// 只生成经过结构核验的一条语句，所有单元格都保留为绑定值。
pub(crate) fn mutation_sql(
    kind: DbType,
    table: &str,
    columns: &[DbColumnInfo],
    change: &TableChange,
) -> Result<(String, Vec<DbValue>), String> {
    let known = |name: &String| columns.iter().any(|column| &column.name == name);
    if change
        .values
        .keys()
        .chain(change.original.keys())
        .any(|key| !known(key))
    {
        return Err("列结构已变化，请刷新后重试".into());
    }
    let mut values = Vec::new();
    let mut changed: Vec<_> = change.values.keys().collect();
    changed.sort();
    let sql = match change.action.as_str() {
        "insert" => {
            if changed.is_empty() {
                return Err("请至少填写一列，其余列使用数据库默认值".into());
            }
            let names = changed
                .iter()
                .map(|name| quote(kind, name))
                .collect::<Vec<_>>()
                .join(", ");
            let params = changed
                .iter()
                .map(|name| {
                    let token = placeholder(kind, values.len());
                    values.push(change.values[*name].clone());
                    token
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("INSERT INTO {table} ({names}) VALUES ({params})")
        }
        "update" | "delete" => {
            let keys: Vec<_> = columns.iter().filter(|column| column.key == "PK").collect();
            if keys.is_empty() {
                return Err("没有主键的表禁止直接更新或删除，请使用 SQL".into());
            }
            if change.original.len() != columns.len() {
                return Err("原始行不完整，禁止覆盖写入".into());
            }
            if keys.iter().any(|key| {
                change
                    .original
                    .get(&key.name)
                    .is_none_or(|value| value.kind == "null")
            }) {
                return Err("主键值缺失，禁止更新或删除".into());
            }
            let prefix = if change.action == "update" {
                if changed.is_empty() {
                    return Err("没有待提交的修改".into());
                }
                let assignments = changed
                    .iter()
                    .map(|name| {
                        let token = placeholder(kind, values.len());
                        values.push(change.values[*name].clone());
                        format!("{} = {token}", quote(kind, name))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("UPDATE {table} SET {assignments}")
            } else {
                format!("DELETE FROM {table}")
            };
            let mut conditions = Vec::new();
            for column in columns {
                let value = change.original.get(&column.name).ok_or("原始值缺失")?;
                let name = quote(kind, &column.name);
                if value.kind == "null" {
                    conditions.push(format!("{name} IS NULL"));
                } else {
                    let token = placeholder(kind, values.len());
                    values.push(value.clone());
                    // PostgreSQL JSON 类型没有等号；文本投影也能保留原始格式作并发比较。
                    if kind == DbType::Postgresql && column.data_type == "json" {
                        conditions.push(format!("CAST({name} AS TEXT) = CAST({token} AS TEXT)"));
                    } else if matches!(kind, DbType::Mysql | DbType::Polardb)
                        && ["text", "json"].contains(&value.kind.as_str())
                    {
                        // 文本原值比较必须区分大小写与尾空格，避免排序规则吞掉并发改动。
                        conditions
                            .push(format!("CAST({name} AS BINARY) = CAST({token} AS BINARY)"));
                    } else {
                        conditions.push(format!("{name} = {token}"));
                    }
                }
            }
            format!("{prefix} WHERE {}", conditions.join(" AND "))
        }
        _ => return Err("未知行操作".into()),
    };
    Ok((sql, values))
}

/// 一次最多 500 行，任何冲突或错误均回滚；事务状态不与 SQL 工作页共享。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_apply(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    cancel_state: State<'_, crate::plugins::database::drivers::DbCancelState>,
    store_state: State<'_, StoreState>,
    secrets_state: State<'_, SecretsState>,
    conn_id: String,
    database: Option<String>,
    schema: Option<String>,
    table: String,
    changes: Vec<TableChange>,
    request_id: String,
) -> Result<u64, String> {
    let config = store::list_connections(&app, &store_state)?
        .into_iter()
        .find(|c| c.id == conn_id)
        .ok_or("连接已删除")?;
    if config.readonly {
        return Err("DB_READ_ONLY: 当前连接只读".into());
    }
    if changes.is_empty() || changes.len() > 500 {
        return Err("每次提交须为 1 至 500 行".into());
    }
    let mut task = BoundTask::register(&cancel_state, request_id, &conn_id)?;
    let entry =
        super::scoped_session(&app, &state, &secrets_state, &conn_id, database.as_deref()).await?;
    let columns = columns_for_entry(&entry, schema.clone(), table.clone()).await?;
    let name = qualified(&entry, schema.as_deref(), &table);
    let statements = changes
        .iter()
        .map(|change| mutation_sql(entry.config.db_type, &name, &columns, change))
        .collect::<Result<Vec<_>, _>>()?;
    let mut conn = BoundConnection::open(&entry).await?;
    task.bind(&conn, &entry).await?;
    if !state.is_current(&entry)? {
        return Err("连接已变化，写入未执行".into());
    }
    task.query(&mut conn, "BEGIN", &[]).await?;
    let started = std::time::Instant::now();
    let result = async {
        let mut affected = 0;
        for (index, (sql, values)) in statements.iter().enumerate() {
            if started.elapsed().as_secs() >= 120 {
                return Err("批量写入超过 120 秒，整批回滚".into());
            }
            let result = task.query(&mut conn, sql, values).await?;
            if result.rows_affected != 1 {
                return Err(format!(
                    "第 {} 行影响 {} 行，原始数据可能已变化；整批操作已回滚，请刷新后重试",
                    index + 1,
                    result.rows_affected
                ));
            }
            affected += result.rows_affected;
        }
        Ok::<u64, String>(affected)
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
            .map_err(|_| "DB_OUTCOME_UNKNOWN: 提交超时，请核对数据后再操作")?
            .map_err(|e| {
                format!("DB_OUTCOME_UNKNOWN: 提交结果无法确认，请刷新核对，勿直接重复提交：{e}")
            })?;
            Ok(count)
        }
        Err(error) => match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            conn.query("ROLLBACK", &[], 1),
        )
        .await
        .map_err(|_| "回滚超时".to_string())
        .and_then(|result| result)
        {
            Ok(_) => Err(error),
            Err(rollback) => Err(format!("{error}；回滚失败：{rollback}，请核对数据库状态")),
        },
    }
}
