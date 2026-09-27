//! 查询结果保真和预算：展示字符串只用于兼容网格，原值用于复制、导出与编辑。
use super::models::{DbValue, QueryResult};
use std::io::Write;

/// JSON 编码沿用剩余结果预算；大字符串写入前拒绝，避免编码完才丢弃。
struct JsonBuffer {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl Write for JsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("查询结果预算不足"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 单次脚本累计保存预算；继续消费协议不再累计结果，避免连接带未读数据回池。
pub(crate) struct ResultBudget {
    rows: u64,
    bytes: usize,
    max_rows: u64,
}

/// 行转换沿用结果预算，超出后释放已转换列，避免先复制或十六进制展开整行再丢弃。
pub(crate) struct ResultRow {
    remaining: usize,
    values: Option<Vec<DbValue>>,
}

impl ResultRow {
    fn append(&mut self, length: usize, convert: impl FnOnce() -> DbValue) {
        let cost = length.saturating_mul(2).saturating_add(128);
        if cost > self.remaining {
            self.values = None;
        } else if let Some(values) = &mut self.values {
            self.remaining -= cost;
            values.push(convert());
        }
    }

    /// 借用驱动的文本，确认可保存后才复制。
    pub(crate) fn text(&mut self, kind: &str, text: &str) {
        self.append(text.len(), || DbValue::text(kind, text.to_string()));
    }

    /// 二进制预算按原有十六进制传输长度计算，不分配被丢弃的编码缓冲。
    pub(crate) fn binary(&mut self, bytes: &[u8]) {
        self.append(bytes.len().saturating_mul(2), || DbValue::binary(bytes));
    }

    /// 侧车的嵌套值按原 JSON 编码保真；超预算释放整行，后续小行仍可保留。
    pub(crate) fn json(&mut self, value: &serde_json::Value) -> Result<(), String> {
        if self.values.is_none() {
            return Ok(());
        }
        if self.remaining < 128 {
            self.values = None;
            return Ok(());
        }
        let mut buffer = JsonBuffer {
            bytes: Vec::new(),
            limit: (self.remaining - 128) / 2,
            exceeded: false,
        };
        if let Err(error) = serde_json::to_writer(&mut buffer, value) {
            if buffer.exceeded {
                self.values = None;
                return Ok(());
            }
            return Err(format!("查询结果 JSON 编码失败: {error}"));
        }
        let text = String::from_utf8(buffer.bytes)
            .map_err(|error| format!("查询结果 JSON 编码无效: {error}"))?;
        self.value(DbValue::text("json", text));
        Ok(())
    }

    /// 已拥有的小标量直接移动；大字段必须使用借用入口或预先核对长度。
    pub(crate) fn value(&mut self, value: DbValue) {
        self.append(value.value.as_ref().map_or(0, String::len), || value);
    }
}

impl ResultBudget {
    /// 行被拒绝不消耗预算，后续较小行仍按原有语义尝试保存。
    pub(crate) fn row(&self) -> ResultRow {
        ResultRow {
            remaining: (8 * 1024 * 1024usize).saturating_sub(self.bytes),
            values: (self.rows < self.max_rows).then(Vec::new),
        }
    }

    /// 消费惰性转换结果；拒绝的行仍由驱动继续读取协议和影响行数。
    pub(crate) fn finish_row(&mut self, result: &mut QueryResult, row: ResultRow) {
        match row.values {
            Some(values) => self.push(result, values),
            None => result.truncated = true,
        }
    }
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
            .fold(0usize, usize::saturating_add);
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
        Self::text("binary", hex::encode(bytes))
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
        if statements.len() <= 1 {
            return statements.pop().unwrap_or_else(Self::empty);
        }
        let last = statements.pop().unwrap_or_else(Self::empty);
        // 多语句只传一份行和原值；根级保留脚本汇总，最后结果仍完整保存在语句列表。
        let mut result = Self {
            ok: last.ok && statements.iter().all(|s| s.ok),
            truncated: last.truncated || statements.iter().any(|s| s.truncated),
            rows_affected: if last.is_query {
                last.rows_affected
            } else {
                last.rows_affected + statements.iter().map(|s| s.rows_affected).sum::<u64>()
            },
            is_query: last.is_query,
            error: last.error.clone(),
            duration_ms: last.duration_ms,
            transaction_active: last.transaction_active,
            statement_index: last.statement_index,
            edit_target: last.edit_target.clone(),
            display_statement: Some(statements.len()),
            ..Self::default()
        };
        statements.push(last);
        result.statements = statements;
        result
    }
    /// 测试读取展示数据，覆盖单语句与多语句的同一用户结果语义。
    #[cfg(test)]
    pub(crate) fn display_result(&self) -> &Self {
        self.display_statement
            .and_then(|index| self.statements.get(index))
            .unwrap_or(self)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_json_uses_existing_budget_without_changing_encoding() {
        let value = serde_json::json!({"text":"中文\n\u{0000}","array":[null,true,1.25,{"x":"\\\""}]});
        let expected = value.to_string();
        let mut row = ResultRow { remaining: expected.len() * 2 + 128, values: Some(Vec::new()) };
        row.json(&value).unwrap();
        let values = row.values.unwrap();
        assert_eq!(values[0].kind, "json");
        assert_eq!(values[0].value.as_deref(), Some(expected.as_str()));
        let mut row = ResultRow { remaining: expected.len() * 2 + 127, values: Some(Vec::new()) };
        row.json(&value).unwrap();
        assert!(row.values.is_none());
    }

    #[test]
    fn oversized_json_stops_before_copying_string_and_later_rows_survive() {
        let value = serde_json::json!({"x":"x".repeat(5 * 1024 * 1024)});
        let mut buffer = JsonBuffer { bytes: Vec::new(), limit: 64, exceeded: false };
        assert!(serde_json::to_writer(&mut buffer, &value).is_err());
        assert!(buffer.exceeded);
        assert!(buffer.bytes.len() <= 64);
        let mut budget = ResultBudget::new(10);
        let mut result = QueryResult::empty();
        let mut row = budget.row();
        row.text("text", "前一列也随整行释放");
        row.json(&value).unwrap();
        budget.finish_row(&mut result, row);
        assert!(result.truncated);
        assert!(result.values.is_empty());
        let mut row = budget.row();
        row.json(&serde_json::json!([1,2])).unwrap();
        budget.finish_row(&mut result, row);
        assert_eq!(result.rows, [vec!["[1,2]"]]);
    }

    #[test]
    fn rejected_rows_never_convert_and_do_not_exhaust_later_small_rows() {
        let mut budget = ResultBudget::new(2);
        let mut result = QueryResult::empty();
        let mut row = budget.row();
        row.append(usize::MAX, || panic!("超预算列不应转换"));
        row.append(1, || panic!("被拒绝行的后续列不应转换"));
        budget.finish_row(&mut result, row);
        assert!(result.truncated);
        assert!(result.values.is_empty());
        let mut row = budget.row();
        row.text("text", "仍可保存");
        row.binary(&[0x00, 0xff]);
        budget.finish_row(&mut result, row);
        assert_eq!(result.rows, [vec!["仍可保存", "0x00ff"]]);
    }

    #[test]
    fn exact_budget_and_row_limit_keep_original_boundary() {
        let mut budget = ResultBudget::new(1);
        let mut result = QueryResult::empty();
        let mut row = budget.row();
        row.text("text", &"x".repeat((8 * 1024 * 1024 - 128) / 2));
        budget.finish_row(&mut result, row);
        assert_eq!(result.values.len(), 1);
        assert!(!result.truncated);
        let mut row = budget.row();
        row.append(0, || panic!("行数耗尽后不应转换"));
        budget.finish_row(&mut result, row);
        assert!(result.truncated);
        assert_eq!(result.values.len(), 1);
    }

    #[test]
    fn script_transfers_final_rows_once_and_points_to_the_full_statement() {
        let mut last = QueryResult::empty();
        last.is_query = true;
        last.rows = vec![vec!["large cell".repeat(100_000)]];
        last.values = vec![vec![DbValue::text("text", "exact value".into())]];
        let rows = last.rows.as_ptr();
        let result = QueryResult::script(vec![QueryResult::empty(), last]);
        assert_eq!(result.display_statement, Some(1));
        assert!(result.rows.is_empty() && result.values.is_empty());
        assert_eq!(result.display_result().rows.as_ptr(), rows);
        let wire = serde_json::to_value(&result).unwrap();
        assert_eq!(wire["rows"], serde_json::json!([]));
        assert_eq!(wire["statements"][1]["values"][0][0]["value"], "exact value");
    }

    #[test]
    fn single_statement_moves_result_buffers_without_copying() {
        let mut statement = QueryResult::empty();
        statement.is_query = true;
        statement.rows = vec![vec!["large text".repeat(100_000)]];
        statement.values = vec![vec![DbValue::text("text", "precise value".into())]];
        let rows = statement.rows.as_ptr();
        let text = statement.rows[0][0].as_ptr();
        let values = statement.values.as_ptr();
        let result = QueryResult::script(vec![statement]);
        assert_eq!(result.rows.as_ptr(), rows);
        assert_eq!(result.rows[0][0].as_ptr(), text);
        assert_eq!(result.values.as_ptr(), values);
        assert!(result.statements.is_empty());
    }

    #[test]
    fn script_keeps_empty_single_failure_and_multiple_statement_semantics() {
        assert!(QueryResult::script(vec![]).ok);
        let failed = QueryResult::script(vec![QueryResult::failed("failure".into())]);
        assert!(!failed.ok);
        assert_eq!(failed.error.as_deref(), Some("failure"));
        let mut first = QueryResult::empty();
        first.rows_affected = 3;
        first.truncated = true;
        let mut second = QueryResult::failed("partial failure".into());
        second.rows_affected = 2;
        let result = QueryResult::script(vec![first, second]);
        assert!(!result.ok);
        assert!(result.truncated);
        assert_eq!(result.rows_affected, 5);
        assert_eq!(result.statements.len(), 2);
        assert_eq!(result.error.as_deref(), Some("partial failure"));
    }

    #[test]
    fn binary_encoding_preserves_every_byte_and_display_prefix() {
        let bytes: Vec<u8> = (0..=255).collect();
        let value = DbValue::binary(&bytes);
        let encoded = value.value.as_deref().unwrap();
        assert_eq!(encoded.len(), bytes.len() * 2);
        assert_eq!(hex::decode(encoded).unwrap(), bytes);
        assert!(encoded.starts_with("00010203"));
        assert_eq!(value.display(), format!("0x{encoded}"));
        assert_eq!(DbValue::binary(&[]).display(), "0x");
    }
}
