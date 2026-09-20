//! 侧车结果协议验证与类型恢复；每条语句保留结果，脚本累计使用统一预算。
use crate::plugins::database::{
    agent::AgentClient,
    models::{DbType, DbValue, QueryResult},
    results::ResultBudget,
    sql_analysis,
};

/// 执行完整脚本，Oracle PL/SQL 块由方言拆分器保持完整。
pub(crate) async fn execute_agent(
    client: &AgentClient,
    session_id: &str,
    sql: &str,
    max_rows: u64,
    kind: DbType,
) -> Result<QueryResult, String> {
    let mut results = Vec::new();
    let mut budget = ResultBudget::new(max_rows);
    for statement in sql_analysis::split(kind, sql)? {
        match client
            .execute_query(session_id, &statement, max_rows)
            .await
            .and_then(|value| decode(value, &mut budget))
        {
            Ok(result) => results.push(result),
            Err(error) => {
                results.push(QueryResult::failed(error));
                break;
            }
        }
    }
    Ok(QueryResult::script(results))
}
fn decode(value: serde_json::Value, budget: &mut ResultBudget) -> Result<QueryResult, String> {
    let columns = value
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or("DB_AGENT_PROTOCOL: 缺少 columns 数组")?;
    let rows = value
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or("DB_AGENT_PROTOCOL: 缺少 rows 数组")?;
    let mut result = QueryResult::empty();
    result.columns = columns
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or("DB_AGENT_PROTOCOL: 列名不是字符串")
        })
        .collect::<Result<_, _>>()?;
    result.column_types = value
        .get("column_types")
        .and_then(|v| v.as_array())
        .map(|types| {
            types
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    result.is_query = !result.columns.is_empty();
    result.rows_affected = value
        .get("affected_rows")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    result.truncated = value
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    for row in rows {
        let row = row.as_array().ok_or("DB_AGENT_PROTOCOL: 结果行不是数组")?;
        if row.len() != result.columns.len() {
            return Err("DB_AGENT_PROTOCOL: 行列数不一致".into());
        }
        let mut cells = Vec::with_capacity(row.len());
        for (i, value) in row.iter().enumerate() {
            let native = result
                .column_types
                .get(i)
                .map(|s| s.to_uppercase())
                .unwrap_or_default();
            let cell = match value {
                serde_json::Value::Null => DbValue::null(),
                serde_json::Value::Bool(v) => DbValue::text("boolean", v.to_string()),
                serde_json::Value::Number(v) => DbValue::text(
                    if v.is_i64() || v.is_u64() {
                        "integer"
                    } else {
                        "decimal"
                    },
                    v.to_string(),
                ),
                serde_json::Value::String(text) => {
                    let kind = if ["RAW", "VARRAW", "LONG RAW", "LONGRAW", "BLOB", "BFILE"]
                        .contains(&native.as_str())
                    {
                        "binary"
                    } else if native.contains("NUMBER")
                        || native.contains("DECIMAL")
                        || native.contains("NUMERIC")
                    {
                        "decimal"
                    } else if native.contains("DATE") || native.contains("TIME") {
                        "temporal"
                    } else {
                        "text"
                    };
                    let text = if kind == "binary" {
                        let hex = text.strip_prefix("0x").unwrap_or(text);
                        hex::decode(hex).map_err(|_| "DB_AGENT_PROTOCOL: 二进制值不是十六进制")?;
                        hex.to_string()
                    } else {
                        text.clone()
                    };
                    DbValue::text(kind, text)
                }
                other => DbValue::text("json", other.to_string()),
            };
            cells.push(cell);
        }
        budget.push(&mut result, cells);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_contract_preserves_empty_headers_null_and_raw() {
        let mut budget = ResultBudget::new(10);
        let result = decode(serde_json::json!({"columns":["n","raw"],"column_types":["NUMBER","RAW"],"rows":[[null,"0x00ff"]]}), &mut budget).unwrap();
        assert_eq!(result.values[0][0].kind, "null");
        assert_eq!(result.values[0][1].value.as_deref(), Some("00ff"));
        assert!(decode(
            serde_json::json!({"columns":["x"],"rows":[[]]}),
            &mut budget
        )
        .is_err());
        let empty = decode(serde_json::json!({"columns":["x"],"rows":[]}), &mut budget).unwrap();
        assert!(empty.is_query);
        assert_eq!(empty.columns, ["x"]);
        assert!(decode(serde_json::json!({}), &mut budget).is_err());
    }
}
