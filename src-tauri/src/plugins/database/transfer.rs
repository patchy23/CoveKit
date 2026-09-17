//! 数据库连接、收藏 SQL 与显式查询历史传输；不连接远端、不执行包内 SQL。

use crate::framework::data_transfer::{
    adapter,
    records::{self, RecordStore, RecordsAdapter},
};
use rusqlite::Connection;
use serde_json::Value;

const CONNECTIONS: &str = "database.connections";
const SAVED: &str = "database.saved_sql";
const HISTORY: &str = "database.history";

struct DatabaseRecords;
impl RecordStore for DatabaseRecords {
    fn owner(&self) -> &'static str {
        "database"
    }
    fn storage(&self) -> &'static str {
        "database"
    }
    fn migrations(&self) -> &'static [&'static str] {
        super::store::MIGRATIONS
    }
    fn datasets(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (CONNECTIONS, "数据库连接"),
            (SAVED, "收藏 SQL"),
            (HISTORY, "查询历史"),
        ]
    }
    fn reference(&self, dataset: &str) -> Option<(&'static str, &'static str)> {
        (dataset == HISTORY).then_some(("connectionId", CONNECTIONS))
    }
    fn read(&self, conn: &Connection, dataset: &str) -> Result<Vec<Value>, String> {
        records::query(conn, match dataset {
            CONNECTIONS => "SELECT json_object('id',id,'label',label,'dbType',db_type,'host',CASE WHEN db_type='sqlite' THEN '' ELSE host END,'port',port,'username',username,'database',CASE WHEN db_type='sqlite' THEN '' ELSE database END,'env',env,'readonly',json(CASE WHEN readonly<>0 THEN 'true' ELSE 'false' END),'ssl',json(CASE WHEN ssl<>0 THEN 'true' ELSE 'false' END),'connectTimeoutMs',connect_timeout_ms) FROM connections ORDER BY sort_order,id",
            SAVED => "SELECT json_object('id',uid,'title',title,'sql',sql,'at',at) FROM saved_sql ORDER BY id",
            HISTORY => "SELECT json_object('id',h.uid,'connectionId',CASE WHEN c.id IS NULL THEN '' ELSE h.conn_id END,'sql',h.sql,'status',h.status,'durationMs',h.duration_ms,'at',h.at) FROM history h LEFT JOIN connections c ON c.id=h.conn_id ORDER BY h.id",
            _ => return Err("未知数据库数据集".into()),
        })
    }
    fn validate(&self, dataset: &str, record: &Value) -> Result<(), String> {
        records::fields(
            record,
            match dataset {
                CONNECTIONS => &[
                    "id",
                    "label",
                    "dbType",
                    "host",
                    "port",
                    "username",
                    "database",
                    "env",
                    "readonly",
                    "ssl",
                    "connectTimeoutMs",
                ],
                SAVED => &["id", "title", "sql", "at"],
                HISTORY => &["id", "connectionId", "sql", "status", "durationMs", "at"],
                _ => return Err("未知数据库数据集".into()),
            },
        )?;
        match dataset {
            CONNECTIONS => {
                let config: super::models::ConnConfig =
                    serde_json::from_value(record.clone()).map_err(|_| "数据库连接字段无效")?;
                if config.id.is_empty()
                    || config.label.trim().is_empty()
                    || config.connect_timeout_ms > 300_000
                {
                    return Err("数据库连接名称或超时无效".into());
                }
                if config.db_type.is_sqlite()
                    && (!config.host.is_empty() || !config.database.is_empty())
                {
                    return Err("SQLite 本机文件路径不能随包导入".into());
                }
            }
            SAVED => {
                for field in ["title", "sql", "at"] {
                    records::string(record, field)?;
                }
            }
            HISTORY => {
                for field in ["connectionId", "sql", "status", "at"] {
                    records::string(record, field)?;
                }
                if !record
                    .get("durationMs")
                    .and_then(Value::as_u64)
                    .is_some_and(|v| v <= i64::MAX as u64)
                {
                    return Err("查询耗时无效".into());
                }
            }
            _ => return Err("未知数据库数据集".into()),
        }
        Ok(())
    }
    fn note(&self, dataset: &str, record: &Value) -> Option<String> {
        (dataset == CONNECTIONS).then(|| {
            if record["dbType"] == "sqlite" {
                "本机 SQLite 路径需重新选择".into()
            } else {
                "数据库凭证不随包传输，请重新填写连接密码".into()
            }
        })
    }
    fn write(&self, conn: &Connection, dataset: &str, record: &Value) -> Result<(), String> {
        let sql = match dataset {
            CONNECTIONS => "INSERT INTO connections(id,label,db_type,host,port,username,database,env,readonly,ssl,connect_timeout_ms,credential_pending) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.label'),json_extract(?1,'$.dbType'),json_extract(?1,'$.host'),json_extract(?1,'$.port'),json_extract(?1,'$.username'),json_extract(?1,'$.database'),json_extract(?1,'$.env'),json_extract(?1,'$.readonly'),json_extract(?1,'$.ssl'),json_extract(?1,'$.connectTimeoutMs'),1) ON CONFLICT(id) DO UPDATE SET label=excluded.label,db_type=excluded.db_type,host=excluded.host,port=excluded.port,username=excluded.username,database=excluded.database,env=excluded.env,readonly=excluded.readonly,ssl=excluded.ssl,connect_timeout_ms=excluded.connect_timeout_ms,credential_pending=1",
            SAVED => "INSERT INTO saved_sql(uid,title,sql,at) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.title'),json_extract(?1,'$.sql'),json_extract(?1,'$.at')) ON CONFLICT(uid) DO UPDATE SET title=excluded.title,sql=excluded.sql,at=excluded.at",
            HISTORY => "INSERT INTO history(uid,conn_id,sql,status,duration_ms,at) VALUES(json_extract(?1,'$.id'),json_extract(?1,'$.connectionId'),json_extract(?1,'$.sql'),json_extract(?1,'$.status'),json_extract(?1,'$.durationMs'),json_extract(?1,'$.at')) ON CONFLICT(uid) DO UPDATE SET conn_id=excluded.conn_id,sql=excluded.sql,status=excluded.status,duration_ms=excluded.duration_ms,at=excluded.at",
            _ => return Err("未知数据库数据集".into()),
        };
        conn.execute(sql, [record.to_string()])
            .map_err(|e| e.to_string())?;
        if dataset == HISTORY {
            let reference = records::string(record, "connectionId")?;
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM connections WHERE id=?1)",
                    [reference],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !reference.is_empty() && !exists {
                return Err("历史记录引用的连接不存在".into());
            }
        }
        Ok(())
    }
    fn clear(&self, conn: &Connection, dataset: &str) -> Result<(), String> {
        let sql = match dataset {
            CONNECTIONS => "DELETE FROM connections",
            SAVED => "DELETE FROM saved_sql",
            HISTORY => "DELETE FROM history",
            _ => return Err("未知数据库数据集".into()),
        };
        conn.execute(sql, []).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// 装配数据库传输能力。
pub(super) fn register() {
    static ADAPTER: RecordsAdapter<DatabaseRecords> = RecordsAdapter(DatabaseRecords);
    adapter::register(&ADAPTER);
}

/// 导入连接必须重新录入密码，避免同 ID 自动使用本地旧凭证。
pub(super) fn credential_pending(app: &tauri::AppHandle, id: &str) -> Result<bool, String> {
    crate::framework::store::PluginDb::open(app, "database", super::store::MIGRATIONS)?.with_conn(
        |conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM connections WHERE id=?1 AND credential_pending<>0)",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
        },
    )
}

