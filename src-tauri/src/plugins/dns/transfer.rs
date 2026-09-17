//! DNS 单槽配置传输；手工配置属于普通数据，Vault 引用必须在目标空间重新配置。

use crate::framework::data_transfer::{
    adapter,
    records::{self, RecordStore, RecordsAdapter},
};
use rusqlite::Connection;
use serde_json::Value;

struct Providers;
impl RecordStore for Providers {
    fn owner(&self) -> &'static str {
        "dns"
    }
    fn storage(&self) -> &'static str {
        "dns"
    }
    fn migrations(&self) -> &'static [&'static str] {
        super::MIGRATIONS
    }
    fn datasets(&self) -> &'static [(&'static str, &'static str)] {
        &[("dns.providers", "DNS 平台配置")]
    }
    fn singleton(&self, _: &str) -> bool {
        true
    }
    fn read(&self, conn: &Connection, _: &str) -> Result<Vec<Value>, String> {
        records::query(conn, "SELECT json_object('id', platform, 'name', platform, 'account', CASE WHEN coalesce(credential_ref,'')='' THEN id ELSE '' END, 'key', CASE WHEN coalesce(credential_ref,'')='' THEN key ELSE '' END, 'needsCredential', json(CASE WHEN coalesce(credential_ref,'')='' AND credential_pending=0 THEN 'false' ELSE 'true' END)) FROM dns_config ORDER BY platform")
    }
    fn validate(&self, _: &str, record: &Value) -> Result<(), String> {
        records::fields(record, &["id", "name", "account", "key", "needsCredential"])?;
        if !matches!(
            records::string(record, "id")?,
            "aliyun" | "dnspod" | "cloudflare"
        ) {
            return Err("不支持的 DNS 平台".into());
        }
        records::string(record, "account")?;
        records::string(record, "key")?;
        if records::string(record, "name")? != records::string(record, "id")? {
            return Err("DNS 平台名称与身份不一致".into());
        }
        if record["needsCredential"] == true
            && (!records::string(record, "account")?.is_empty()
                || !records::string(record, "key")?.is_empty())
        {
            return Err("待补录的 DNS 凭证不能夹带手工配置".into());
        }
        if !record.get("needsCredential").is_some_and(Value::is_boolean) {
            return Err("DNS 凭证补录标记无效".into());
        }
        Ok(())
    }
    fn note(&self, _: &str, record: &Value) -> Option<String> {
        (record["needsCredential"] == true)
            .then(|| "Vault 凭证不随包传输，请在目标空间重新选择凭证".into())
    }
    fn write(&self, conn: &Connection, _: &str, record: &Value) -> Result<(), String> {
        let pending = record["needsCredential"] == true;
        conn.execute("INSERT INTO dns_config(platform,id,key,credential_ref,credential_pending) VALUES(?1,?2,?3,NULL,?4) ON CONFLICT(platform) DO UPDATE SET id=excluded.id,key=excluded.key,credential_ref=NULL,credential_pending=excluded.credential_pending", rusqlite::params![records::string(record,"id")?, if pending { "" } else { records::string(record,"account")? }, if pending { "" } else { records::string(record,"key")? }, pending]).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn clear(&self, conn: &Connection, _: &str) -> Result<(), String> {
        conn.execute("DELETE FROM dns_config", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// 注册 DNS 逻辑适配器。
pub(super) fn register() {
    static ADAPTER: RecordsAdapter<Providers> = RecordsAdapter(Providers);
    adapter::register(&ADAPTER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vault_reference_excludes_fallback_secret_but_manual_configuration_roundtrips() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::MIGRATIONS).unwrap();
        conn.execute("INSERT INTO dns_config(platform,id,key,credential_ref) VALUES('aliyun','synthetic-id','synthetic-key','vault-id')", []).unwrap();
        let record = Providers.read(&conn, "dns.providers").unwrap().remove(0);
        assert_eq!(record["key"], "");
        assert_eq!(record["needsCredential"], true);
        Providers.write(&conn, "dns.providers", &record).unwrap();
        let reference: Option<String> = conn
            .query_row("SELECT credential_ref FROM dns_config", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(reference.is_none());
        let manual = serde_json::json!({"id":"dnspod","name":"dnspod","account":"manual","key":"synthetic","needsCredential":false});
        Providers.write(&conn, "dns.providers", &manual).unwrap();
        assert!(Providers
            .read(&conn, "dns.providers")
            .unwrap()
            .contains(&manual));
    }
}
