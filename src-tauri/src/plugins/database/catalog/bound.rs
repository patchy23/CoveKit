//! 表格筛选和编辑的参数化执行器；值绝不拼接到 SQL。
use crate::plugins::database::{
    drivers::{DbSession, DbSessionEntry},
    models::{DbValue, QueryResult},
    results::ResultBudget,
};
use mysql_async::prelude::Queryable;
use std::sync::{Arc, Mutex};
use tokio_postgres::types::{Format, IsNull, ToSql, Type};

#[derive(Debug)]
struct TextParameter(Option<String>);
impl ToSql for TextParameter {
    fn to_sql(
        &self,
        _: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match &self.0 {
            Some(text) => {
                out.extend_from_slice(text.as_bytes());
                Ok(IsNull::No)
            }
            None => Ok(IsNull::Yes),
        }
    }
    fn accepts(_: &Type) -> bool {
        true
    }
    fn encode_format(&self, _: &Type) -> Format {
        Format::Text
    }
    tokio_postgres::types::to_sql_checked!();
}

/// 一批表格操作只租用同一连接；PostgreSQL 独占租约退出时关闭，防止事务归池。
pub(crate) enum BoundConnection {
    Mysql(mysql_async::Conn),
    Postgres(deadpool_postgres::ClientWrapper),
    Sqlite(Arc<Mutex<rusqlite::Connection>>),
}
impl BoundConnection {
    /// 按执行目标获取原生连接；调用方持有连接直至事务和取消收尾。
    pub(crate) async fn open(entry: &DbSessionEntry) -> Result<Self, String> {
        match &entry.session {
            DbSession::Mysql(pool) => Ok(Self::Mysql(
                pool.get_conn().await.map_err(|e| e.to_string())?,
            )),
            DbSession::Postgres(pool) => Ok(Self::Postgres(deadpool_postgres::Object::take(
                pool.get().await.map_err(|e| e.to_string())?,
            ))),
            DbSession::Sqlite(_) => {
                let config = entry.config.clone();
                let conn = tokio::task::spawn_blocking(move || {
                    crate::plugins::database::drivers::sqlite::sqlite_conn(&config)
                })
                .await
                .map_err(|e| e.to_string())??;
                Ok(Self::Sqlite(conn))
            }
            _ => Err("DB_UNSUPPORTED: 当前驱动未提供参数绑定；请使用 SQL 工作页".into()),
        }
    }
    /// 读取 SQL 必须在外层限定页大小；PG 投影文本列，原生类型由元数据恢复。
    pub(crate) async fn query(
        &mut self,
        sql: &str,
        params: &[DbValue],
        limit: u64,
    ) -> Result<QueryResult, String> {
        let mut result = QueryResult::empty();
        let mut budget = ResultBudget::new(limit);
        match self {
            Self::Mysql(conn) => {
                // MySQL 的事务控制不支持预处理协议；仅这三个内部固定命令走文本协议。
                if params.is_empty() && matches!(sql, "BEGIN" | "COMMIT" | "ROLLBACK") {
                    conn.query_drop(sql).await.map_err(|e| e.to_string())?;
                    return Ok(result);
                }
                let values = params
                    .iter()
                    .map(mysql_param)
                    .collect::<Result<Vec<_>, _>>()?;
                let mut query = conn
                    .exec_iter(sql, values)
                    .await
                    .map_err(|e| e.to_string())?;
                result.columns = query
                    .columns_ref()
                    .iter()
                    .map(|c| c.name_str().to_string())
                    .collect();
                result.column_types = query
                    .columns_ref()
                    .iter()
                    .map(|c| format!("{:?}", c.column_type()))
                    .collect();
                let binary: Vec<_> = query
                    .columns_ref()
                    .iter()
                    .map(|c| c.character_set() == 63)
                    .collect();
                result.is_query = !result.columns.is_empty();
                while let Some(row) = query.next().await.map_err(|e| e.to_string())? {
                    let mut cells = Vec::with_capacity(row.len());
                    for i in 0..row.len() {
                        let value = row
                            .get::<mysql_async::Value, usize>(i)
                            .ok_or("结果缺少单元格")?;
                        cells.push(crate::plugins::database::drivers::mysql::mysql_value(
                            value,
                            binary[i],
                            &result.column_types[i],
                        ));
                    }
                    budget.push(&mut result, cells);
                }
                result.rows_affected = query.affected_rows();
                query.drop_result().await.map_err(|e| e.to_string())?;
            }
            Self::Postgres(client) => {
                let values = params.iter().map(pg_param).collect::<Result<Vec<_>, _>>()?;
                let refs: Vec<&(dyn ToSql + Sync)> =
                    values.iter().map(|v| v as &(dyn ToSql + Sync)).collect();
                let statement = client.prepare(sql).await.map_err(|e| e.to_string())?;
                result.columns = statement
                    .columns()
                    .iter()
                    .map(|c| c.name().to_string())
                    .collect();
                result.is_query = !result.columns.is_empty();
                if result.is_query {
                    use futures_util::TryStreamExt;
                    let stream = client
                        .query_raw(&statement, refs)
                        .await
                        .map_err(|e| e.to_string())?;
                    futures_util::pin_mut!(stream);
                    while let Some(row) = stream.try_next().await.map_err(|e| e.to_string())? {
                        let mut cells = Vec::new();
                        for i in 0..row.len() {
                            cells.push(
                                match row
                                    .try_get::<_, Option<String>>(i)
                                    .map_err(|e| e.to_string())?
                                {
                                    Some(value) => DbValue::text("text", value),
                                    None => DbValue::null(),
                                },
                            );
                        }
                        budget.push(&mut result, cells);
                    }
                } else {
                    result.rows_affected = client
                        .execute(&statement, &refs)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            Self::Sqlite(conn) => {
                let conn = Arc::clone(conn);
                let sql = sql.to_string();
                let values = params
                    .iter()
                    .map(sqlite_param)
                    .collect::<Result<Vec<_>, _>>()?;
                return tokio::task::spawn_blocking(move || {
                    let guard = conn.lock().map_err(|e| e.to_string())?;
                    let mut statement = guard.prepare(&sql).map_err(|e| e.to_string())?;
                    let mut result = QueryResult::empty();
                    result.columns = statement
                        .column_names()
                        .iter()
                        .map(|c| c.to_string())
                        .collect();
                    result.is_query = !result.columns.is_empty();
                    if result.is_query {
                        let mut rows = statement
                            .query(rusqlite::params_from_iter(values))
                            .map_err(|e| e.to_string())?;
                        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                            let mut cells = Vec::new();
                            for i in 0..result.columns.len() {
                                use rusqlite::types::ValueRef;
                                cells.push(match row.get_ref(i).map_err(|e| e.to_string())? {
                                    ValueRef::Null => DbValue::null(),
                                    ValueRef::Integer(n) => DbValue::text("integer", n.to_string()),
                                    ValueRef::Real(n) => DbValue::text("float", n.to_string()),
                                    ValueRef::Text(s) => DbValue::text(
                                        "text",
                                        std::str::from_utf8(s)
                                            .map_err(|e| e.to_string())?
                                            .to_string(),
                                    ),
                                    ValueRef::Blob(s) => DbValue::binary(s),
                                });
                            }
                            budget.push(&mut result, cells);
                        }
                    } else {
                        result.rows_affected = statement
                            .execute(rusqlite::params_from_iter(values))
                            .map_err(|e| e.to_string())?
                            as u64;
                    }
                    Ok(result)
                })
                .await
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(result)
    }
}
fn scalar(value: &DbValue) -> Result<Option<&str>, String> {
    if value.kind == "null" {
        return Ok(None);
    }
    if ![
        "text", "integer", "decimal", "float", "boolean", "temporal", "json", "binary",
    ]
    .contains(&value.kind.as_str())
    {
        return Err("未知单元格类型".into());
    }
    let text = value.value.as_deref().ok_or("非 NULL 值缺少内容")?;
    if text.len() > 1024 * 1024 {
        return Err("单元格超过 1 MiB 编辑限制".into());
    }
    Ok(Some(text))
}
fn mysql_param(value: &DbValue) -> Result<mysql_async::Value, String> {
    Ok(match scalar(value)? {
        None => mysql_async::Value::NULL,
        Some(text) if value.kind == "boolean" => mysql_async::Value::Int(match text {
            "true" | "1" => 1,
            "false" | "0" => 0,
            _ => return Err("布尔值无效".into()),
        }),
        Some(text) if value.kind == "binary" => {
            mysql_async::Value::Bytes(hex::decode(text).map_err(|_| "二进制必须为十六进制")?)
        }
        Some(text) => mysql_async::Value::Bytes(text.as_bytes().to_vec()),
    })
}
fn pg_param(value: &DbValue) -> Result<TextParameter, String> {
    Ok(TextParameter(match scalar(value)? {
        Some(text) if value.kind == "binary" => {
            hex::decode(text).map_err(|_| "二进制必须为十六进制")?;
            Some(format!("\\x{text}"))
        }
        value => value.map(str::to_string),
    }))
}
fn sqlite_param(value: &DbValue) -> Result<rusqlite::types::Value, String> {
    use rusqlite::types::Value;
    Ok(match scalar(value)? {
        None => Value::Null,
        Some(text) => match value.kind.as_str() {
            "integer" => Value::Integer(
                text.parse()
                    .map_err(|_| "整数超出 SQLite 有符号 64 位范围")?,
            ),
            "float" => Value::Real(text.parse().map_err(|_| "浮点值无效")?),
            "boolean" => Value::Integer(match text {
                "true" | "1" => 1,
                "false" | "0" => 0,
                _ => return Err("布尔值无效".into()),
            }),
            "binary" => Value::Blob(hex::decode(text).map_err(|_| "二进制必须为十六进制")?),
            _ => Value::Text(text.to_string()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mysql_boolean_and_binary_parameters_keep_native_values() {
        assert_eq!(
            mysql_param(&DbValue::text("boolean", "true".into())).unwrap(),
            mysql_async::Value::Int(1)
        );
        assert_eq!(
            mysql_param(&DbValue::text("boolean", "0".into())).unwrap(),
            mysql_async::Value::Int(0)
        );
        assert!(mysql_param(&DbValue::text("boolean", "yes".into())).is_err());
        assert_eq!(
            mysql_param(&DbValue::binary(&[0, 255])).unwrap(),
            mysql_async::Value::Bytes(vec![0, 255])
        );
    }
}

/// 参数化写入登记真实取消句柄，断开连接可找到它；超时后等待驱动收尾。
pub(crate) struct BoundTask<'a> {
    /// 借用调用方的取消注册表。
    state: &'a crate::plugins::database::drivers::DbCancelState,
    /// 任务唯一标识，Drop 时从注册表移除。
    id: String,
    /// 当前任务发布的真实取消句柄。
    pub(crate) handle: crate::plugins::database::drivers::CancelHandle,
    /// 两次语句之间持有，防止迟到取消作用于归还连接。
    idle_gate: Option<tokio::sync::OwnedMutexGuard<()>>,
}
impl<'a> BoundTask<'a> {
    /// 注册任务身份，重复身份拒绝；生命周期结束时移除取消句柄。
    pub(crate) fn register(
        state: &'a crate::plugins::database::drivers::DbCancelState,
        id: String,
        connection_id: &str,
    ) -> Result<Self, String> {
        let mut handle = crate::plugins::database::drivers::CancelHandle::pending();
        handle.connection_id = connection_id.into();
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        if id.is_empty() || map.contains_key(&id) {
            return Err("任务身份无效或重复".into());
        }
        map.insert(id.clone(), handle.clone());
        Ok(Self {
            state,
            id,
            handle,
            idle_gate: None,
        })
    }
    /// 持有空闲门闩后发布真实取消句柄，避免取消误伤复用连接。
    pub(crate) async fn bind(
        &mut self,
        conn: &BoundConnection,
        entry: &DbSessionEntry,
    ) -> Result<(), String> {
        self.idle_gate = Some(self.handle.gate.clone().lock_owned().await);
        match conn {
            BoundConnection::Mysql(conn) => {
                self.handle.mysql_thread_id = Some(conn.id());
                if let DbSession::Mysql(pool) = &entry.session {
                    self.handle.mysql_pool = Some(pool.clone());
                }
            }
            BoundConnection::Postgres(client) => {
                self.handle.pg_cancel = Some(client.cancel_token());
                self.handle.pg_ssl = entry.config.ssl;
            }
            BoundConnection::Sqlite(conn) => {
                self.handle.sqlite = Some(Arc::new(
                    conn.lock()
                        .map_err(|e| e.to_string())?
                        .get_interrupt_handle(),
                ));
            }
        }
        self.state
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(self.id.clone(), self.handle.clone());
        self.check()
    }
    /// 检查取消标记；取消后禁止发出下一条 SQL。
    pub(crate) fn check(&self) -> Result<(), String> {
        if self
            .handle
            .aborted
            .load(std::sync::atomic::Ordering::Acquire)
        {
            Err("DB_CANCELLED: 操作已取消，未提交修改将回滚".into())
        } else {
            Ok(())
        }
    }
    /// 以单行结果预算执行参数化语句，并等待超时取消收尾。
    pub(crate) async fn query(
        &mut self,
        conn: &mut BoundConnection,
        sql: &str,
        params: &[DbValue],
    ) -> Result<QueryResult, String> {
        self.query_limited(conn, sql, params, 1).await
    }
    /// 限定结果行数和执行时间；查询结束后持锁直到下次执行或释放。
    pub(crate) async fn query_limited(
        &mut self,
        conn: &mut BoundConnection,
        sql: &str,
        params: &[DbValue],
        limit: u64,
    ) -> Result<QueryResult, String> {
        self.check()?;
        self.idle_gate.take();
        let query = conn.query(sql, params, limit);
        tokio::pin!(query);
        let result = tokio::select! {
            result = &mut query => result,
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                let cancelled = tokio::time::timeout(std::time::Duration::from_secs(5),self.handle.cancel()).await;
                let settled = tokio::time::timeout(std::time::Duration::from_secs(5), &mut query).await;
                if settled.is_err() || !matches!(cancelled,Ok(Ok(()))) {
                    Err("DB_OUTCOME_UNKNOWN: 操作超时且服务端停止未确认，请核对事务状态".into())
                } else { Err("DB_TIMEOUT: 操作超过 30 秒，已请求服务端停止".into()) }
            }
        };
        // 归还连接前阻止迟到取消；下一条同任务 SQL 开始时才释放。
        self.idle_gate = Some(self.handle.gate.clone().lock_owned().await);
        result
    }
    /// 与取消请求互斥后进入提交阶段，提交期间不再发送迟到取消。
    pub(crate) async fn before_commit(&mut self) -> Result<(), String> {
        if self.idle_gate.is_none() {
            self.idle_gate = Some(self.handle.gate.clone().lock_owned().await);
        }
        self.handle
            .finished
            .store(true, std::sync::atomic::Ordering::Release);
        let result = self.check();
        self.idle_gate.take();
        result
    }
}
impl Drop for BoundTask<'_> {
    fn drop(&mut self) {
        self.handle
            .finished
            .store(true, std::sync::atomic::Ordering::Release);
        if let Ok(mut map) = self.state.0.lock() {
            map.remove(&self.id);
        }
    }
}
