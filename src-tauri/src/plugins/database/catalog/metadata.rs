//! 数据库查询与目录 · metadata

use super::session;
use crate::plugins::database::dialect::dialect_or_err;
use crate::plugins::database::drivers;
use crate::plugins::database::drivers::DbSession;
use crate::plugins::database::drivers::DbState;
use crate::plugins::database::models::DbColumnInfo;
use crate::plugins::database::models::DbObjectInfo;
use mysql_async::prelude::Queryable;
use tauri::State;

// ──────────────────────────────────────────────────────────────────────────
// 元数据命令
// ──────────────────────────────────────────────────────────────────────────

/// 数据库列表
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_databases(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<Vec<String>, String> {
    let entry = session(&state, &conn_id)?;
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            drivers::mysql::query_strings_mysql(
                pool,
                dialect.databases_sql().ok_or("该类型无库列表")?,
            )
            .await
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            drivers::postgres::query_strings_pg(
                pool,
                dialect.databases_sql().ok_or("该类型无库列表")?,
            )
            .await
        }
        DbSession::Sqlite(_) => Ok(vec!["main".to_string()]),
        DbSession::Redis(_) => Ok(vec![entry.config.database.clone()]),
        DbSession::Agent { client, session_id } => client.list_databases(session_id).await,
    }
}

/// schema 列表（mysql/redis 无 schema 层返回空）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_schemas(
    app: tauri::AppHandle,
    secrets_state: State<'_, crate::plugins::database::secrets::SecretsState>,
    database: Option<String>,
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<Vec<String>, String> {
    let entry = crate::plugins::database::catalog::scoped_session(
        &app,
        &state,
        &secrets_state,
        &conn_id,
        database.as_deref(),
    )
    .await?;
    match &entry.session {
        DbSession::Mysql(_) | DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Postgres(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            drivers::postgres::query_strings_pg(
                pool,
                dialect.schemas_sql().ok_or("该类型无 schema 列表")?,
            )
            .await
        }
        DbSession::Sqlite(_) => Ok(vec!["main".to_string()]),
        DbSession::Agent { client, session_id } => client.list_schemas(session_id).await,
    }
}

/// 对象列表（表/视图等；schema 为 null 时用默认值）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_objects(
    app: tauri::AppHandle,
    secrets_state: State<'_, crate::plugins::database::secrets::SecretsState>,
    database: Option<String>,
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
) -> Result<Vec<DbObjectInfo>, String> {
    let entry = crate::plugins::database::catalog::scoped_session(
        &app,
        &state,
        &secrets_state,
        &conn_id,
        database.as_deref(),
    )
    .await?;
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let rows = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?
                .exec(dialect.objects_sql(), (database.clone(),))
                .await
                .map_err(|e| format!("对象列表失败: {e}"))?;
            Ok(rows
                .into_iter()
                .map(|row| DbObjectInfo {
                    kind: drivers::mysql::mysql_str(&row, 1),
                    name: drivers::mysql::mysql_str(&row, 0),
                })
                .collect())
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let rows = client
                .query(dialect.objects_sql(), &[&schema])
                .await
                .map_err(|e| format!("对象列表失败: {e}"))?;
            Ok(rows
                .iter()
                .map(|row| DbObjectInfo {
                    kind: row.try_get::<usize, String>(1).unwrap_or_default(),
                    name: row.try_get::<usize, String>(0).unwrap_or_default(),
                })
                .collect())
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let dialect = dialect_or_err(crate::plugins::database::models::DbType::Sqlite)?;
            let mut stmt = guard
                .prepare(dialect.objects_sql())
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(DbObjectInfo {
                        kind: row.get::<_, String>(1)?,
                        name: row.get::<_, String>(0)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())
        }
        DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Agent { client, session_id } => {
            let schema = schema.unwrap_or_default();
            let objects = client.list_objects(session_id, &schema).await?;
            Ok(objects
                .into_iter()
                .map(|(kind, name)| DbObjectInfo { kind, name })
                .collect())
        }
    }
}

