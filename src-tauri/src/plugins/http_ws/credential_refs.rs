//! 接口库向凭证框架自报引用，阻止删除仍被保存接口使用的凭证。
use crate::framework::{
    credential_refs::{CredentialReference, CredentialReferenceProvider},
    store::plugin_db_path,
};
use rusqlite::{Connection, OpenFlags};
use tauri::AppHandle;

struct ApiCredentialRefs;
impl CredentialReferenceProvider for ApiCredentialRefs {
    fn owner(&self) -> &'static str {
        "http_ws"
    }
    fn scan(&self, app: &AppHandle) -> Result<Vec<CredentialReference>, String> {
        let path = plugin_db_path(app, "api")?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
        scan(&conn)
    }
}

fn scan(conn: &Connection) -> Result<Vec<CredentialReference>, String> {
    // v1–v3 尚无认证引用能力，未迁移旧库不可能存在引用。
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version < 4 {
        return Ok(Vec::new());
    }
    let mut statement = conn
        .prepare("SELECT CAST(id AS TEXT), name, options FROM api_list")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    for row in rows {
        let (object_id, object_name, raw) = row.map_err(|e| e.to_string())?;
        let options: serde_json::Value =
            serde_json::from_str(&super::persistence::validate_options(&raw)?)
                .map_err(|e| e.to_string())?;
        if let Some(id) = options
            .pointer("/auth/credentialId")
            .and_then(|value| value.as_str())
            .filter(|id| !id.is_empty())
        {
            found.push(CredentialReference {
                owner: "http_ws".into(),
                credential_id: id.into(),
                object_id,
                object_name,
            });
        }
    }
    Ok(found)
}

/// 与 owner 一起装配，扫描只读且不隐式迁移用户数据。
pub(super) fn register_provider() {
    static PROVIDER: ApiCredentialRefs = ApiCredentialRefs;
    crate::framework::credential_refs::register(&PROVIDER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_schema_has_no_refs_but_corrupt_new_schema_is_error() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(scan(&conn).unwrap().is_empty());
        conn.execute_batch("PRAGMA user_version=4").unwrap();
        assert!(scan(&conn).is_err());
    }
}
