//! 同一次只读查询的分批消费。执行者独占工作连接，滚动请求只推进原结果流。
//! 未读完的协议不能直接还池；关闭、超时和维护都等待真实驱动收尾。

use futures_util::{stream::BoxStream, StreamExt, TryStreamExt};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot, watch, OwnedMutexGuard};

use super::{
    workspace::{self, WorkspaceConnection, WorkspaceSession},
    CancelHandle, DbCancelState,
};
use crate::plugins::database::{
    models::{DbValue, QueryResult},
    results::{ResultBudget, RESULT_BYTES_LIMIT},
};

const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
type Reply = oneshot::Sender<Result<QueryResult, String>>;
struct Demand {
    offset: u64,
    reply: Reply,
}
type Row = Vec<DbValue>;
type RowReply = oneshot::Sender<Result<Option<Row>, String>>;

#[derive(Clone)]
struct Entry {
    connection: String,
    workspace: String,
    epoch: Option<u64>,
    requests: mpsc::Sender<Demand>,
    cancel: CancelHandle,
    done: watch::Receiver<Option<Result<(), String>>>,
    replay: Arc<Mutex<Option<(u64, QueryResult)>>>,
    created: Instant,
}

/// 保存控制句柄和至多上一批重放数据；驱动借用只留在唯一执行者中。
#[derive(Default)]
pub struct CursorState(Mutex<HashMap<String, Entry>>);

impl CursorState {
    fn trim_completed(&self) {
        let Ok(mut entries) = self.0.lock() else {
            log::error!("清理查询结果缓存失败：注册表锁不可用");
            return;
        };
        let mut completed = entries
            .iter()
            .filter(|(_, entry)| entry.cancel.finished.load(Ordering::Acquire))
            .map(|(id, entry)| {
                let bytes = entry
                    .replay
                    .lock()
                    .ok()
                    .and_then(|replay| {
                        replay.as_ref().map(|(_, result)| {
                            result.values.iter().flatten().fold(0usize, |sum, value| {
                                sum.saturating_add(
                                    value
                                        .value
                                        .as_ref()
                                        .map_or(0, String::len)
                                        .saturating_mul(2)
                                        .saturating_add(128),
                                )
                            })
                        })
                    })
                    .unwrap_or(0);
                (id.clone(), entry.created, bytes)
            })
            .collect::<Vec<_>>();
        completed.sort_by_key(|(_, created, _)| *created);
        let mut bytes = completed
            .iter()
            .fold(0usize, |sum, (_, _, size)| sum.saturating_add(*size));
        for (id, _, size) in completed {
            if bytes <= 16 * 1024 * 1024 {
                break;
            }
            entries.remove(&id);
            bytes = bytes.saturating_sub(size);
        }
    }

    fn entry(&self, connection: &str, workspace: &str, id: &str) -> Result<Entry, String> {
        let entry = self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get(id)
            .cloned()
            .ok_or("DB_CURSOR_EXPIRED: 结果已结束或空闲过期，已加载数据仍可查看")?;
        if entry.connection != connection || entry.workspace != workspace {
            return Err("DB_CURSOR_SCOPE: 结果不属于当前工作页".into());
        }
        if entry
            .epoch
            .is_some_and(|epoch| !crate::framework::context::is_current(epoch))
        {
            return Err("DB_CURSOR_EXPIRED: 数据上下文已变化".into());
        }
        Ok(entry)
    }
}

