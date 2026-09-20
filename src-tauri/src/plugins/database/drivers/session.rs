//! 数据库驱动层 · 会话注册表与快照
//! 职责：持有全部连接会话状态 —— 会话注册表 DbState、取消句柄注册表 DbCancelState、
//! agent 运行时共享表 AgentRuntimeState；定义会话本体 DbSession（封闭枚举，按驱动分派）、
//! 会话条目 DbSessionEntry（连接元信息 + 探测缓存）与取消句柄 CancelHandle；
//! 提供前端连接列表用的全量快照 snapshot（含 agent 进程存活校验）。
//! 生命周期与锁不变量：注册表锁一律不跨 await —— 锁内只做最小提取（克隆 agent 句柄 +
//! 同步算好 to_info 快照），await 校验在锁外执行；会话条目在 disconnect 时移除，
//! agent 子进程由最后一个共享它的会话负责 shutdown（见 connection::disconnect）。

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection as SqliteConn;
use tauri::State;

use crate::plugins::database::agent::AgentClient;
use crate::plugins::database::models::{ConnConfig, ConnStatus, DbConnectionInfo};

/// agent 运行时共享表：driver key → 进程客户端（同驱动多会话共享一个进程）
pub struct AgentRuntimeState(pub Mutex<HashMap<&'static str, Arc<AgentClient>>>);

/// 会话注册表 State（命令层通过本结构取会话）
pub struct DbState(
    pub Mutex<HashMap<String, DbSessionEntry>>,
    pub Mutex<HashMap<String, uuid::Uuid>>,
);
impl DbState {
    /// 生命周期代次在建连前发布，保存、断开和删除会使迟到结果失效。
    pub(crate) fn next_generation(&self, id: &str) -> Result<uuid::Uuid, String> {
        let generation = uuid::Uuid::new_v4();
        self.1
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id.to_string(), generation);
        Ok(generation)
    }
    /// 核对连接代际，防止断开或重连后继续使用旧会话。
    pub(crate) fn is_current(&self, entry: &DbSessionEntry) -> Result<bool, String> {
        Ok(self
            .1
            .lock()
            .map_err(|e| e.to_string())?
            .get(&entry.config.id)
            == Some(&entry.generation))
    }
}

/// 查询取消注册表 State（conn_id → 取消句柄）
pub struct DbCancelState(pub Mutex<HashMap<String, CancelHandle>>);

/// 会话本体：按驱动分派的连接句柄
#[derive(Clone)]
pub enum DbSession {
    /// mysql_async 连接池（mysql/polardb）
    Mysql(mysql_async::Pool),
    /// deadpool-postgres 连接池
    Postgres(deadpool_postgres::Pool),
    /// rusqlite 单连接（SQLite 文件库，Arc 共享 + 锁内串行；会话表按值克隆）
    Sqlite(Arc<Mutex<SqliteConn>>),
    /// redis 连接管理器（多路复用）
    Redis(::redis::aio::ConnectionManager),
    /// agent 侧车会话（进程客户端 + 会话 id）
    Agent {
        /// 进程客户端（与同驱动其它会话共享）
        client: Arc<AgentClient>,
        /// 逻辑会话 id
        session_id: String,
    },
}

/// 会话条目：会话 + 连接元信息（版本/时延探测结果缓存）
#[derive(Clone)]
pub struct DbSessionEntry {
    /// 本次建连代际；后续重连使旧引用失效。
    pub generation: uuid::Uuid,
    /// 连接配置（不含密码）
    pub config: ConnConfig,
    /// 会话本体
    pub session: DbSession,
    /// 连接时探测的版本号
    pub version: String,
    /// 连接时探测的时延（毫秒）
    pub latency_ms: u64,
    /// 建立时间（epoch 秒）
    pub connected_at: u64,
}

/// 取消句柄绑定真实执行连接；gate 防止迟到取消命中同页的后继查询。
#[derive(Clone)]
pub struct CancelHandle {
    /// 所属连接，断开时据此收集任务。
    pub connection_id: String,
    /// 所属 SQL 页；空值表示短期表格任务。
    pub workspace_id: String,
    /// 已请求取消的共享标记。
    pub aborted: Arc<AtomicBool>,
    /// 驱动已收尾或已进入不可取消提交阶段。
    pub finished: Arc<AtomicBool>,
    /// 取消与连接复用的互斥门闩。
    pub gate: Arc<tokio::sync::Mutex<()>>,
    /// PostgreSQL 原生取消令牌。
    pub pg_cancel: Option<tokio_postgres::CancelToken>,
    /// 取消连接是否需要 TLS。
    pub pg_ssl: bool,
    /// 本次真实 MySQL 会话线程号。
    pub mysql_thread_id: Option<u32>,
    /// 发送 KILL QUERY 的管理连接池。
    pub mysql_pool: Option<mysql_async::Pool>,
    /// 当前 SQLite 连接的 interrupt 句柄。
    pub sqlite: Option<Arc<rusqlite::InterruptHandle>>,
    /// 侧车客户端及真实会话标识。
    pub agent: Option<(Arc<AgentClient>, String)>,
}
impl CancelHandle {
    /// 尚未取得工作连接时先登记，用户可在建连期间请求取消。
    pub(crate) fn pending() -> Self {
        Self {
            connection_id: String::new(),
            workspace_id: String::new(),
            aborted: Arc::new(AtomicBool::new(false)),
            finished: Arc::new(AtomicBool::new(false)),
            gate: Arc::new(tokio::sync::Mutex::new(())),
            pg_cancel: None,
            pg_ssl: false,
            mysql_thread_id: None,
            mysql_pool: None,
            sqlite: None,
            agent: None,
        }
    }
    /// 取消只表示请求已发送，最终结果由执行通道确定。
    pub(crate) async fn cancel(&self) -> Result<(), String> {
        use mysql_async::prelude::Queryable;
        use std::sync::atomic::Ordering;
        let _gate = self.gate.lock().await;
        if self.finished.load(Ordering::Acquire) {
            return Ok(());
        }
        self.aborted.store(true, Ordering::Release);
        if let Some(token) = &self.pg_cancel {
            if self.pg_ssl {
                let mut roots = rustls::RootCertStore::empty();
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                let config = rustls::ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth();
                token
                    .cancel_query(tokio_postgres_rustls::MakeRustlsConnect::new(config))
                    .await
                    .map_err(|e| format!("PG 取消失败: {e}"))?;
            } else {
                token
                    .cancel_query(tokio_postgres::NoTls)
                    .await
                    .map_err(|e| format!("PG 取消失败: {e}"))?;
            }
        }
        if let (Some(pool), Some(id)) = (&self.mysql_pool, self.mysql_thread_id) {
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("取消控制连接失败: {e}"))?;
            conn.query_drop(format!("KILL QUERY {id}"))
                .await
                .map_err(|e| format!("MySQL 取消失败: {e}"))?;
        }
        if let Some(handle) = &self.sqlite {
            handle.interrupt();
        }
        if let Some((client, id)) = &self.agent {
            client.cancel_session(id).await?;
        }
        Ok(())
    }
}

