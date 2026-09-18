//! 数据库插件 · 关闭清理钩子（AR06 §9.2 消费方接入）
//!
//! 清理由本模块自己提供，框架只负责协调、超时与汇总（`framework/lifecycle.rs`）。
//!
//! 退出路径只做**同步**动作：置取消标志 → 结束 agent 子进程 → 清空注册表。
//! agent 子进程必须显式结束：父子进程之间没有作业对象绑定，父进程退出不会连带结束子进程，
//! 留一个孤儿 sidecar 会在系统里长期挂着；协议 `shutdown` 是异步的，退出预算内不等它。
//!
//! 作用域只声明 `exit`：页签关闭时页面内 owner 按连接粒度逐个断开，
//! 后端再按工具整体清一遍会误伤同工具下其它页签还在用的连接。

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::Manager;

use crate::framework::lifecycle::CloseReason;
use crate::plugins::database::agent::AgentClient;

use super::drivers::{AgentRuntimeState, DbCancelState, DbState};

/// 本插件在统一关闭入口里的 owner 名（与 `covekit_module!{ owner: ... }` 一致，前端按它展示）
const OWNER: &str = "database";

/// 退出清理：取消查询 → 结束 agent 进程 → 清会话注册表
///
/// 返回失败原因（形如 `owner: 原因`）：锁中毒与子进程结束失败都不允许静默。
pub(crate) fn on_dispose(app: Option<&tauri::AppHandle>, _reason: CloseReason) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    let mut failures = Vec::new();

    // 1) 进行中的查询：置取消标志，让执行中的命令尽快返回
    match app.state::<DbCancelState>().0.lock() {
        Ok(map) => {
            for handle in map.values() {
                handle.aborted.store(true, Ordering::Relaxed);
            }
        }
        Err(_) => failures.push(format!("{OWNER}.query: 取消注册表锁不可用")),
    }

    // 2) agent 子进程：取走句柄后同步结束（取走即清表，重复清理不会重复 kill）
    let clients: Vec<Arc<AgentClient>> = match app.state::<AgentRuntimeState>().0.lock() {
        Ok(mut registry) => registry.drain().map(|(_, client)| client).collect(),
        Err(_) => {
            failures.push(format!("{OWNER}.agent: 运行时表锁不可用"));
            Vec::new()
        }
    };
    for client in clients {
        if let Err(error) = client.kill_now() {
            failures.push(format!("{OWNER}.agent: {error}"));
        }
    }

    // 3) 会话注册表：清空后连接池与连接随 drop 关闭（SQLite 连接在 drop 时关闭文件）
    match app.state::<DbState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.session: 会话注册表锁不可用")),
    }

    failures
}