/// 调用方已完成单语句只读判断、取消登记及工作页所有权转移。
pub(crate) async fn start(
    app: AppHandle,
    mut lane: OwnedMutexGuard<Option<WorkspaceSession>>,
    handle: CancelHandle,
    request_id: String,
    sql: String,
    batch_size: u64,
) -> Result<QueryResult, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (requests, receiver) = mpsc::channel(1);
    let (first, response) = oneshot::channel();
    let (done, completion) = watch::channel(None);
    let replay = Arc::new(Mutex::new(None));
    let entry = Entry {
        connection: handle.connection_id.clone(),
        workspace: handle.workspace_id.clone(),
        epoch: crate::framework::context::current().map(|context| context.epoch()),
        requests,
        cancel: handle.clone(),
        done: completion,
        replay: Arc::clone(&replay),
        created: Instant::now(),
    };
    let registered = (|| -> Result<(), String> {
        let state = app.state::<CursorState>();
        let mut entries = state.0.lock().map_err(|e| e.to_string())?;
        if entries.len() >= 32 {
            let oldest = entries
                .iter()
                .filter(|(_, entry)| entry.cancel.finished.load(Ordering::Acquire))
                .min_by_key(|(_, entry)| entry.created)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            } else {
                return Err("DB_CURSOR_LIMIT: 请先关闭部分查询结果".into());
            }
        }
        entries.insert(id.clone(), entry);
        Ok(())
    })();
    if let Err(error) = registered {
        let _gate = handle.gate.lock().await;
        handle.finished.store(true, Ordering::Release);
        if let Ok(mut map) = app.state::<DbCancelState>().0.lock() {
            map.remove(&request_id);
        }
        return Err(error);
    }
    crate::framework::storage::access::spawn(async move {
        let mut pager = Pager {
            id: id.clone(),
            first: Some(first),
            receiver,
            handle: handle.clone(),
            size: batch_size.clamp(1, 1000),
            loaded: 0,
            columns: Vec::new(),
            types: Vec::new(),
            replay,
            terminal: None,
            failure: None,
        };
        let outcome = if handle.aborted.load(Ordering::Acquire) {
            pager.failure = Some("DB_CANCELLED: 已取消，语句尚未发往服务器".into());
            Ok(())
        } else {
            match lane.as_mut() {
                Some(session) if !session.transaction => match &mut session.connection {
                    WorkspaceConnection::Mysql(conn, _) => mysql(conn, &sql, &mut pager).await,
                    WorkspaceConnection::Postgres(client) => {
                        postgres(client, &sql, &mut pager).await
                    }
                    WorkspaceConnection::Sqlite(conn) => {
                        sqlite(Arc::clone(conn), sql, &mut pager).await
                    }
                    _ => Err("当前驱动不支持连续读取结果".into()),
                },
                _ => Err("工作连接不存在或仍有活动事务".into()),
            }
        };
        let outcome = match outcome {
            Ok(()) => Ok(()),
            Err(error) => {
                // 任何未确认的协议状态都移出工作槽，绝不把未读连接当作健康连接复用。
                if let Some(session) = lane.take() {
                    match tokio::time::timeout(CLOSE_TIMEOUT, workspace::close(session)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(close)) => log::warn!("查询游标失效后关闭工作连接失败：{close}"),
                        Err(_) => log::warn!("查询游标失效后关闭工作连接超时"),
                    }
                }
                Err(format!(
                    "DB_CURSOR_SESSION_RESET: {error}；工作连接已失效，已加载数据保留"
                ))
            }
        };
        // 与迟到取消互斥后再释放工作槽；下一条查询不能被上一条取消误伤。
        let gate = handle.gate.lock().await;
        handle.finished.store(true, Ordering::Release);
        if let Ok(mut map) = app.state::<DbCancelState>().0.lock() {
            if map
                .get(&request_id)
                .is_some_and(|registered| Arc::ptr_eq(&registered.finished, &handle.finished))
            {
                map.remove(&request_id);
            }
        } else {
            log::error!("释放查询游标取消登记失败");
        }
        let mysql_session_gone = handle.mysql_session_gone.load(Ordering::Acquire);
        let lost_session = if mysql_session_gone {
            lane.take()
        } else {
            None
        };
        drop(lane);
        drop(gate);
        if let Some(session) = lost_session {
            match tokio::time::timeout(CLOSE_TIMEOUT, workspace::close(session)).await {
                Ok(Ok(())) => {}
                Ok(Err(close)) => log::warn!("MySQL 取消目标已消失，关闭工作连接失败：{close}"),
                Err(_) => log::warn!("MySQL 取消目标已消失，关闭工作连接超时"),
            }
        }
        if let Some(first) = pager.first.take() {
            let response = match &outcome {
                Err(error) => Err(error.clone()),
                Ok(()) => match pager.failure.take() {
                    Some(error) => Err(error),
                    None => pager
                        .terminal
                        .take()
                        .ok_or_else(|| "查询没有返回结果".to_string()),
                },
            };
            if let Ok(result) = &response {
                if let Ok(mut replay) = pager.replay.lock() {
                    *replay = Some((pager.loaded - result.values.len() as u64, result.clone()));
                }
            }
            app.state::<CursorState>().trim_completed();
            let _ = done.send(Some(outcome));
            let _ = first.send(response);
        } else {
            app.state::<CursorState>().trim_completed();
            let _ = done.send(Some(outcome));
        }
        // 只保留最后一批用于响应丢失后的同 offset 重试，不保留连接和操作租约。
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(IDLE_TIMEOUT).await;
            if let Ok(mut map) = app.state::<CursorState>().0.lock() {
                map.remove(&id);
            } else {
                log::error!("释放查询游标重放缓存失败");
            }
        });
    });
    response
        .await
        .map_err(|_| "查询游标执行者已结束".to_string())?
}

