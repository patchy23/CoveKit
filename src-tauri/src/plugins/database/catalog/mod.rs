//! 数据库查询与目录 · mod

pub(crate) mod metadata;
pub(crate) mod query;
pub(crate) mod redis;
pub(crate) mod table;
use crate::plugins::database::drivers::DbSessionEntry;
use crate::plugins::database::drivers::DbState;
use tauri::State;

/// 默认查询行数上限（防止大表拖垮 UI）
pub(super) const DEFAULT_MAX_ROWS: u64 = 1000;

/// 取会话条目（连接不存在报错）
pub(super) fn session(state: &State<'_, DbState>, conn_id: &str) -> Result<DbSessionEntry, String> {
    state.entry(conn_id)
}
