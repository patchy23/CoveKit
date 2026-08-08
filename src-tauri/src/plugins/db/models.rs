//! SQLite 数据库插件 · 数据结构（serde，与前端 plugins/sqlite/contracts.ts 同步）

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbOpenResult {
    pub(crate) ok: bool,
    pub(crate) tables: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryResult {
    pub(crate) ok: bool,
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) rows_affected: u64,
    pub(crate) is_query: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