pub(crate) async fn fetch(
    state: &CursorState,
    conn_id: &str,
    workspace_id: &str,
    cursor_id: &str,
    expected_offset: u64,
) -> Result<QueryResult, String> {
    let entry = state.entry(conn_id, workspace_id, cursor_id)?;
    if let Some((offset, result)) = entry.replay.lock().map_err(|e| e.to_string())?.as_ref() {
        if *offset == expected_offset {
            return Ok(result.clone());
        }
    }
    if let Some(done) = entry.done.borrow().as_ref() {
        return Err(done
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| "DB_CURSOR_EXPIRED: 结果已经结束".into()));
    }
    let (reply, response) = oneshot::channel();
    entry
        .requests
        .try_send(Demand {
            offset: expected_offset,
            reply,
        })
        .map_err(|_| "DB_CURSOR_BUSY: 正在读取或关闭结果，请稍后重试".to_string())?;
    response
        .await
        .map_err(|_| "DB_CURSOR_EXPIRED: 结果执行者已结束".to_string())?
}

async fn cancel(handle: &CancelHandle) -> Result<(), String> {
    tokio::time::timeout(CLOSE_TIMEOUT, handle.cancel())
        .await
        .map_err(|_| "取消结果读取超时".to_string())?
}

async fn close_entry(mut entry: Entry) -> Result<(), String> {
    let cancellation = cancel(&entry.cancel).await;
    let completed = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(result) = entry.done.borrow().clone() {
                return result;
            }
            entry
                .done
                .changed()
                .await
                .map_err(|_| "游标完成通道已关闭".to_string())?;
        }
    })
    .await
    .map_err(|_| "DB_CURSOR_SESSION_RESET: 查询仍在收尾，暂不能执行下一条语句".to_string())?;
    completed.and(cancellation)
}

pub(crate) async fn close_cursor(
    state: &CursorState,
    conn_id: &str,
    workspace_id: &str,
    cursor_id: &str,
) -> Result<(), String> {
    let entry = match state.entry(conn_id, workspace_id, cursor_id) {
        Ok(entry) => entry,
        Err(error) if error.starts_with("DB_CURSOR_EXPIRED:") => return Ok(()),
        Err(error) => return Err(error),
    };
    let done = entry.done.clone();
    let result = close_entry(entry).await;
    if done.borrow().is_some() {
        state.0.lock().map_err(|e| e.to_string())?.remove(cursor_id);
    }
    result
}

pub(crate) async fn close_workspace(
    state: &CursorState,
    conn_id: &str,
    workspace_id: &str,
) -> Result<(), String> {
    let entries = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|(_, entry)| entry.connection == conn_id && entry.workspace == workspace_id)
        .map(|(id, entry)| (id.clone(), entry.clone()))
        .collect::<Vec<_>>();
    for (id, entry) in entries {
        let done = entry.done.clone();
        let result = close_entry(entry).await;
        if done.borrow().is_some() {
            state.0.lock().map_err(|e| e.to_string())?.remove(&id);
        }
        result?;
    }
    Ok(())
}

