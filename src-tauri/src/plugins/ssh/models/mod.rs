//! SSH 插件 · 数据结构（serde，与前端 plugins/ssh/contracts 同步）
//! 按域拆分子模块，此处 pub use 重导出——调用方引用路径 crate::plugins::ssh::models::* 不变。

mod common;
mod connect;
mod file;
mod host_key;
mod profile;
mod terminal;

pub use common::*;
pub use connect::*;
pub use file::*;
pub use host_key::*;
pub use profile::*;
pub use terminal::*;