/// 用户保存密码后解除导入补录状态，不改变密文内容。
pub(super) fn credential_saved(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    crate::framework::store::PluginDb::open(app, "database", super::store::MIGRATIONS)?.with_conn(
        |conn| {
            conn.execute(
                "UPDATE connections SET credential_pending=0 WHERE id=?1",
                [id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_history_reference_rolls_back_the_owner_transaction() {
        let path = std::env::temp_dir().join(format!(
            "patchybox-database-transfer-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = crate::framework::store::PluginDb::open_at(&path, super::super::store::MIGRATIONS)
            .unwrap();
        let result = db.with_transaction(|conn| {
            DatabaseRecords.write(conn, SAVED, &serde_json::json!({"id":"saved","title":"新收藏","sql":"SELECT 1","at":""}))?;
            DatabaseRecords.write(conn, HISTORY, &serde_json::json!({"id":"history","connectionId":"missing","sql":"SELECT 1","status":"success","durationMs":0,"at":""}))
        });
        assert!(result.is_err());
        assert!(db
            .with_conn(|conn| DatabaseRecords.read(conn, SAVED))
            .unwrap()
            .is_empty());
        assert!(db
            .with_conn(|conn| DatabaseRecords.read(conn, HISTORY))
            .unwrap()
            .is_empty());
        drop(db);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn import_does_not_execute_sql_and_marks_credentials_pending() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::framework::store::migrate(&mut conn, super::super::store::MIGRATIONS).unwrap();
        let profile = serde_json::json!({"id":"conn","label":"测试","dbType":"mysql","host":"example.invalid","port":3306,"username":"demo","database":"demo","env":"开发","readonly":false,"ssl":false,"connectTimeoutMs":10000});
        DatabaseRecords.validate(CONNECTIONS, &profile).unwrap();
        DatabaseRecords.write(&conn, CONNECTIONS, &profile).unwrap();
        let sql =
            serde_json::json!({"id":"sql","title":"只存储","sql":"DROP TABLE connections","at":""});
        DatabaseRecords.write(&conn, SAVED, &sql).unwrap();
        assert_eq!(
            DatabaseRecords.read(&conn, CONNECTIONS).unwrap(),
            vec![profile]
        );
        assert_eq!(
            conn.query_row("SELECT credential_pending FROM connections", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(DatabaseRecords.read(&conn, SAVED).unwrap(), vec![sql]);
    }
}