/// 维护只发取消；后台租约一直保留到执行者实际退出。
pub(crate) fn close_all(state: &CursorState) {
    let entries = match state.0.lock() {
        Ok(mut map) => map.drain().map(|(_, entry)| entry).collect::<Vec<_>>(),
        Err(error) => {
            log::error!("取消全部查询游标失败：{error}");
            return;
        }
    };
    for entry in entries {
        entry.cancel.aborted.store(true, Ordering::Release);
        entry.cancel.notify.notify_waiters();
        crate::framework::storage::access::spawn(async move {
            if let Err(error) = cancel(&entry.cancel).await {
                log::warn!("取消查询游标失败：{error}");
            }
        });
    }
}

enum End {
    Complete,
    Closed,
}

struct Pager {
    id: String,
    first: Option<Reply>,
    receiver: mpsc::Receiver<Demand>,
    handle: CancelHandle,
    size: u64,
    loaded: u64,
    columns: Vec<String>,
    types: Vec<String>,
    replay: Arc<Mutex<Option<(u64, QueryResult)>>>,
    terminal: Option<QueryResult>,
    failure: Option<String>,
}

impl Pager {
    async fn serve(
        &mut self,
        rows: &mut BoxStream<'_, Result<Row, String>>,
    ) -> Result<End, String> {
        let mut pending = None;
        let mut demand = Demand {
            offset: 0,
            reply: self.first.take().ok_or("首批结果应答已释放")?,
        };
        loop {
            if demand.offset != self.loaded {
                let cached = self.replay.lock().map_err(|e| e.to_string())?.clone();
                let result = match cached {
                    Some((offset, result)) if offset == demand.offset => Ok(result),
                    _ => Err("DB_CURSOR_OFFSET: 结果位置已变化，请勿跳过批次".into()),
                };
                let _ = demand.reply.send(result);
                match self.next_demand().await {
                    Some(next) => {
                        demand = next;
                        continue;
                    }
                    None => return Ok(End::Closed),
                }
            }
            let started = Instant::now();
            let page = self.page(rows, &mut pending).await;
            match page {
                Ok(mut result) => {
                    result.duration_ms = started.elapsed().as_millis() as u64;
                    let more = result.has_more;
                    if !more {
                        self.terminal = Some(result);
                        self.first = Some(demand.reply);
                        return Ok(End::Complete);
                    }
                    *self.replay.lock().map_err(|e| e.to_string())? =
                        Some((demand.offset, result.clone()));
                    // 应答接收端消失时仍保留有限批次；下一次同 offset 请求可重放。
                    let _ = demand.reply.send(Ok(result));
                }
                Err(error) => {
                    self.failure = Some(error.clone());
                    self.first = Some(demand.reply);
                    return Err(error);
                }
            }
            demand = match self.next_demand().await {
                Some(next) => next,
                None => return Ok(End::Closed),
            };
        }
    }

    async fn next_demand(&mut self) -> Option<Demand> {
        let notification = self.handle.notify.notified();
        tokio::pin!(notification);
        notification.as_mut().enable();
        if self.handle.aborted.load(Ordering::Acquire) {
            return None;
        }
        tokio::select! {
            request = self.receiver.recv() => request,
            _ = &mut notification => None,
            _ = tokio::time::sleep(IDLE_TIMEOUT) => None,
        }
    }

    async fn next(
        rows: &mut BoxStream<'_, Result<Row, String>>,
        deadline: tokio::time::Instant,
    ) -> Result<Option<Row>, String> {
        tokio::time::timeout_at(deadline, rows.try_next())
            .await
            .map_err(|_| "DB_TIMEOUT: 读取下一批结果超时".to_string())?
    }

