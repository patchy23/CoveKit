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
        &[("http_ws.requests", "HTTP 与 WebSocket 请求")]
    }
    fn read(&self, conn: &Connection, _: &str) -> Result<Vec<Value>, String> {
        records::query(conn, "SELECT json_object('id', uid, 'name', name, 'kind', type, 'method', method, 'url', url, 'params', params, 'headers', headers, 'bodyMode', body_mode, 'body', body) FROM api_list ORDER BY id")
    }
    fn validate(&self, _: &str, record: &Value) -> Result<(), String> {
        records::fields(
            record,
            &[
                "id", "name", "kind", "method", "url", "params", "headers", "bodyMode", "body",
            ],
        )?;
        for field in [
            "name", "kind", "method", "url", "params", "headers", "bodyMode", "body",
        ] {
            records::string(record, field)?;
        }
        if !matches!(records::string(record, "kind")?, "http" | "ws") {
            return Err("请求类型不支持".into());
        }
        for field in ["params", "headers"] {
            let value: Value = serde_json::from_str(records::string(record, field)?)
                .map_err(|_| format!("请求 {field} 不是有效 JSON"))?;
            if !value.is_array() {
                return Err(format!("请求 {field} 必须是数组"));
            }
        }
        Ok(())
    }
    fn write(&self, conn: &Connection, _: &str, record: &Value) -> Result<(), String> {
        conn.execute("INSERT INTO api_list(uid,name,type,method,url,params,headers,body_mode,body,updated_at) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.name'),json_extract(?1,'$.kind'),json_extract(?1,'$.method'),json_extract(?1,'$.url'),json_extract(?1,'$.params'),json_extract(?1,'$.headers'),json_extract(?1,'$.bodyMode'),json_extract(?1,'$.body'),datetime('now')) ON CONFLICT(uid) DO UPDATE SET name=excluded.name,type=excluded.type,method=excluded.method,url=excluded.url,params=excluded.params,headers=excluded.headers,body_mode=excluded.body_mode,body=excluded.body,updated_at=excluded.updated_at", [record.to_string()]).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn clear(&self, conn: &Connection, _: &str) -> Result<(), String> {
        conn.execute("DELETE FROM api_list", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
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
    fn request_roundtrip_preserves_raw_content_and_identity() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        let record = serde_json::json!({"id":"stable", "name":"示例", "kind":"http", "method":"POST", "url":"https://example.invalid", "params":"[]", "headers":"[{\"name\":\"Authorization\",\"value\":\"synthetic-token\"}]", "bodyMode":"raw", "body":"示例原文"});
        Requests.validate("http_ws.requests", &record).unwrap();
        Requests.write(&conn, "http_ws.requests", &record).unwrap();
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
