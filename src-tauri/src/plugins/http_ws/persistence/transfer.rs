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
        &[
            ("http_ws.requests", "HTTP、SSE 与 WebSocket 接口"),
            ("http_ws.layout", "接口分组与布局"),
        ]
    }
    fn singleton(&self, dataset: &str) -> bool {
        dataset == "http_ws.layout"
    }
    fn read(&self, conn: &Connection, dataset: &str) -> Result<Vec<Value>, String> {
        if dataset == "http_ws.layout" {
            let groups = records::query(conn, "SELECT json_object('path',path,'sortOrder',sort_order) FROM api_groups ORDER BY sort_order,path")?;
            return Ok(vec![
                serde_json::json!({"id":"layout","name":"接口分组与布局","groups":groups}),
            ]);
        }
        let mut records = records::query(conn, "SELECT json_object('id', uid, 'name', name, 'kind', type, 'method', method, 'url', url, 'params', params, 'headers', headers, 'bodyMode', body_mode, 'body', body, 'groupName', group_name, 'options', options, 'sortOrder', sort_order) FROM api_list ORDER BY sort_order,id")?;
        for record in &mut records {
            record["options"] = Value::String(portable_options(record)?);
        }
        Ok(records)
    }
    fn note(&self, dataset: &str, _: &Value) -> Option<String> {
        if dataset == "http_ws.layout" {
            return Some(
                "布局作为整体导入；使用导入版本将替换空分组及分组排序，已有接口所属路径仍保留。"
                    .into(),
            );
        }
        Some("认证方式随接口保留；凭证库引用不跨设备传输，导入后请重新选择凭证。".into())
    }
    fn validate(&self, dataset: &str, record: &Value) -> Result<(), String> {
        if dataset == "http_ws.layout" {
            return validate_layout(record);
        }
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
                "sortOrder",
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
        sort_order(record)?;
        portable_options(record)?;
        Ok(())
    }
    fn write(&self, conn: &Connection, dataset: &str, record: &Value) -> Result<(), String> {
        if dataset == "http_ws.layout" {
            validate_layout(record)?;
            conn.execute("DELETE FROM api_groups", [])
                .map_err(|e| e.to_string())?;
            for group in record["groups"].as_array().ok_or("分组列表无效")? {
                let path = records::string(group, "path")?;
                super::ensure_group_paths(conn, path)?;
                conn.execute(
                    "UPDATE api_groups SET sort_order=?2 WHERE path=?1",
                    rusqlite::params![path, sort_order(group)?],
                )
                .map_err(|e| e.to_string())?;
            }
            // 单独导入布局也不能使现有接口失去分组目录。
            let paths = records::query(conn, "SELECT json_object('path',group_name) FROM api_list WHERE group_name<>'' GROUP BY group_name")?;
            for path in paths {
                super::ensure_group_paths(conn, records::string(&path, "path")?)?;
            }
            return Ok(());
        }
        super::ensure_group_paths(
            conn,
            record
                .get("groupName")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )?;
        let mut record = record.clone();
        record["options"] = Value::String(portable_options(&record)?);
        conn.execute("INSERT INTO api_list(uid,name,type,method,url,params,headers,body_mode,body,group_name,options,updated_at) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.name'),json_extract(?1,'$.kind'),json_extract(?1,'$.method'),json_extract(?1,'$.url'),json_extract(?1,'$.params'),json_extract(?1,'$.headers'),json_extract(?1,'$.bodyMode'),json_extract(?1,'$.body'),coalesce(json_extract(?1,'$.groupName'),''),json_extract(?1,'$.options'),datetime('now')) ON CONFLICT(uid) DO UPDATE SET name=excluded.name,type=excluded.type,method=excluded.method,url=excluded.url,params=excluded.params,headers=excluded.headers,body_mode=excluded.body_mode,body=excluded.body,group_name=excluded.group_name,options=excluded.options,updated_at=excluded.updated_at", [record.to_string()]).map_err(|e| e.to_string())?;
        if record.get("sortOrder").is_some() {
            conn.execute(
                "UPDATE api_list SET sort_order=?2 WHERE uid=?1",
                rusqlite::params![records::string(&record, "id")?, sort_order(&record)?],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn clear(&self, conn: &Connection, dataset: &str) -> Result<(), String> {
        // 布局是固定身份的完整快照，由 write 原子替换；空块不隐式清除目录。
        if dataset == "http_ws.layout" {
            return Ok(());
        }
        conn.execute("DELETE FROM api_list", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

fn sort_order(record: &Value) -> Result<i64, String> {
    match record.get("sortOrder") {
        None => Ok(i64::from(i32::MAX)),
        Some(value) => value
            .as_i64()
            .filter(|n| *n >= 0)
            .ok_or_else(|| "排序值必须为非负整数".into()),
    }
}

fn validate_layout(record: &Value) -> Result<(), String> {
    records::fields(record, &["id", "name", "groups"])?;
    if records::string(record, "id")? != "layout" {
        return Err("布局身份无效".into());
    }
    records::string(record, "name")?;
    let groups = record["groups"].as_array().ok_or("分组列表必须为数组")?;
    let mut seen = std::collections::BTreeSet::new();
    for group in groups {
        records::fields(group, &["path", "sortOrder"])?;
        let path = records::string(group, "path")?;
        if path.is_empty() || !seen.insert(path) {
            return Err("分组路径不能为空或重复".into());
        }
        sort_order(group)?;
    }
    Ok(())
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
    fn database() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        conn
    }
    #[test]
    fn layout_and_requests_roundtrip_with_empty_groups_and_order() {
        let source = database();
        super::super::ensure_group_paths(&source, "空组/子组").unwrap();
        source
            .execute(
                "UPDATE api_groups SET sort_order=7 WHERE path='空组/子组'",
                [],
            )
            .unwrap();
        source.execute("INSERT INTO api_list(uid,name,method,url,group_name,updated_at,sort_order) VALUES('a','接口','GET','https://example.invalid','业务','',3)", []).unwrap();
        super::super::ensure_group_paths(&source, "业务").unwrap();
        let mut target = database();
        let tx = target.transaction().unwrap();
        for (dataset, _) in Requests.datasets() {
            let values = Requests.read(&source, dataset).unwrap();
            for record in &values {
                Requests.validate(dataset, record).unwrap();
                Requests.write(&tx, dataset, record).unwrap();
            }
            assert_eq!(Requests.read(&tx, dataset).unwrap(), values);
        }
        tx.commit().unwrap();
        for (dataset, _) in Requests.datasets() {
            assert_eq!(
                Requests.read(&source, dataset).unwrap(),
                Requests.read(&target, dataset).unwrap()
            );
        }
        let layout = serde_json::json!({"id":"layout","name":"布局","groups":[]});
        Requests.write(&target, "http_ws.layout", &layout).unwrap();
        let remaining = Requests.read(&target, "http_ws.layout").unwrap();
        assert_eq!(remaining[0]["groups"].as_array().unwrap().len(), 1);
        assert_eq!(remaining[0]["groups"][0]["path"], "业务");
    }
    #[test]
    fn invalid_layout_and_sort_are_rejected_and_old_sort_is_retained() {
        for groups in [
            serde_json::json!([{"path":"a"},{"path":"a"}]),
            serde_json::json!([{"path":"a","sortOrder":-1}]),
        ] {
            assert!(Requests
                .validate(
                    "http_ws.layout",
                    &serde_json::json!({"id":"layout","name":"布局","groups":groups})
                )
                .is_err());
        }
        let conn = database();
        let mut old = serde_json::json!({"id":"a","name":"接口","kind":"http","method":"GET","url":"","params":"[]","headers":"[]","bodyMode":"none","body":""});
        Requests.write(&conn, "http_ws.requests", &old).unwrap();
        conn.execute("UPDATE api_list SET sort_order=2", [])
            .unwrap();
        Requests.write(&conn, "http_ws.requests", &old).unwrap();
        assert_eq!(
            Requests.read(&conn, "http_ws.requests").unwrap()[0]["sortOrder"],
            2
        );
        old["sortOrder"] = serde_json::json!("2");
        assert!(Requests.validate("http_ws.requests", &old).is_err());
    }
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
        record["sortOrder"] = serde_json::json!(2147483647);
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