    async fn page(
        &mut self,
        rows: &mut BoxStream<'_, Result<Row, String>>,
        pending: &mut Option<Row>,
    ) -> Result<QueryResult, String> {
        let mut result = QueryResult::empty();
        result.columns.clone_from(&self.columns);
        result.column_types.clone_from(&self.types);
        result.is_query = true;
        let mut budget = ResultBudget::new(self.size);
        let mut used = 0usize;
        let deadline = tokio::time::Instant::now() + READ_TIMEOUT;
        loop {
            if self.handle.aborted.load(Ordering::Acquire) {
                return Err("DB_CANCELLED: 已停止读取结果".into());
            }
            let row = match pending.take() {
                Some(row) => Some(row),
                None => Self::next(rows, deadline).await?,
            };
            let Some(row) = row else {
                break;
            };
            // 一行最多 32 MiB；超大行显式失败，不跳过后继续声称结果完整。
            let cost = row.iter().fold(0usize, |sum, value| {
                sum.saturating_add(
                    value
                        .value
                        .as_ref()
                        .map_or(0, String::len)
                        .saturating_mul(2)
                        .saturating_add(128),
                )
            });
            if cost > RESULT_BYTES_LIMIT {
                return Err(
                    "DB_ROW_TOO_LARGE: 单行结果超过 32 MiB，已停止读取；请缩小投影列".into(),
                );
            }
            if result.values.len() as u64 >= self.size
                || used.saturating_add(cost) > RESULT_BYTES_LIMIT
            {
                *pending = Some(row);
                result.has_more = true;
                break;
            }
            used += cost;
            budget.push(&mut result, row);
        }
        self.loaded += result.values.len() as u64;
        result.rows_affected = self.loaded;
        result.cursor_id = result.has_more.then(|| self.id.clone());
        Ok(result)
    }
}

fn single_row(
    budget: &mut ResultBudget,
    row: crate::plugins::database::results::ResultRow,
) -> Result<Row, String> {
    let mut result = QueryResult::empty();
    budget.finish_row(&mut result, row);
    result
        .values
        .pop()
        .ok_or_else(|| "DB_ROW_TOO_LARGE: 单行结果超过 32 MiB，已停止读取；请缩小投影列".into())
}

async fn mysql(conn: &mut mysql_async::Conn, sql: &str, pager: &mut Pager) -> Result<(), String> {
    use mysql_async::prelude::Queryable;
    let mut query = tokio::time::timeout(READ_TIMEOUT, conn.query_iter(sql))
        .await
        .map_err(|_| "MySQL 开始查询超时".to_string())?
        .map_err(|e| e.to_string())?;
    pager.columns = query
        .columns_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect();
    pager.types = query
        .columns_ref()
        .iter()
        .map(|column| format!("{:?}", column.column_type()))
        .collect();
    let binary = query
        .columns_ref()
        .iter()
        .map(|column| column.character_set() == 63)
        .collect::<Vec<_>>();
    let types = pager.types.clone();
    let stream = query
        .stream::<mysql_async::Row>()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("查询没有行结果")?;
    let mut stream = stream
        .map(move |row| {
            let row = row.map_err(|error| match error {
                mysql_async::Error::Server(error) => {
                    format!("MYSQL_{}: {}", error.code, error.message)
                }
                error => error.to_string(),
            })?;
            let mut budget = ResultBudget::new(1);
            let mut values = budget.row();
            for (index, value) in row.unwrap_raw().into_iter().enumerate() {
                super::mysql::append_mysql_value(
                    &mut values,
                    value.ok_or("MySQL 结果缺少单元格")?,
                    binary.get(index).copied().unwrap_or(false),
                    types.get(index).map(String::as_str).unwrap_or(""),
                );
            }
            single_row(&mut budget, values)
        })
        .boxed();
    let result = pager.serve(&mut stream).await;
    drop(stream);
    let end = match result {
        Ok(end) => end,
        Err(error)
            if pager.handle.aborted.load(Ordering::Acquire)
                && (error.starts_with("MYSQL_1317:") || error.starts_with("DB_CANCELLED:")) =>
        {
            End::Closed
        }
        Err(error) => return Err(error),
    };
    match end {
        End::Complete => tokio::time::timeout(CLOSE_TIMEOUT, query.drop_result())
            .await
            .map_err(|_| "MySQL 结果完成后的协议收尾超时".to_string())?
            .map_err(|error| format!("MySQL 结果完成后的协议收尾失败：{error}")),
        End::Closed => {
            cancel(&pager.handle).await?;
            if pager.handle.mysql_session_gone.load(Ordering::Acquire) {
                return Ok(());
            }
            match tokio::time::timeout(CLOSE_TIMEOUT, query.drop_result()).await {
                Ok(Ok(())) => Ok(()),
                // 驱动读取 ERR 包时已清 pending result，可以安全保留原工作连接。
                Ok(Err(mysql_async::Error::Server(error))) if error.code == 1317 => Ok(()),
                Ok(Err(error)) => Err(format!("MySQL 协议收尾失败：{error}")),
                Err(_) => Err("MySQL 协议收尾超时".into()),
            }
        }
    }
}

