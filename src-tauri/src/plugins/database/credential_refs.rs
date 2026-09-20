//! 数据库 插件 · 凭证引用自报（可靠性 T09）
//!
//! 只读扫描 `connections.credential_ref`：框架删除凭证前据此判断还有哪些服务器在用，
//! 数据库路径由 `plugin_db_path` 统一处理（含旧布局回落）。
//! 库不可读或列不认识时返回错误（记为计数未知），不能返回 0 条。
use rusqlite::{Connection, OpenFlags};
use tauri::AppHandle;

use crate::framework::credential_refs::{CredentialReference, CredentialReferenceProvider};
use crate::framework::store::plugin_db_path;

/// 本插件的引用提供者（进程级单例，装配阶段登记）
struct DatabaseCredentialRefs;

impl CredentialReferenceProvider for DatabaseCredentialRefs {
    fn owner(&self) -> &'static str {
        "database"
    }

    fn scan(&self, app: &AppHandle) -> Result<Vec<CredentialReference>, String> {
        let path = plugin_db_path(app, "database")?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("数据库连接存储不可读: {e}"))?;
        scan_connections(&conn)
    }
}

/// 在已打开的连接上扫描引用（独立函数便于用内存库覆盖旧表结构）
fn scan_connections(conn: &Connection) -> Result<Vec<CredentialReference>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, label, credential_ref FROM connections \
             WHERE credential_ref IS NOT NULL AND credential_ref <> ''",
        )
        .map_err(|e| format!("数据库连接表结构不认识（缺少 credential_ref 列）: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            let object_id: String = row.get(0)?;
            let object_name: String = row.get(1)?;
            let credential_id: String = row.get(2)?;
            Ok(CredentialReference {
                owner: "database".to_string(),
                credential_id,
                object_id,
                object_name,
            })
        })
        .map_err(|e| format!("数据库 引用扫描失败: {e}"))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(row.map_err(|e| format!("数据库 引用读取失败: {e}"))?);
    }
    Ok(found)
}

/// 装配阶段登记（重复调用幂等）
pub fn register_provider() {
    static PROVIDER: DatabaseCredentialRefs = DatabaseCredentialRefs;
    crate::framework::credential_refs::register(&PROVIDER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_are_visible_and_unknown_schema_is_not_empty() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE connections(id TEXT, label TEXT)")
            .unwrap();
        assert!(scan_connections(&conn).is_err());
        conn.execute_batch("ALTER TABLE connections ADD COLUMN credential_ref TEXT; INSERT INTO connections VALUES('a','测试连接','credential-a'); INSERT INTO connections VALUES('b','无密码',NULL);").unwrap();
        let refs = scan_connections(&conn).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].owner, "database");
        assert_eq!(refs[0].credential_id, "credential-a");
    }
}
