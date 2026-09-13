//! SSH 插件 · 凭证引用自报（可靠性 T09）
//!
//! 只读扫描 `ssh_profiles.credential_ref`：框架删除凭证前据此判断还有哪些服务器在用，
//! 旧 data/ssh.db 与 `data/ssh.db` 路径解析由 `plugin_db_path` 统一处理（含旧布局回落）。
//! 库不可读或列不认识时返回错误（记为计数未知），不能返回 0 条。
use rusqlite::{Connection, OpenFlags};
use tauri::AppHandle;

use crate::framework::credential_refs::{CredentialReference, CredentialReferenceProvider};
use crate::framework::store::plugin_db_path;

/// 本插件的引用提供者（进程级单例，装配阶段登记）
struct SshCredentialRefs;

impl CredentialReferenceProvider for SshCredentialRefs {
    fn owner(&self) -> &'static str {
        "ssh"
    }

    fn scan(&self, app: &AppHandle) -> Result<Vec<CredentialReference>, String> {
        let path = plugin_db_path(app, "ssh")?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("SSH 数据库不可读: {e}"))?;
        scan_connections(&conn)
    }
}

/// 在已打开的连接上扫描引用（独立函数便于用内存库覆盖旧表结构）
fn scan_connections(conn: &Connection) -> Result<Vec<CredentialReference>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, name, credential_ref FROM ssh_profiles \
             WHERE credential_ref IS NOT NULL AND credential_ref <> ''",
        )
        .map_err(|e| format!("SSH 服务器表结构不认识（缺少 credential_ref 列）: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            let object_id: String = row.get(0)?;
            let object_name: String = row.get(1)?;
            let credential_id: String = row.get(2)?;
            Ok(CredentialReference {
                owner: "ssh".to_string(),
                credential_id,
                object_id,
                object_name,
            })
        })
        .map_err(|e| format!("SSH 引用扫描失败: {e}"))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(row.map_err(|e| format!("SSH 引用读取失败: {e}"))?);
    }
    Ok(found)
}

/// 装配阶段登记（重复调用幂等）
pub fn register_provider() {
    static PROVIDER: SshCredentialRefs = SshCredentialRefs;
    crate::framework::credential_refs::register(&PROVIDER);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一凭证被多个服务器引用时逐条列出，未引用凭证的服务器不出现
    #[test]
    fn scan_lists_referencing_profiles_only() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE ssh_profiles (id TEXT PRIMARY KEY, name TEXT, credential_ref TEXT);
             INSERT INTO ssh_profiles VALUES ('p1', '生产机', 'cred-1');
             INSERT INTO ssh_profiles VALUES ('p2', '测试机', 'cred-1');
             INSERT INTO ssh_profiles VALUES ('p3', '本地机', NULL);",
        )
        .unwrap();
        let found = scan_connections(&conn).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|item| item.credential_id == "cred-1"));
        assert!(found.iter().any(|item| item.object_name == "生产机"));
    }

    /// 旧表没有 credential_ref 列：报错（未知），不能当成无引用
    #[test]
    fn legacy_schema_without_column_is_unknown() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE ssh_profiles (id TEXT PRIMARY KEY, name TEXT);")
            .unwrap();
        let error = scan_connections(&conn).unwrap_err();
        assert!(error.contains("缺少 credential_ref 列"), "{error}");
    }
}