async fn postgres(
    client: &deadpool_postgres::ClientWrapper,
    sql: &str,
    pager: &mut Pager,
) -> Result<(), String> {
    let prepared = tokio::time::timeout(READ_TIMEOUT, client.prepare(sql))
        .await
        .map_err(|_| "PostgreSQL 读取列信息超时".to_string())?
        .map_err(|e| e.to_string())?;
    pager.columns = prepared
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect();
    pager.types = prepared
        .columns()
        .iter()
        .map(|column| column.type_().name().to_string())
        .collect();
    let types = pager.types.clone();
    let stream = tokio::time::timeout(READ_TIMEOUT, client.simple_query_raw(sql))
        .await
        .map_err(|_| "PostgreSQL 开始查询超时".to_string())?
        .map_err(|e| e.to_string())?;
    let mut stream = stream
        .filter_map(move |message| {
            let row = match message {
                Ok(tokio_postgres::SimpleQueryMessage::Row(row)) => pg_row(&row, &types).map(Some),
                Ok(_) => Ok(None),
                Err(error) => Err(format!(
                    "{}: {error}",
                    error
                        .code()
                        .map(|code| code.code())
                        .unwrap_or("PG_PROTOCOL")
                )),
            };
            std::future::ready(row.transpose())
        })
        .boxed();
    let end = match pager.serve(&mut stream).await {
        Ok(end) => end,
        Err(error)
            if pager.handle.aborted.load(Ordering::Acquire)
                && (error.starts_with("57014:") || error.starts_with("DB_CANCELLED:")) =>
        {
            End::Closed
        }
        Err(error) => return Err(error),
    };
    match end {
        End::Complete => Ok(()),
        End::Closed => {
            cancel(&pager.handle).await?;
            // 取消没有完成确认；继续读到流结束，后台响应通道容量为一，不预先积累全量行。
            tokio::time::timeout(CLOSE_TIMEOUT, async {
                while let Some(row) = stream.next().await {
                    if let Err(error) = row {
                        if error.starts_with("57014:") {
                            break;
                        }
                        return Err(error);
                    }
                }
                Ok(())
            })
            .await
            .map_err(|_| "PostgreSQL 协议收尾超时".to_string())?
        }
    }
}

fn pg_row(row: &tokio_postgres::SimpleQueryRow, types: &[String]) -> Result<Row, String> {
    let mut budget = ResultBudget::new(1);
    let mut values = budget.row();
    for index in 0..row.len() {
        let value = row.try_get(index).map_err(|e| e.to_string())?;
        let native = types.get(index).map(String::as_str).unwrap_or("text");
        match value {
            None => values.value(DbValue::null()),
            Some(value) => {
                let kind = match native {
                    "int2" | "int4" | "int8" | "oid" => "integer",
                    "numeric" | "money" => "decimal",
                    "float4" | "float8" => "float",
                    "bool" => "boolean",
                    "json" | "jsonb" => "json",
                    "bytea" => "binary",
                    "date" | "time" | "timetz" | "timestamp" | "timestamptz" | "interval" => {
                        "temporal"
                    }
                    _ => "text",
                };
                let text = if kind == "binary" {
                    value.strip_prefix("\\x").unwrap_or(value)
                } else if kind == "boolean" {
                    if value == "t" {
                        "true"
                    } else {
                        "false"
                    }
                } else {
                    value
                };
                values.text(kind, text);
            }
        }
    }
    single_row(&mut budget, values)
}

