//! SSH 插件 · 关闭清理钩子（AR06 §9.2 消费方接入）
//!
//! 清理由本模块自己提供，框架只负责协调、超时与汇总（`framework/lifecycle.rs`）。
//!
//! 只做**同步**动作：置取消标志 + 清空注册表（句柄 drop 即关闭底层连接）。
//! 理由：退出清理有总预算（`lifecycle::DISPOSE_TIMEOUT`），在这里等对端回包会把预算耗光，
//! 后面的模块就没机会清理了；进程随即退出，远端会看到连接结束。
//!
//! 作用域只声明 `exit`：页签关闭时页面内 owner 按连接/终端粒度逐个断开，
//! 后端再按工具整体清一遍会误伤同工具下其它页签还在用的会话。

use std::sync::atomic::Ordering;

use tauri::Manager;

use crate::framework::lifecycle::CloseReason;

use super::conn::{HostKeyState, SshState};
use super::monitor::MonitorState;
use super::sftp::TransferState;
use super::terminal::TerminalState;
use super::tunnel::TunnelState;

/// 本插件在统一关闭入口里的 owner 名（与 `patchybox_module!{ owner: ... }` 一致）与失败前缀
const OWNER: &str = "ssh";

/// 退出清理：取消传输 → 停终端 → 停隧道 → 断会话 → 放掉主机密钥等待
///
/// 返回失败原因（形如 `owner: 原因`）：注册表锁不可用等异常不允许静默。
pub(crate) fn on_dispose(app: Option<&tauri::AppHandle>, _reason: CloseReason) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    let mut failures = Vec::new();

    // 1) 进行中的 SFTP 传输：置取消标志，让传输任务收尾并清理临时文件
    match app.state::<TransferState>().0.lock() {
        Ok(map) => {
            for flag in map.values() {
                flag.store(true, Ordering::Relaxed);
            }
        }
        Err(_) => failures.push(format!("{OWNER}.transfer: 传输注册表锁不可用")),
    }

    // 2) 终端会话：清空注册表，句柄 drop 关掉 PTY 与读线程
    match app.state::<TerminalState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.terminal: 终端注册表锁不可用")),
    }

    // 3) 隧道：清空注册表，本地监听随句柄释放停止
    match app.state::<TunnelState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.tunnel: 隧道注册表锁不可用")),
    }

    // 4) 会话：清空注册表，russh 句柄 drop 关闭连接（进程即将退出，不等优雅断开）
    match app.state::<SshState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.session: 会话注册表锁不可用")),
    }

    // 5) 待应答的主机密钥确认：清空后等待中的命令立即返回，不会挂住退出
    match app.state::<HostKeyState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.hostkey: 主机密钥注册表锁不可用")),
    }

    // 6) 监控采样：无外部资源，顺手清掉，避免下次读到大盘期的旧计数
    match app.state::<MonitorState>().0.lock() {
        Ok(mut registry) => registry.clear(),
        Err(_) => failures.push(format!("{OWNER}.monitor: 监控注册表锁不可用")),
    }

    failures
}
