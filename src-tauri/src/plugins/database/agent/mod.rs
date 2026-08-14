//! agent 驱动子模块：进程侧车 + JSON-RPC 2.0 客户端（stdin/stdout NDJSON）
//! 协议对齐 dbx 的 agent 规范（agents/docs/agent-protocol-v2.md）：
//! - 进程启动后首行输出 {"ready":true}
//! - 请求/响应各占一行 JSON（{"jsonrpc":"2.0","id":N,"method":"...","params":{...}}）
//! - open_session 参数含 agentSessionId + 连接字段（snake_case，与 dbx ConnectParams 对齐）
//! - 查询结果形状：{columns, column_types, rows, affected_rows, execution_time_ms, truncated}
//!
//! 目录：client.rs（协议客户端）/ manager.rs（驱动 store 与下载）/ runtime.rs（进程生命周期）

pub mod client;
pub mod manager;

pub use client::{AgentClient, AgentConnectParams};
pub use manager::DriverStore;

/// agent 驱动 key（driver store 目录名，与 dbx 驱动注册名一致）
pub fn driver_key(db_type: crate::plugins::database::models::DbType) -> &'static str {
    match db_type {
        crate::plugins::database::models::DbType::Oracle => "oracle",
        crate::plugins::database::models::DbType::Vastbase => "vastbase",
        crate::plugins::database::models::DbType::Kingbase => "kingbase",
        crate::plugins::database::models::DbType::Dameng => "dameng",
        _ => "unknown",
    }
}