async fn sqlite(
    conn: Arc<Mutex<rusqlite::Connection>>,
    sql: String,
    pager: &mut Pager,
) -> Result<(), String> {
    let (requests, mut receiver) = mpsc::channel::<RowReply>(1);
    let (ready, metadata) = oneshot::channel();
    let work = crate::framework::storage::access::spawn_blocking(move || {
        let mut ready = Some(ready);
        let result = (|| -> Result<(), String> {
            let conn = conn.lock().map_err(|e| e.to_string())?;
            let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
            if !statement.readonly() {
                return Err("该 SQLite 语句不是只读查询".into());
            }
            let columns = statement
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            if let Some(ready) = ready.take() {
                let _ = ready.send(Ok(columns));
            }
            let mut rows = statement.query([]).map_err(|e| e.to_string())?;
            while let Some(reply) = receiver.blocking_recv() {
                let row = rows
                    .next()
                    .map_err(|error| {
                        if error.sqlite_error_code()
                            == Some(rusqlite::ErrorCode::OperationInterrupted)
                        {
                            format!("SQLITE_INTERRUPT: {error}")
                        } else {
                            error.to_string()
                        }
                    })
                    .and_then(|row| row.map(sqlite_row).transpose());
                let complete = !matches!(&row, Ok(Some(_)));
                if reply.send(row).is_err() || complete {
                    break;
                }
            }
            Ok(())
        })();
        if let Some(ready) = ready.take() {
            let _ = ready.send(Err(result
                .as_ref()
                .err()
                .cloned()
                .unwrap_or_else(|| "SQLite 查询初始化失败".into())));
        }
        result
    });
    let columns = metadata
        .await
        .unwrap_or_else(|_| Err("SQLite 初始化任务已结束".to_string()));
    let result = match columns {
        Ok(columns) => {
            pager.columns = columns;
            let mut stream = futures_util::stream::unfold(requests, |requests| async move {
                let (reply, response) = oneshot::channel();
                if requests.send(reply).await.is_err() {
                    return None;
                }
                match response.await {
                    Ok(Ok(Some(row))) => Some((Ok(row), requests)),
                    Ok(Ok(None)) => None,
                    Ok(Err(error)) => Some((Err(error), requests)),
                    Err(_) => Some((Err("SQLite 读取任务已结束".into()), requests)),
                }
            })
            .boxed();
            pager.serve(&mut stream).await.map(|_| ())
        }
        Err(error) => {
            drop(requests);
            Err(error)
        }
    };
    // 取消等待者不代表 sqlite3_step 已结束；始终等待真实阻塞工作退出。
    if result.is_err() || pager.handle.aborted.load(Ordering::Acquire) {
        if let Err(error) = cancel(&pager.handle).await {
            log::warn!("取消 SQLite 游标失败：{error}");
        }
    }
    let finished = work
        .await
        .map_err(|e| format!("SQLite 游标任务失败：{e}"))?;
    finished?;
    match result {
        Err(error)
            if pager.handle.aborted.load(Ordering::Acquire)
                && (error.starts_with("DB_CANCELLED:")
                    || error.starts_with("SQLITE_INTERRUPT:")) =>
        {
            Ok(())
        }
        result => result,
    }
}