impl DbState {
    /// 取会话条目（不存在返回"未连接"错误）
    pub fn entry(&self, conn_id: &str) -> Result<DbSessionEntry, String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .get(conn_id)
            .cloned()
            .ok_or_else(|| format!("连接「{conn_id}」未建立，请先连接"))
    }
}

impl DbSessionEntry {
    /// 会话快照（前端连接列表展示）
    pub fn to_info(&self) -> DbConnectionInfo {
        DbConnectionInfo {
            credential_id: self.config.credential_id.clone(),
            id: self.config.id.clone(),
            label: self.config.label.clone(),
            db_type: self.config.db_type,
            env: self.config.env.clone(),
            status: ConnStatus::Online,
            version: self.version.clone(),
            latency_ms: self.latency_ms,
            readonly: self.config.readonly,
            host: self.config.host.clone(),
            database: self.config.database.clone(),
            error: None,
            connected_at: self.connected_at,
            port: self.config.port,
            username: self.config.username.clone(),
            ssl: self.config.ssl,
            connect_timeout_ms: self.config.connect_timeout_ms,
        }
    }
}

/// snapshot 锁内预提取的中间结构（agent 校验句柄 + 同步快照）
type PreppedSnapshot = Vec<(Option<(Arc<AgentClient>, String)>, Option<DbConnectionInfo>)>;

/// 全量会话快照（前端连接列表刷新；agent 会话做存活校验，进程被杀时标记离线）
pub async fn snapshot(state: &State<'_, DbState>, configs: &[ConnConfig]) -> Vec<DbConnectionInfo> {
    // 锁内做最小提取：同步算好 to_info 快照，只把 agent 校验需要的句柄拿出锁外 await
    // （原实现整图 m.clone()：每次快照深拷贝全部会话条目，且 unwrap_or_default 静默吞锁错误）
    let prepped: PreppedSnapshot = {
        let map = match state.0.lock() {
            Ok(map) => map,
            Err(e) => {
                eprintln!("[database] 会话注册表锁失败，快照返回空: {e}");
                return Vec::new();
            }
        };
        configs
            .iter()
            .map(|config| match map.get(&config.id) {
                Some(entry) => {
                    let agent = match &entry.session {
                        DbSession::Agent { client, session_id } => {
                            Some((client.clone(), session_id.clone()))
                        }
                        _ => None,
                    };
                    (agent, Some(entry.to_info()))
                }
                None => (None, None),
            })
            .collect()
    };
    let mut out = Vec::with_capacity(configs.len());
    for (config, (agent, info)) in configs.iter().zip(prepped) {
        let info = match info {
            Some(info) => match agent {
                Some((client, session_id)) => {
                    let alive = tokio::time::timeout(
                        Duration::from_secs(1),
                        client.validate_session(&session_id),
                    )
                    .await;
                    match alive {
                        Ok(Ok(())) => info,
                        Ok(Err(e)) => {
                            let mut info = info;
                            info.status = ConnStatus::Offline;
                            info.error = Some(format!("agent 进程不可达：{e}"));
                            info
                        }
                        Err(_) => {
                            let mut info = info;
                            info.status = ConnStatus::Offline;
                            info.error = Some("agent 存活校验超时（进程可能已退出）".to_string());
                            info
                        }
                    }
                }
                None => info,
            },
            None => DbConnectionInfo {
                credential_id: config.credential_id.clone(),
                id: config.id.clone(),
                label: config.label.clone(),
                db_type: config.db_type,
                env: config.env.clone(),
                status: ConnStatus::Offline,
                version: String::new(),
                latency_ms: 0,
                readonly: config.readonly,
                host: config.host.clone(),
                database: config.database.clone(),
                error: None,
                connected_at: 0,
                port: config.port,
                username: config.username.clone(),
                ssl: config.ssl,
                connect_timeout_ms: config.connect_timeout_ms,
            },
        };
        out.push(info);
    }
    out
}
