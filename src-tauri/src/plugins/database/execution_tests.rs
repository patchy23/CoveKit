//! 数据库执行核心回归；联网用例显式 ignored，仅对调用者提供的专用环境执行。
#[cfg(test)]
mod tests {
    use super::super::{
        drivers,
        models::{ConnConfig, DbType},
        sql_analysis,
    };
    use mysql_async::prelude::Queryable;
    fn config(kind: DbType, host: &str, port: u16, username: &str, database: &str) -> ConnConfig {
        ConnConfig {
            credential_id: None,
            id: "isolated-test".into(),
            label: "隔离测试".into(),
            db_type: kind,
            host: host.into(),
            port,
            username: username.into(),
            database: database.into(),
            env: "测试".into(),
            readonly: false,
            ssl: false,
            connect_timeout_ms: 8000,
        }
    }
    #[test]
    fn source_ranges_preserve_comments_and_vendor_quotes() {
        let sql = "SELECT/*中文*/1; SELECT $$a;b$$; -- ;\nSELECT 'it''s; fine';";
        let parts = sql_analysis::split(DbType::Postgresql, sql).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "SELECT/*中文*/1");
        assert_eq!(parts[1], "SELECT $$a;b$$");
        assert!(parts[2].contains("-- ;\nSELECT"));
        assert_eq!(sql_analysis::split(DbType::Sqlite, "CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET v='a;b'; INSERT INTO t VALUES(2); END; SELECT 1;").unwrap().len(), 2);
    }
    #[test]
    fn risk_checks_cte_and_quoted_keywords() {
        assert_eq!(
            sql_analysis::assess(DbType::Postgresql, "SELECT 'DROP TABLE x'").unwrap(),
            (false, false)
        );
        assert_eq!(
            sql_analysis::assess(
                DbType::Postgresql,
                "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d"
            )
            .unwrap(),
            (true, true)
        );
        assert_eq!(
            sql_analysis::assess(DbType::Postgresql, "DELETE FROM t WHERE id=1").unwrap(),
            (true, false)
        );
        assert_eq!(
            sql_analysis::assess(DbType::Postgresql, "DELETE FROM t").unwrap(),
            (true, true)
        );
        assert_eq!(
            sql_analysis::assess(DbType::Postgresql, "BEGIN; COMMIT").unwrap(),
            (false, false)
        );
    }
    #[test]
    fn sqlite_results_preserve_null_binary_empty_columns_and_partial_success() {
        let conn = std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap());
        let result = drivers::sqlite::execute_sqlite(
            &conn,
            "SELECT NULL AS n, 'NULL' AS t, '' AS e, X'00ff' AS b, 9223372036854775807 AS i",
            10,
        )
        .unwrap();
        assert!(result.ok);
        assert_eq!(result.values[0][0].kind, "null");
        assert_eq!(result.values[0][1].value.as_deref(), Some("NULL"));
        assert_eq!(result.values[0][2].value.as_deref(), Some(""));
        assert_eq!(result.values[0][3].value.as_deref(), Some("00ff"));
        assert_eq!(
            result.values[0][4].value.as_deref(),
            Some("9223372036854775807")
        );
        let empty =
            drivers::sqlite::execute_sqlite(&conn, "SELECT 1 AS present WHERE 0", 10).unwrap();
        assert_eq!(empty.columns, ["present"]);
        let partial = drivers::sqlite::execute_sqlite(
            &conn,
            "CREATE TABLE t(id INTEGER); INSERT INTO t VALUES(1); SELECT * FROM missing",
            10,
        )
        .unwrap();
        assert!(!partial.ok);
        assert_eq!(partial.statements.len(), 3);
        assert!(partial.statements[0].ok && partial.statements[1].ok);
        let returning =
            drivers::sqlite::execute_sqlite(&conn, "INSERT INTO t VALUES (2) RETURNING id", 10)
                .unwrap();
        assert_eq!(returning.rows[0][0], "2");
    }

    #[tokio::test]
    #[ignore = "需要 COVEKIT_DB_TEST_CONFIG 专用环境，禁止默认访问真实数据库"]
    async fn dedicated_database_roundtrip_and_cancel() {
        let raw = std::env::var("COVEKIT_DB_TEST_CONFIG").expect("测试配置只由进程环境传入");
        let cfg: serde_json::Value = serde_json::from_str(&raw).expect("解析测试配置");
        let host = cfg["host"].as_str().unwrap();
        let unique = format!("covekit_{}", uuid::Uuid::new_v4().simple());
        let my = config(
            DbType::Mysql,
            host,
            cfg["mysql"]["port"].as_u64().unwrap().try_into().unwrap(),
            cfg["mysql"]["user"].as_str().unwrap(),
            "",
        );
        let pool = drivers::mysql::mysql_pool(&my, cfg["mysql"]["password"].as_str().unwrap())
            .await
            .unwrap();
        let mut conn = pool.get_conn().await.unwrap();
        let create = format!("CREATE DATABASE {unique}");
        conn.query_drop(&create).await.unwrap();
        conn.query_drop(format!("USE {unique}")).await.unwrap();
        if let Some(sql) = cfg["fixtures"]["mysql"].as_str() {
            let fixture = drivers::mysql::execute_mysql_conn(&mut conn, sql, 100)
                .await
                .unwrap();
            assert!(fixture.ok, "{:?}", fixture.error);
            assert_eq!(fixture.rows.len(), 10);
        }
        let script = drivers::mysql::execute_mysql_conn(&mut conn,
            "CREATE TABLE t(id BIGINT PRIMARY KEY, value DECIMAL(38,12), text_value TEXT); INSERT INTO t VALUES(1,12345678901234567890.123456789012,NULL); SELECT id,value,text_value,'NULL', '' FROM t", 100).await.unwrap();
        assert!(script.ok, "{:?}", script.error);
        assert_eq!(script.statements.len(), 3);
        assert_eq!(
            script.values[0][1].value.as_deref(),
            Some("12345678901234567890.123456789012")
        );
        assert_eq!(script.values[0][2].kind, "null");
        assert!(
            drivers::mysql::execute_mysql_conn(
                &mut conn,
                "BEGIN; INSERT INTO t VALUES(2,0,'rollback')",
                100
            )
            .await
            .unwrap()
            .ok
        );
        assert!(
            drivers::mysql::execute_mysql_conn(&mut conn, "ROLLBACK", 100)
                .await
                .unwrap()
                .ok
        );
        assert_eq!(
            drivers::mysql::execute_mysql_conn(&mut conn, "SELECT COUNT(*) FROM t", 100)
                .await
                .unwrap()
                .rows[0][0],
            "1"
        );
        conn.query_drop(
            "CREATE PROCEDURE multi_result() BEGIN SELECT 11 AS v1; SELECT 22 AS v2; END",
        )
        .await
        .unwrap();
        let multiple = drivers::mysql::execute_mysql_conn(
            &mut conn,
            "CALL multi_result(); SELECT 33 AS third_value",
            100,
        )
        .await
        .unwrap();
        assert!(multiple.ok, "{:?}", multiple.error);
        let sets: Vec<_> = multiple
            .statements
            .iter()
            .filter(|result| result.is_query)
            .collect();
        assert_eq!(sets.len(), 3);
        assert_eq!(sets[0].rows[0][0], "11");
        assert_eq!(sets[1].rows[0][0], "22");
        assert_eq!(sets[2].rows[0][0], "33");
        assert_eq!(sets[1].statement_index, Some(0));
        assert_eq!(sets[2].statement_index, Some(1));
        let mut cancel = drivers::CancelHandle::pending();
        cancel.mysql_thread_id = Some(conn.id());
        cancel.mysql_pool = Some(pool.clone());
        let started = std::time::Instant::now();
        let (slow, cancelled) = tokio::join!(
            drivers::mysql::execute_mysql_conn(&mut conn, "SELECT SLEEP(20)", 10),
            async {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                cancel.cancel().await
            }
        );
        cancelled.unwrap();
        let slow = slow.unwrap();
        assert!(
            !slow.ok
                || slow
                    .rows
                    .first()
                    .and_then(|r| r.first())
                    .is_some_and(|value| value == "1"),
            "SLEEP 被 KILL QUERY 中断时可正常返回 1"
        );
        assert!(started.elapsed().as_secs() < 10);
        let mut bound = super::super::catalog::bound::BoundConnection::Mysql(conn);
        verify_bound_changes(&mut bound, DbType::Mysql).await;
        bound
            .query(&format!("DROP DATABASE {unique}"), &[], 1)
            .await
            .unwrap();
        drop(bound);
        pool.disconnect().await.unwrap();
        eprintln!("[database-test] MySQL: 类型、脚本、多次事务和真实取消通过");

        let pg = config(
            DbType::Postgresql,
            host,
            cfg["postgresql"]["port"]
                .as_u64()
                .unwrap()
                .try_into()
                .unwrap(),
            cfg["postgresql"]["user"].as_str().unwrap(),
            "postgres",
        );
        let pool = drivers::postgres::pg_pool(&pg, cfg["postgresql"]["password"].as_str().unwrap())
            .await
            .unwrap();
        let client = pool.get().await.unwrap();
        client
            .batch_execute(&format!(
                "CREATE SCHEMA {unique}; SET search_path TO {unique}"
            ))
            .await
            .unwrap();
        if let Some(sql) = cfg["fixtures"]["postgresql"].as_str() {
            let fixture = drivers::postgres::execute_postgres_client(&client, sql, 100)
                .await
                .unwrap();
            assert!(fixture.ok, "{:?}", fixture.error);
            assert_eq!(fixture.rows.len(), 10);
        }
        let script = drivers::postgres::execute_postgres_client(&client,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, value NUMERIC, text_value TEXT); INSERT INTO t VALUES(1,12345678901234567890.123456789012,NULL) RETURNING id; SELECT id,value,text_value,'NULL', '' FROM t", 100).await.unwrap();
        assert!(script.ok, "{:?}", script.error);
        assert_eq!(script.statements.len(), 3);
        assert_eq!(script.statements[1].rows[0][0], "1");
        assert_eq!(script.values[0][0].kind, "integer");
        assert_eq!(
            script.values[0][1].value.as_deref(),
            Some("12345678901234567890.123456789012")
        );
        assert_eq!(script.values[0][2].kind, "null");
        let empty = drivers::postgres::execute_postgres_client(
            &client,
            "SELECT id FROM t WHERE false",
            100,
        )
        .await
        .unwrap();
        assert_eq!(empty.columns, ["id"]);
        let limit = drivers::postgres::execute_postgres_client(
            &client,
            "SELECT generate_series(1,10000)",
            50,
        )
        .await
        .unwrap();
        assert_eq!(limit.rows.len(), 50);
        assert!(limit.truncated);
        client
            .batch_execute("BEGIN; INSERT INTO t VALUES(2,0,'rollback')")
            .await
            .unwrap();
        drivers::postgres::execute_postgres_client(&client, "ROLLBACK", 100)
            .await
            .unwrap();
        assert_eq!(
            drivers::postgres::execute_postgres_client(&client, "SELECT COUNT(*) FROM t", 100)
                .await
                .unwrap()
                .rows[0][0],
            "1"
        );
        let mut cancel = drivers::CancelHandle::pending();
        cancel.pg_cancel = Some(client.cancel_token());
        let started = std::time::Instant::now();
        let (slow, cancelled) = tokio::join!(
            drivers::postgres::execute_postgres_client(&client, "SELECT pg_sleep(20)", 10),
            async {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                cancel.cancel().await
            }
        );
        cancelled.unwrap();
        assert!(!slow.unwrap().ok);
        assert!(started.elapsed().as_secs() < 10);
        let mut bound = super::super::catalog::bound::BoundConnection::Postgres(
            deadpool_postgres::Object::take(client),
        );
        verify_bound_changes(&mut bound, DbType::Postgresql).await;
        bound
            .query(&format!("DROP SCHEMA {unique} CASCADE"), &[], 1)
            .await
            .unwrap();
        drop(bound);
        pool.close();
        eprintln!("[database-test] PostgreSQL: 类型、RETURNING、空列、截断、事务和真实取消通过");

        if let Some(sql) = cfg["fixtures"]["sqlite"].as_str() {
            let path = std::env::temp_dir().join(format!("{unique}.sqlite"));
            let local = config(DbType::Sqlite, path.to_str().unwrap(), 0, "", "main");
            let connection = drivers::sqlite::sqlite_conn(&local).unwrap();
            let fixture = drivers::sqlite::execute_sqlite(&connection, sql, 100).unwrap();
            assert!(fixture.ok, "{:?}", fixture.error);
            assert_eq!(fixture.rows.len(), 10);
            drop(connection);
            std::fs::remove_file(path).unwrap();
        }
        let rd = config(
            DbType::Redis,
            host,
            cfg["redis"]["port"].as_u64().unwrap().try_into().unwrap(),
            "",
            "0",
        );
        let mut manager =
            drivers::connection::redis_mgr(&rd, cfg["redis"]["password"].as_str().unwrap())
                .await
                .unwrap();
        assert_eq!(
            drivers::redis::exec_command(&mut manager, "PING")
                .await
                .unwrap(),
            "PONG"
        );
        let _: () = redis::cmd("SET")
            .arg(&unique)
            .arg("")
            .arg("EX")
            .arg(60)
            .query_async(&mut manager)
            .await
            .unwrap();
        let value: String = redis::cmd("GET")
            .arg(&unique)
            .query_async(&mut manager)
            .await
            .unwrap();
        assert_eq!(value, "");
        let _: u64 = redis::cmd("DEL")
            .arg(&unique)
            .query_async(&mut manager)
            .await
            .unwrap();
        eprintln!("[database-test] Redis: 认证、空值写读和清理通过");
    }
    async fn verify_bound_changes(
        conn: &mut super::super::catalog::bound::BoundConnection,
        kind: DbType,
    ) {
        use super::super::{
            catalog::{mutation::mutation_sql, table_sql},
            models::{DbColumnInfo, DbValue, TableChange, TableFilter, TableOptions},
        };
        use std::collections::HashMap;
        let binary = if kind == DbType::Postgresql {
            "BYTEA"
        } else {
            "BLOB"
        };
        conn.query(&format!("CREATE TABLE bound_values(id BIGINT PRIMARY KEY, label TEXT, payload {binary}, enabled BOOLEAN, amount DECIMAL(38,12))"), &[], 1).await.unwrap();
        let columns: Vec<_> = [
            ("id", "bigint", "PK"),
            ("label", "text", "—"),
            ("payload", binary, "—"),
            ("enabled", "boolean", "—"),
            ("amount", "numeric", "—"),
        ]
        .into_iter()
        .map(|(name, data_type, key)| DbColumnInfo {
            name: name.into(),
            data_type: data_type.into(),
            key: key.into(),
            nullable: "是".into(),
            default_value: String::new(),
            comment: String::new(),
        })
        .collect();
        let values = HashMap::from([
            (
                "id".into(),
                DbValue::text("integer", "9223372036854775807".into()),
            ),
            (
                "label".into(),
                DbValue::text("text", "中文'); DROP TABLE bound_values; --".into()),
            ),
            ("payload".into(), DbValue::binary(&[0, 255, 10])),
            ("enabled".into(), DbValue::text("boolean", "true".into())),
            (
                "amount".into(),
                DbValue::text("decimal", "12345678901234567890.123456789012".into()),
            ),
        ]);
        let insert = TableChange {
            action: "insert".into(),
            values: values.clone(),
            original: HashMap::new(),
        };
        let (sql, params) = mutation_sql(kind, "bound_values", &columns, &insert).unwrap();
        conn.query("BEGIN", &[], 1).await.unwrap();
        assert_eq!(conn.query(&sql, &params, 1).await.unwrap().rows_affected, 1);
        conn.query("COMMIT", &[], 1).await.unwrap();
        let options = TableOptions {
            filters: vec![TableFilter {
                values: Vec::new(),
                column: "id".into(),
                operator: "eq".into(),
                value: values["id"].clone(),
            }],
            sort: vec![],
        };
        let (predicate, params) = table_sql::predicates(kind, &columns, &options).unwrap();
        let sql = format!(
            "SELECT {} FROM bound_values{predicate}",
            table_sql::projection(kind, &columns)
        );
        let mut row = conn.query(&sql, &params, 10).await.unwrap();
        if kind == DbType::Postgresql {
            table_sql::restore_pg_types(&mut row, &columns);
        }
        assert_eq!(
            row.values[0][0].value.as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(row.values[0][1].value, values["label"].value);
        assert_eq!(row.values[0][2].value.as_deref(), Some("00ff0a"));
        assert_eq!(row.values[0][4].value, values["amount"].value);
        let original = columns
            .iter()
            .zip(&row.values[0])
            .map(|(c, v)| (c.name.clone(), v.clone()))
            .collect();
        let change = TableChange {
            action: "update".into(),
            values: HashMap::from([("label".into(), DbValue::null())]),
            original,
        };
        let (update, params) = mutation_sql(kind, "bound_values", &columns, &change).unwrap();
        conn.query("BEGIN", &[], 1).await.unwrap();
        assert_eq!(
            conn.query(&update, &params, 1).await.unwrap().rows_affected,
            1
        );
        conn.query("ROLLBACK", &[], 1).await.unwrap();
        assert_eq!(
            conn.query(&update, &params, 1).await.unwrap().rows_affected,
            1
        );
        assert_eq!(
            conn.query(&update, &params, 1).await.unwrap().rows_affected,
            0,
            "旧原值必须检测出冲突"
        );
        eprintln!("[database-test] {kind}: 参数化筛选、精度、二进制、布尔、写入、回滚与冲突通过");
    }
    #[tokio::test]
    async fn bound_task_interrupts_sqlite_and_late_cancel_cannot_hit_reused_connection() {
        use super::super::catalog::bound::{BoundConnection, BoundTask};
        use std::sync::{Arc, Mutex};
        let native = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        let entry = drivers::DbSessionEntry {
            generation: uuid::Uuid::new_v4(),
            config: config(DbType::Sqlite, ":memory:", 0, "", "main"),
            session: drivers::DbSession::Sqlite(native.clone()),
            version: String::new(),
            latency_ms: 0,
            connected_at: 0,
        };
        let registry = drivers::DbCancelState(Mutex::new(Default::default()));
        let mut conn = BoundConnection::Sqlite(native);
        let mut task = BoundTask::register(&registry, "slow".into(), &entry.config.id).unwrap();
        task.bind(&conn, &entry).await.unwrap();
        let handle = task.handle.clone();
        let (result,cancelled) = tokio::join!(task.query(&mut conn,"WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM s WHERE n<1000000000) SELECT COUNT(*) FROM s",&[]),async {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            handle.cancel().await
        });
        cancelled.unwrap();
        assert!(result.is_err());
        drop(task);
        assert!(registry.0.lock().unwrap().is_empty());
        let mut task = BoundTask::register(&registry, "fast".into(), &entry.config.id).unwrap();
        task.bind(&conn, &entry).await.unwrap();
        assert_eq!(
            task.query(&mut conn, "SELECT 1", &[]).await.unwrap().rows[0][0],
            "1"
        );
        let handle = task.handle.clone();
        let late = handle.clone();
        let cancelled = tokio::spawn(async move { late.cancel().await });
        tokio::task::yield_now().await;
        drop(task);
        tokio::time::timeout(std::time::Duration::from_secs(1), cancelled)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!handle.aborted.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(
            conn.query("SELECT 2", &[], 1).await.unwrap().rows[0][0],
            "2"
        );
    }
}