fn sqlite_row(row: &rusqlite::Row<'_>) -> Result<Row, String> {
    use rusqlite::types::ValueRef;
    let mut budget = ResultBudget::new(1);
    let mut values = budget.row();
    for index in 0..row.as_ref().column_count() {
        match row.get_ref(index).map_err(|e| e.to_string())? {
            ValueRef::Null => values.value(DbValue::null()),
            ValueRef::Integer(value) => values.value(DbValue::text("integer", value.to_string())),
            ValueRef::Real(value) => values.value(DbValue::text("float", value.to_string())),
            ValueRef::Text(value) => match std::str::from_utf8(value) {
                Ok(value) => values.text("text", value),
                Err(_) => values.binary(value),
            },
            ValueRef::Blob(value) => values.binary(value),
        }
    }
    single_row(&mut budget, values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pager(
        size: u64,
    ) -> (
        Pager,
        mpsc::Sender<Demand>,
        oneshot::Receiver<Result<QueryResult, String>>,
    ) {
        let (sender, receiver) = mpsc::channel(1);
        let (first, response) = oneshot::channel();
        (
            Pager {
                id: "test-cursor".into(),
                first: Some(first),
                receiver,
                handle: CancelHandle::pending(),
                size,
                loaded: 0,
                columns: vec!["value".into()],
                types: Vec::new(),
                replay: Arc::new(Mutex::new(None)),
                terminal: None,
                failure: None,
            },
            sender,
            response,
        )
    }

    async fn next(sender: &mpsc::Sender<Demand>, offset: u64) -> QueryResult {
        let (reply, response) = oneshot::channel();
        sender.send(Demand { offset, reply }).await.unwrap();
        response.await.unwrap().unwrap()
    }

    fn finish_test(pager: &mut Pager) {
        let terminal = pager.terminal.take().unwrap();
        pager.first.take().unwrap().send(Ok(terminal)).unwrap();
    }

    #[tokio::test]
    async fn demand_pauses_original_stream_and_retries_replay_without_reading() {
        use std::sync::atomic::AtomicUsize;
        let (mut pager, sender, first) = pager(2);
        let reads = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&reads);
        let work = tokio::spawn(async move {
            let mut rows = futures_util::stream::iter((0..5).map(move |index| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![DbValue::text("integer", index.to_string())])
            }))
            .boxed();
            assert!(matches!(
                pager.serve(&mut rows).await.unwrap(),
                End::Complete
            ));
            finish_test(&mut pager);
        });
        let initial = first.await.unwrap().unwrap();
        assert_eq!(initial.rows, [vec!["0"], vec!["1"]]);
        assert!(initial.has_more);
        assert_eq!(reads.load(Ordering::SeqCst), 3, "只允许一行预读");
        let replay = next(&sender, 0).await;
        assert_eq!(replay.rows, initial.rows);
        assert_eq!(reads.load(Ordering::SeqCst), 3, "重放不能推进结果流");
        let second = next(&sender, 2).await;
        assert_eq!(second.rows, [vec!["2"], vec!["3"]]);
        let last = next(&sender, 4).await;
        assert_eq!(last.rows, [vec!["4"]]);
        assert!(!last.has_more);
        assert_eq!(last.rows_affected, 5);
        work.await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_reuses_one_statement_then_releases_connection_before_final_reply() {
        let connection = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        let worker_connection = Arc::clone(&connection);
        let (mut pager, sender, first) = pager(2);
        let work = tokio::spawn(async move {
            sqlite(worker_connection, "WITH RECURSIVE n(value) AS (VALUES(1) UNION ALL SELECT value+1 FROM n WHERE value<5) SELECT value FROM n".into(), &mut pager).await.unwrap();
            finish_test(&mut pager);
        });
        assert_eq!(first.await.unwrap().unwrap().rows.len(), 2);
        assert!(
            connection.try_lock().is_err(),
            "分批期间同一连接仍由执行者持有"
        );
        assert_eq!(next(&sender, 2).await.rows, [vec!["3"], vec!["4"]]);
        assert_eq!(next(&sender, 4).await.rows, [vec!["5"]]);
        assert_eq!(
            connection
                .lock()
                .unwrap()
                .query_row("SELECT 7", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            7
        );
        work.await.unwrap();
    }

    #[tokio::test]
    async fn oversized_row_is_reported_instead_of_skipped() {
        let (mut pager, _sender, _first) = pager(2);
        let mut rows = futures_util::stream::iter(vec![
            Ok(vec![DbValue::text("text", "x".repeat(4 * 1024 * 1024))]),
            Ok(vec![DbValue::text(
                "text",
                "must not skip to this row".into(),
            )]),
        ])
        .boxed();
        let error = pager.page(&mut rows, &mut None).await.err().unwrap();
        assert!(error.starts_with("DB_ROW_TOO_LARGE:"));
        assert_eq!(pager.loaded, 0);
    }
}
