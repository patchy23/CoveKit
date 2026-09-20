//! 数据库查询与目录 · mod

pub(crate) mod bound;
pub(crate) mod csv_import;
pub(crate) mod export;
pub(crate) mod metadata;
pub(crate) mod mutation;
pub(crate) mod query;
pub(crate) mod redis;
pub(crate) mod table;
pub(crate) mod table_sql;
use crate::plugins::database::drivers::DbSessionEntry;
use crate::plugins::database::drivers::DbState;
use tauri::State;

/// 取会话条目（连接不存在报错）
pub(super) fn session(state: &State<'_, DbState>, conn_id: &str) -> Result<DbSessionEntry, String> {
    state.entry(conn_id)
}

/// PostgreSQL 跨库元数据使用目标库的真实连接，不能仅改变前端标签。
pub(crate) async fn scoped_session(
    app: &tauri::AppHandle,
    state: &State<'_, DbState>,
    secrets: &State<'_, crate::plugins::database::secrets::SecretsState>,
    conn_id: &str,
    database: Option<&str>,
) -> Result<DbSessionEntry, String> {
    let mut entry = state.entry(conn_id)?;
    if entry.config.db_type == crate::plugins::database::models::DbType::Postgresql {
        if let Some(database) =
            database.filter(|name| !name.is_empty() && *name != entry.config.database)
        {
            entry.config.database = database.to_string();
            let password = crate::plugins::database::secrets::secret_get(app, secrets, conn_id)?;
            entry.session = crate::plugins::database::drivers::DbSession::Postgres(
                crate::plugins::database::drivers::postgres::pg_pool(&entry.config, &password)
                    .await?,
            );
        }
    }
    Ok(entry)
}

#[cfg(test)]
mod table_tests;
