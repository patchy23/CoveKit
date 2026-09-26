//! 新闻正文与阅读状态分开持久化；旧版整份缓存首次写入时在同一事务内迁移。
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

fn validate_preferences(value: &Value) -> Result<(), String> {
    let valid = value.as_object().is_some_and(|object| object.len() == 3)
        && value.get("auto").is_some_and(Value::is_boolean)
        && value.get("checkedAt").is_some_and(Value::is_string)
        && value
            .get("read")
            .and_then(Value::as_object)
            .is_some_and(|read| read.values().all(Value::is_string));
    if !valid {
        return Err("消息阅读状态格式无效".into());
    }
    Ok(())
}

fn split(mut value: Value) -> Result<(String, Value), String> {
    let object = value.as_object_mut().ok_or("消息缓存格式无效")?;
    let mut preferences = serde_json::Map::new();
    for key in ["read", "auto", "checkedAt"] {
        preferences.insert(
            key.into(),
            object.remove(key).ok_or("消息缓存缺少阅读状态")?,
        );
    }
    let preferences = Value::Object(preferences);
    validate_preferences(&preferences)?;
    Ok((
        serde_json::to_string(&value).map_err(|e| e.to_string())?,
        preferences,
    ))
}

fn raw(conn: &Connection, table: &str) -> Result<Option<String>, String> {
    // 表名只由本模块固定调用点传入，不接收 IPC 输入。
    conn.query_row(
        &format!("SELECT value FROM {table} WHERE id=1"),
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub(super) fn load(conn: &Connection) -> Result<Option<Value>, String> {
    let Some(content) = raw(conn, "news_state")? else {
        return Ok(None);
    };
    let mut value: Value =
        serde_json::from_str(&content).map_err(|e| format!("消息缓存损坏：{e}"))?;
    if let Some(preferences) = raw(conn, "news_preferences")? {
        let preferences: Value = serde_json::from_str(&preferences)
            .map_err(|e| format!("消息阅读状态损坏：{e}"))?;
        validate_preferences(&preferences)?;
        if let Value::Object(preferences) = preferences {
            value
                .as_object_mut()
                .ok_or("消息缓存格式无效")?
                .extend(preferences);
        }
    }
    Ok(Some(value))
}

/// 调用方持有写事务；状态写入既不读取也不序列化已分离的新闻正文。
pub(super) fn save(conn: &Connection, value: Value) -> Result<(), String> {
    let (content, preferences) = if value.get("snapshot").is_some() {
        let (content, preferences) = split(value)?;
        (Some(content), preferences)
    } else {
        validate_preferences(&value)?;
        let separated: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM news_preferences WHERE id=1)",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let content = if separated {
            None
        } else {
            let legacy = raw(conn, "news_state")?.ok_or("消息缓存尚未建立")?;
            let legacy =
                serde_json::from_str(&legacy).map_err(|e| format!("消息缓存损坏：{e}"))?;
            Some(split(legacy)?.0)
        };
        (content, value)
    };
    let preferences = serde_json::to_string(&preferences).map_err(|e| e.to_string())?;
    let content_bytes = match &content {
        Some(content) => content.len(),
        None => conn
            .query_row(
                "SELECT length(CAST(value AS BLOB)) FROM news_state WHERE id=1",
                [],
                |row| row.get::<_, usize>(0),
            )
            .map_err(|e| format!("读取消息缓存大小失败：{e}"))?,
    };
    // 两个对象合并少一对花括号、多一个逗号；沿用原完整缓存的 2 MiB 契约。
    if content_bytes
        .saturating_add(preferences.len())
        .saturating_sub(1)
        > super::MAX_BYTES
    {
        return Err("消息缓存格式无效或过大".into());
    }
    if let Some(content) = content {
        conn.execute("INSERT INTO news_state(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [content])
            .map_err(|e| e.to_string())?;
    }
    conn.execute("INSERT INTO news_preferences(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value", [preferences])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_read_and_metadata_only_updates_preserve_snapshot() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        let cache = serde_json::json!({"version":1,"snapshot":{"summary":"原文"},"read":{},"auto":true,"checkedAt":""});
        conn.execute("INSERT INTO news_state VALUES(1,?1)", [cache.to_string()]).unwrap();
        assert_eq!(load(&conn).unwrap(), Some(cache));
        let preferences = serde_json::json!({"read":{"one":"revision"},"auto":false,"checkedAt":"today"});
        save(&conn, preferences.clone()).unwrap();
        let content = raw(&conn, "news_state").unwrap();
        assert_eq!(load(&conn).unwrap().unwrap()["read"], preferences["read"]);
        // 后续状态写入不执行正文 UPDATE；用触发器证明，而非只比较最终文本。
        conn.execute_batch("CREATE TRIGGER prevent_news_rewrite BEFORE UPDATE ON news_state BEGIN SELECT RAISE(ABORT,'正文不应重写'); END;").unwrap();
        save(&conn, serde_json::json!({"read":{},"auto":true,"checkedAt":"next"})).unwrap();
        assert_eq!(raw(&conn, "news_state").unwrap(), content);
        assert_eq!(load(&conn).unwrap().unwrap()["snapshot"]["summary"], "原文");
    }

    #[test]
    fn invalid_metadata_does_not_replace_existing_cache() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        let cache = serde_json::json!({"version":1,"snapshot":null,"read":{},"auto":true,"checkedAt":""});
        save(&conn, cache.clone()).unwrap();
        assert!(save(&conn, serde_json::json!({"read":{},"auto":false})).is_err());
        let oversized = serde_json::json!({"read":{"one":"x".repeat(super::super::MAX_BYTES)},"auto":false,"checkedAt":""});
        assert!(save(&conn, oversized).is_err());
        assert_eq!(load(&conn).unwrap(), Some(cache));
    }
}
