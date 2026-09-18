//! HTTP/WS 收藏请求的逻辑传输；不发送请求，不携带运行缓冲。

use crate::framework::data_transfer::{
    adapter,
    records::{self, RecordStore, RecordsAdapter},
};
use rusqlite::Connection;
use serde_json::Value;

struct Requests;
impl RecordStore for Requests {
    fn owner(&self) -> &'static str {
        "http_ws"
    }
    fn storage(&self) -> &'static str {
        super::STORAGE_KEY
    }
    fn migrations(&self) -> &'static [&'static str] {
        super::MIGRATIONS
    }
    fn datasets(&self) -> &'static [(&'static str, &'static str)] {
        &[("http_ws.requests", "HTTP、SSE 与 WebSocket 接口")]
    }
    fn read(&self, conn: &Connection, _: &str) -> Result<Vec<Value>, String> {
        let mut records = records::query(conn, "SELECT json_object('id', uid, 'name', name, 'kind', type, 'method', method, 'url', url, 'params', params, 'headers', headers, 'bodyMode', body_mode, 'body', body, 'groupName', group_name, 'options', options) FROM api_list ORDER BY id")?;
        for record in &mut records {
            record["options"] = Value::String(portable_options(record)?);
        }
        Ok(records)
    }
    fn note(&self, _: &str, _: &Value) -> Option<String> {
        Some("认证方式随接口保留；凭证库引用不跨设备传输，导入后请重新选择凭证。".into())
    }
    fn validate(&self, _: &str, record: &Value) -> Result<(), String> {
        records::fields(
            record,
            &[
                "id",
                "name",
                "kind",
                "method",
                "url",
                "params",
                "headers",
                "bodyMode",
                "body",
                "groupName",
                "options",
            ],
        )?;
        for field in [
            "name", "kind", "method", "url", "params", "headers", "bodyMode", "body",
        ] {
            records::string(record, field)?;
        }
        if !matches!(records::string(record, "kind")?, "http" | "sse" | "ws") {
            return Err("请求类型不支持".into());
        }
        for field in ["params", "headers"] {
            let value: Value = serde_json::from_str(records::string(record, field)?)
                .map_err(|_| format!("请求 {field} 不是有效 JSON"))?;
            if !value.is_array() {
                return Err(format!("请求 {field} 必须是数组"));
            }
        }
        if record.get("groupName").is_some() {
            records::string(record, "groupName")?;
        }
        portable_options(record)?;
        Ok(())
    }
    fn write(&self, conn: &Connection, _: &str, record: &Value) -> Result<(), String> {
        let mut record = record.clone();
        record["options"] = Value::String(portable_options(&record)?);
        conn.execute("INSERT INTO api_list(uid,name,type,method,url,params,headers,body_mode,body,group_name,options,updated_at) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.name'),json_extract(?1,'$.kind'),json_extract(?1,'$.method'),json_extract(?1,'$.url'),json_extract(?1,'$.params'),json_extract(?1,'$.headers'),json_extract(?1,'$.bodyMode'),json_extract(?1,'$.body'),coalesce(json_extract(?1,'$.groupName'),''),json_extract(?1,'$.options'),datetime('now')) ON CONFLICT(uid) DO UPDATE SET name=excluded.name,type=excluded.type,method=excluded.method,url=excluded.url,params=excluded.params,headers=excluded.headers,body_mode=excluded.body_mode,body=excluded.body,group_name=excluded.group_name,options=excluded.options,updated_at=excluded.updated_at", [record.to_string()]).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn clear(&self, conn: &Connection, _: &str) -> Result<(), String> {
        conn.execute("DELETE FROM api_list", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// 逻辑传输不导出本机凭证身份；旧包缺省设置仍可导入。
fn portable_options(record: &Value) -> Result<String, String> {
    let raw = match record.get("options") {
        Some(_) => records::string(record, "options")?,
        None => "{}",
    };
    let mut options: Value =
        serde_json::from_str(&super::validate_options(raw)?).map_err(|e| e.to_string())?;
    if let Some(auth) = options.get_mut("auth") {
        auth["credentialId"] = Value::String(String::new());
    }
    Ok(options.to_string())
}

/// 随 HTTP/WS owner 装配一次。
pub(super) fn register() {
    static ADAPTER: RecordsAdapter<Requests> = RecordsAdapter(Requests);
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_settings_survive_without_cross_device_credential_refs() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        let record = serde_json::json!({"id":"stream", "name":"事件流", "kind":"sse", "method":"POST", "url":"https://example.invalid", "params":"[]", "headers":"[]", "bodyMode":"json", "body":"{}", "groupName":"开发", "options":r#"{"timeoutMs":5000,"auth":{"mode":"bearer","credentialId":"local-only"}}"#});
        Requests.validate("http_ws.requests", &record).unwrap();
        Requests.write(&conn, "http_ws.requests", &record).unwrap();
        let exported = Requests.read(&conn, "http_ws.requests").unwrap();
        assert_eq!(exported[0]["kind"], "sse");
        assert_eq!(exported[0]["groupName"], "开发");
        let options: Value =
            serde_json::from_str(exported[0]["options"].as_str().unwrap()).unwrap();
        assert_eq!(options["timeoutMs"], 5000);
        assert_eq!(options["auth"]["credentialId"], "");
        assert!(super::super::validate_options(
            r#"{"auth":{"mode":"bearer","credentialId":"","secret":"test-only"}}"#
        )
        .is_err());
    }
    #[test]
    fn request_roundtrip_preserves_raw_content_and_identity() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        let mut record = serde_json::json!({"id":"stable", "name":"示例", "kind":"http", "method":"POST", "url":"https://example.invalid", "params":"[]", "headers":"[{\"name\":\"Authorization\",\"value\":\"synthetic-token\"}]", "bodyMode":"raw", "body":"示例原文"});
        Requests.validate("http_ws.requests", &record).unwrap();
        Requests.write(&conn, "http_ws.requests", &record).unwrap();
        record["groupName"] = Value::String(String::new());
        record["options"] = Value::String("{}".into());
        Requests.write(&conn, "http_ws.requests", &record).unwrap();
        assert_eq!(
            Requests.read(&conn, "http_ws.requests").unwrap(),
            vec![record]
        );
        conn.execute(
            "INSERT INTO api_list(name,method,url,updated_at) VALUES('新增','GET','', '')",
            [],
        )
        .unwrap();
        let uid: String = conn
            .query_row("SELECT uid FROM api_list WHERE name='新增'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!uid.is_empty());
    }
}
