//! 数据库驱动层门面：只声明子模块并重导出，不含实现。
//! 子模块职责：session = 会话注册表/取消句柄/快照；connection = 连接、测试与断开；
//! probe = 版本与时延探测；execution = 查询执行与取消分派；
//! mysql / postgres / redis / sqlite = 各驱动传输实现（连接构建 + 查询执行 + 单元格字符串化）。
//! 公开路径经本文件重导出保持稳定（`drivers::connect`、`drivers::DbState` 等），
//! agent 与 dialect 的边界不变：传输归本层，SQL 语义归 dialect。

pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;

pub(crate) mod connection;
pub(crate) mod execution;
pub(crate) mod probe;
pub(crate) mod session;
pub(crate) mod workspace;
pub use workspace::WorkspaceState;

pub use connection::{connect, disconnect, test_connection};
pub(crate) use execution::execute_agent;
pub use session::{
    snapshot, AgentRuntimeState, CancelHandle, DbCancelState, DbSession, DbSessionEntry, DbState,
};
