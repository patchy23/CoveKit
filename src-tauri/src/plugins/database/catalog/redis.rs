//! 数据库查询与目录 · redis

use super::session;
use crate::plugins::database::drivers::redis;
use crate::plugins::database::drivers::DbSession;
use crate::plugins::database::drivers::DbState;
use tauri::State;

/// Redis 键列表（SCAN 游标）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_keys(
    state: State<'_, DbState>,
    conn_id: String,
    pattern: String,
    cursor: u64,
    database: Option<String>,
) -> Result<(u64, Vec<String>), String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(redis_session) = &entry.session else {
        // 理论不变量（上方已校验 is_redis）；不 panic，类型错配时显式报错
        return Err("连接会话与库类型不一致，请断开重连".to_string());
    };
    let database = database
        .as_deref()
        .map(crate::plugins::database::drivers::redis::parse_database_index)
        .transpose()?
        .unwrap_or_else(|| redis_session.default_database());
    let mut mgr = redis_session.manager(database).await?;
    redis::scan_keys(&mut mgr, &pattern, cursor, 200).await
}

/// Redis 逻辑库列表；warning 说明清单或键数统计受到限制。
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_databases(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<crate::plugins::database::models::RedisDatabaseList, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(redis_session) = &entry.session else {
        return Err("连接会话与库类型不一致，请断开重连".to_string());
    };
    redis::database_list(redis_session).await
}

/// Redis 键信息（TYPE/TTL/值预览）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_key_info(
    state: State<'_, DbState>,
    conn_id: String,
    key: String,
    database: Option<String>,
) -> Result<crate::plugins::database::models::RedisKeyInfo, String> {
    let _storage_operation = crate::framework::storage::access::operation()?;
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(redis_session) = &entry.session else {
        // 理论不变量（上方已校验 is_redis）；不 panic，类型错配时显式报错
        return Err("连接会话与库类型不一致，请断开重连".to_string());
    };
    let database = database
        .as_deref()
        .map(crate::plugins::database::drivers::redis::parse_database_index)
        .transpose()?
        .unwrap_or_else(|| redis_session.default_database());
    let mut mgr = redis_session.manager(database).await?;
    redis::key_info(&mut mgr, &key).await
}
