//! 查询结果保真和预算：展示字符串只用于兼容网格，原值用于复制、导出与编辑。
use super::models::{DbValue, QueryResult};

/// 单次脚本累计保存预算；继续消费协议不再累计结果，避免连接带未读数据回池。
pub(crate) struct ResultBudget {
    rows: u64,
    bytes: usize,
    max_rows: u64,
}

impl ResultBudget {
    /// 行数限制同时作用于整个脚本，防止多结果规避预算。
    pub(crate) fn new(max_rows: u64) -> Self {
        Self {
            rows: 0,
            bytes: 0,
            max_rows: max_rows.clamp(1, 100_000),
        }
    }

    /// 保存一行，预算耗尽只标记截断，调用方仍负责收尾数据库协议。
    pub(crate) fn push(&mut self, result: &mut QueryResult, row: Vec<DbValue>) {
        let bytes = row
            .iter()
            .map(|v| {
                v.value
                    .as_ref()
                    .map_or(0, String::len)
                    .saturating_mul(2)
                    .saturating_add(128)
            })
            .sum::<usize>();
        if self.rows >= self.max_rows || self.bytes.saturating_add(bytes) > 8 * 1024 * 1024 {
            result.truncated = true;
            return;
        }
        self.rows += 1;
        self.bytes += bytes;
        result.rows.push(row.iter().map(DbValue::display).collect());
        result.values.push(row);
    }
}

impl DbValue {
    /// SQL NULL 不再与文本 NULL 混淆。
    pub(crate) fn null() -> Self {
        Self {
            kind: "null".into(),
            value: None,
        }
    }
    /// 数值与日期保留字符串精度，不经过 JavaScript Number。
    pub(crate) fn text(kind: &str, value: String) -> Self {
        Self {
            kind: kind.into(),
            value: Some(value),
        }
    }
    /// 二进制以十六进制传输，可逆且无需有损 UTF-8 转换。
    pub(crate) fn binary(bytes: &[u8]) -> Self {
        Self::text("binary", bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    /// 兼容网格展示；复制和持久化必须读取 value 与 kind。
    pub(crate) fn display(&self) -> String {
        match &self.value {
            None => "NULL".into(),
            Some(value) if self.kind == "binary" => format!("0x{value}"),
            Some(value) => value.clone(),
        }
    }
}

impl QueryResult {
    /// 初始化成功的空语句结果。
    pub(crate) fn empty() -> Self {
        Self {
            ok: true,
            ..Self::default()
        }
    }
    /// 保留脚本内已完成结果及失败语句，不把部分成功改写成全成功。
    pub(crate) fn script(mut statements: Vec<Self>) -> Self {
        let mut result = statements.last().cloned().unwrap_or_else(Self::empty);
        result.ok = statements.iter().all(|s| s.ok);
        result.truncated = statements.iter().any(|s| s.truncated);
        if !result.is_query {
            result.rows_affected = statements.iter().map(|s| s.rows_affected).sum();
        }
        if statements.len() > 1 {
            result.statements = std::mem::take(&mut statements);
        }
        result
    }
    /// 将驱动错误归属于当前语句；上层仍可展示之前已经完成的结果。
    pub(crate) fn failed(message: String) -> Self {
        Self {
            ok: false,
            error: Some(message),
            ..Self::default()
        }
    }
}
