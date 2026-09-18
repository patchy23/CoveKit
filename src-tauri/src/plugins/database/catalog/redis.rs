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
) -> Result<(u64, Vec<String>), String> {
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(mgr) = &entry.session else {
        // 理论不变量（上方已校验 is_redis）；不 panic，类型错配时显式报错
        return Err("连接会话与库类型不一致，请断开重连".to_string());
    };
    let mut mgr = mgr.clone();
    redis::scan_keys(&mut mgr, &pattern, cursor, 200).await
}

/// Redis 键信息（TYPE/TTL/值预览）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_redis_key_info(
    state: State<'_, DbState>,
    conn_id: String,
    key: String,
) -> Result<crate::plugins::database::models::RedisKeyInfo, String> {
    let entry = session(&state, &conn_id)?;
    if !entry.config.db_type.is_redis() {
        return Err("当前连接不是 Redis".to_string());
    }
    let DbSession::Redis(mgr) = &entry.session else {
        // 理论不变量（上方已校验 is_redis）；不 panic，类型错配时显式报错
        return Err("连接会话与库类型不一致，请断开重连".to_string());
    };
    let mut mgr = mgr.clone();
    redis::key_info(&mut mgr, &key).await
}
