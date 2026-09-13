//! DNS 插件 · 凭证引用自报（可靠性 T09）
//!
//! 只读扫描 `dns_config.credential_ref`：框架删除凭证前据此判断还有哪些平台配置在用，
//! 不需要框架去写 DNS 的业务 SQL。库不可读或表结构不认识时返回错误（记为计数未知），
//! **不能返回 0 条**——那会诱导用户删掉仍在使用的凭证。
use rusqlite::{Connection, OpenFlags};
use tauri::AppHandle;

use crate::framework::credential_refs::{CredentialReference, CredentialReferenceProvider};
use crate::framework::store::plugin_db_path;

/// 本插件的引用提供者（进程级单例，装配阶段登记）
struct DnsCredentialRefs;

impl CredentialReferenceProvider for DnsCredentialRefs {
    fn owner(&self) -> &'static str {
        "dns"
    }

    fn scan(&self, app: &AppHandle) -> Result<Vec<CredentialReference>, String> {
        let path = plugin_db_path(app, "dns")?;
        // 文件不存在 = 插件从未保存过配置，确实没有引用
        if !path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("DNS 数据库不可读: {e}"))?;
        scan_connections(&conn)
    }
}

/// 在已打开的连接上扫描引用（独立函数便于用内存库覆盖旧表结构）
fn scan_connections(conn: &Connection) -> Result<Vec<CredentialReference>, String> {
    let mut statement = conn
        .prepare(
            "SELECT platform, credential_ref FROM dns_config \
             WHERE credential_ref IS NOT NULL AND credential_ref <> ''",
        )
        .map_err(|e| format!("DNS 配置表结构不认识（缺少 credential_ref 列）: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            let platform: String = row.get(0)?;
            let credential_id: String = row.get(1)?;
            Ok(CredentialReference {
                owner: "dns".to_string(),
                credential_id,
                object_id: platform.clone(),
                object_name: platform,
            })
        })
        .map_err(|e| format!("DNS 引用扫描失败: {e}"))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(row.map_err(|e| format!("DNS 引用读取失败: {e}"))?);
    }
    Ok(found)
}

/// 装配阶段登记（重复调用幂等）
pub fn register_provider() {
    static PROVIDER: DnsCredentialRefs = DnsCredentialRefs;
    crate::framework::credential_refs::register(&PROVIDER);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 多条配置引用同一凭证都要逐条列出（计数由框架聚合）
    #[test]
    fn scan_lists_every_referencing_platform() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dns_config (platform TEXT PRIMARY KEY, id TEXT, key TEXT, credential_ref TEXT);
             INSERT INTO dns_config VALUES ('alidns', '', '', 'cred-1');
             INSERT INTO dns_config VALUES ('dnspod', '', '', 'cred-1');
             INSERT INTO dns_config VALUES ('cloudflare', '', '', NULL);",
        )
        .unwrap();
        let mut found = scan_connections(&conn).unwrap();
        found.sort_by(|a, b| a.object_id.cmp(&b.object_id));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].credential_id, "cred-1");
        assert_eq!(found[0].object_name, "alidns");
        assert!(found.iter().all(|item| item.owner == "dns"));
    }

    /// 旧表没有 credential_ref 列：必须报错（记为未知），不能当成 0 条引用
    #[test]
    fn legacy_schema_without_column_is_unknown() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dns_config (platform TEXT PRIMARY KEY, id TEXT, key TEXT);",
        )
        .unwrap();
        let error = scan_connections(&conn).unwrap_err();
        assert!(error.contains("缺少 credential_ref 列"), "{error}");
    }

    /// 没有引用时返回空清单（成功且 0 条）
    #[test]
    fn empty_table_scans_as_ok() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE dns_config (platform TEXT PRIMARY KEY, id TEXT, key TEXT, credential_ref TEXT);",
        )
        .unwrap();
        assert!(scan_connections(&conn).unwrap().is_empty());
    }
}
