//! SQLite 数据库插件 · 数据结构（serde，与前端 plugins/sqlite/contracts.ts 同步）
//! 本文件只声明类型，不包含任何逻辑；连接管理与 SQL 执行见 mod.rs。

use serde::Serialize;

/// 打开数据库的结果（成功时携带表列表，失败时 error 携带原因）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbOpenResult {
    /// 是否成功打开
    pub(crate) ok: bool,
    /// 库内全部表名（成功时返回，供前端左侧表列表渲染）
    pub(crate) tables: Vec<String>,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// SQL 执行结果：查询返回表格，非查询返回影响行数（is_query 区分两种形态）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryResult {
    /// 是否执行成功
    pub(crate) ok: bool,
    /// 列名（查询语句时非空）
    pub(crate) columns: Vec<String>,
    /// 数据行（全部单元格转字符串；NULL/BLOB 由 cell_str 特殊标注）
    pub(crate) rows: Vec<Vec<String>>,
    /// 受影响行数（非查询语句时有效）
    pub(crate) rows_affected: u64,
    /// true = 查询语句（返回表格），false = 写语句（返回影响行数）
    pub(crate) is_query: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