/// 表结构列信息
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_columns(
    app: tauri::AppHandle,
    secrets_state: State<'_, crate::plugins::database::secrets::SecretsState>,
    database: Option<String>,
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<DbColumnInfo>, String> {
    let entry = crate::plugins::database::catalog::scoped_session(
        &app,
        &state,
        &secrets_state,
        &conn_id,
        database.as_deref(),
    )
    .await?;
    columns_for_entry(&entry, schema, table).await
}

/// 供表浏览与数据修改共用的真实列元数据，避免重复拼写列身份。
pub(crate) async fn columns_for_entry(
    entry: &drivers::DbSessionEntry,
    schema: Option<String>,
    table: String,
) -> Result<Vec<DbColumnInfo>, String> {
    match &entry.session {
        DbSession::Mysql(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let database = schema.unwrap_or_else(|| entry.config.database.clone());
            let rows = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?
                .exec(dialect.columns_sql(), (database.clone(), table.clone()))
                .await
                .map_err(|e| format!("列信息失败: {e}"))?;
            Ok(rows
                .into_iter()
                .map(|row| DbColumnInfo {
                    name: drivers::mysql::mysql_str(&row, 0),
                    data_type: drivers::mysql::mysql_str(&row, 1),
                    nullable: if drivers::mysql::mysql_str(&row, 2) == "NO" {
                        "否"
                    } else {
                        "是"
                    }
                    .to_string(),
                    default_value: drivers::mysql::mysql_str(&row, 3),
                    key: drivers::mysql::mysql_key(&row, 5),
                    comment: drivers::mysql::mysql_str(&row, 4),
                })
                .collect())
        }
        DbSession::Postgres(pool) => {
            let dialect = dialect_or_err(entry.config.db_type)?;
            let schema = schema.unwrap_or_else(|| "public".to_string());
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let rows = client
                .query(dialect.columns_sql(), &[&schema, &table])
                .await
                .map_err(|e| format!("列信息失败: {e}"))?;
            // 键标记探测（PK/UK）
            let keys = drivers::postgres::pg_keys(&client, &schema, &table).await?;
            Ok(rows
                .iter()
                .map(|row| {
                    let name: String = row.try_get::<usize, String>(0).unwrap_or_default();
                    DbColumnInfo {
                        name: name.clone(),
                        data_type: row.try_get::<usize, String>(1).unwrap_or_default(),
                        nullable: row.try_get::<usize, String>(2).unwrap_or_default(),
                        default_value: row.try_get::<usize, String>(3).unwrap_or_default(),
                        key: keys.get(&name).cloned().unwrap_or_else(|| "—".to_string()),
                        comment: row.try_get::<usize, String>(4).unwrap_or_default(),
                    }
                })
                .collect())
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let safe = table.replace('"', "\"\"");
            let mut stmt = guard
                .prepare(&format!("PRAGMA table_xinfo(\"{safe}\")"))
                .map_err(|e| format!("列信息失败: {e}"))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(DbColumnInfo {
                        name: row.get::<_, String>(1)?,
                        data_type: row.get::<_, String>(2)?,
                        nullable: if row.get::<_, i64>(3)? != 0 {
                            "否"
                        } else {
                            "是"
                        }
                        .to_string(),
                        default_value: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                        key: if row.get::<_, i64>(5)? > 0 {
                            "PK".to_string()
                        } else {
                            "—".to_string()
                        },
                        comment: String::new(),
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())
        }
        DbSession::Redis(_) => Ok(Vec::new()),
        DbSession::Agent { client, session_id } => {
            let schema = schema.unwrap_or_default();
            let value = client.get_columns(session_id, &schema, &table).await?;
            let items = value
                .as_array()
                .or_else(|| {
                    value
                        .get("columns")
                        .or_else(|| value.get("items"))
                        .and_then(|v| v.as_array())
                })
                .ok_or("DB_DRIVER_PROTOCOL: 列信息必须是数组")?;
            Ok(items
                .iter()
                .map(|item| DbColumnInfo {
                    name: item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    data_type: item
                        .get("data_type")
                        .or_else(|| item.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    nullable: item
                        .get("is_nullable")
                        .or_else(|| item.get("nullable"))
                        .and_then(|v| v.as_bool())
                        .map(|b| if b { "是" } else { "否" }.to_string())
                        .unwrap_or_default(),
                    default_value: item
                        .get("column_default")
                        .or_else(|| item.get("default"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    key: if item
                        .get("is_primary_key")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    {
                        "PK".into()
                    } else {
                        item.get("key")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string()
                    },
                    comment: item
                        .get("comment")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect())
        }
    }
}
