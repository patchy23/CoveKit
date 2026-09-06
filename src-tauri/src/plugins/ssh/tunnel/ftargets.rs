//! SSH 隧道 · -R 转发目标表（SshHandler 回调按监听地址查表）

use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

/// -R 转发目标（SshHandler 回调按监听地址查此表，把服务端入站连接管道到本机侧目标）
pub struct ForwardTarget {
    /// 本机侧目标主机
    pub(crate) host: String,
    /// 本机侧目标端口
    pub(crate) port: u16,
    /// 活动连接数（与 TunnelHandle 共享同一计数器）
    pub(crate) counter: Arc<AtomicU64>,
}

/// -R 目标表：(监听 host, 监听 port) → 目标
pub type ForwardTargets = Arc<Mutex<HashMap<(String, u16), ForwardTarget>>>;

/// 新建空目标表（open_session 时创建，handler 与会话句柄共享）
pub fn new_forward_targets() -> ForwardTargets {
    Arc::new(Mutex::new(HashMap::new()))
}
