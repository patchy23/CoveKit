//! 参数化表格行为回归：特殊字符、NULL、主键和乐观并发，不依赖外部服务。
#[cfg(test)]
mod tests {
    use super::super::{bound::BoundConnection, mutation::mutation_sql, table_sql};
    use crate::plugins::database::models::{
        DbColumnInfo, DbType, DbValue, TableChange, TableFilter, TableOptions,
    };
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    fn columns() -> Vec<DbColumnInfo> {
        [
            ("id", "INTEGER", "PK"),
            ("text", "TEXT", "—"),
            ("data", "BLOB", "—"),
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
        .collect()
    }
    #[tokio::test]
    async fn bound_row_changes_preserve_values_and_reject_stale_original() {
        let mut conn = BoundConnection::Sqlite(Arc::new(Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        )));
        conn.query(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, text TEXT, data BLOB)",
            &[],
            1,
        )
        .await
        .unwrap();
        let text = "x'); DROP TABLE t; --\n中文";
        let values = HashMap::from([
            (
                "id".into(),
                DbValue::text("integer", "9223372036854775807".into()),
            ),
            ("text".into(), DbValue::text("text", text.into())),
            ("data".into(), DbValue::binary(&[0, 255])),
        ]);
        let insert = TableChange {
            action: "insert".into(),
            values: values.clone(),
            original: HashMap::new(),
        };
        let (sql, params) = mutation_sql(DbType::Sqlite, "\"t\"", &columns(), &insert).unwrap();
        assert!(!sql.contains(text));
        assert_eq!(conn.query(&sql, &params, 1).await.unwrap().rows_affected, 1);
        let row = conn
            .query("SELECT id,text,data FROM t", &[], 1)
            .await
            .unwrap();
        assert_eq!(row.values[0][1].value.as_deref(), Some(text));
        assert_eq!(row.values[0][2].value.as_deref(), Some("00ff"));
        let update = TableChange {
            action: "update".into(),
            original: values.clone(),
            values: HashMap::from([("text".into(), DbValue::null())]),
        };
        let (sql, params) = mutation_sql(DbType::Sqlite, "\"t\"", &columns(), &update).unwrap();
        conn.query("BEGIN", &[], 1).await.unwrap();
        assert_eq!(conn.query(&sql, &params, 1).await.unwrap().rows_affected, 1);
        conn.query("ROLLBACK", &[], 1).await.unwrap();
        assert_eq!(
            conn.query("SELECT text FROM t", &[], 1).await.unwrap().rows[0][0],
            text
        );
        conn.query("UPDATE t SET text='other client'", &[], 1)
            .await
            .unwrap();
        assert_eq!(
            conn.query(&sql, &params, 1).await.unwrap().rows_affected,
            0,
            "原值改变时不能覆盖其他客户端的数据"
        );
        let no_keys: Vec<_> = columns()
            .into_iter()
            .map(|mut c| {
                c.key = "—".into();
                c
            })
            .collect();
        assert!(mutation_sql(DbType::Sqlite, "\"t\"", &no_keys, &update).is_err());
    }
    #[test]
    fn filters_bind_values_and_sort_rejects_unknown_columns() {
        let options = TableOptions {
            filters: vec![TableFilter {
                values: Vec::new(),
                column: "text".into(),
                operator: "eq".into(),
                value: DbValue::text("text", "' OR 1=1 --".into()),
            }],
            sort: vec![],
        };
        let (sql, values) =
            table_sql::predicates(DbType::Postgresql, &columns(), &options).unwrap();
        assert_eq!(sql, " WHERE \"text\" = $1");
        assert_eq!(values.len(), 1);
        let mut invalid = options;
        invalid.filters[0].column = "missing".into();
        assert!(table_sql::predicates(DbType::Postgresql, &columns(), &invalid).is_err());
    }

    #[tokio::test]
    async fn combined_in_and_range_filters_bind_all_values() {
        let mut conn = BoundConnection::Sqlite(Arc::new(Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        )));
        conn.query(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, text TEXT, data BLOB)",
            &[],
            1,
        )
        .await
        .unwrap();
        conn.query(
            "INSERT INTO t VALUES(1,'one',NULL),(2,'two',NULL),(3,'three',NULL)",
            &[],
            1,
        )
        .await
        .unwrap();
        let text = |value: &str| DbValue::text("text", value.into());
        let options = TableOptions {
            filters: vec![
                TableFilter {
                    column: "id".into(),
                    operator: "between".into(),
                    value: DbValue::null(),
                    values: vec![text("1"), text("3")],
                },
                TableFilter {
                    column: "text".into(),
                    operator: "in".into(),
                    value: DbValue::null(),
                    values: vec![text("two"), text("x'); DROP TABLE t; --")],
                },
            ],
            sort: vec![],
        };
        let (filter, params) = table_sql::predicates(DbType::Sqlite, &columns(), &options).unwrap();
        let result = conn
            .query(&format!("SELECT id FROM t{filter}"), &params, 10)
            .await
            .unwrap();
        assert_eq!(result.rows, vec![vec!["2".to_string()]]);
        assert!(!filter.contains("DROP"));
    }
}
