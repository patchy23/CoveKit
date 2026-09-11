//! SSH 数据模型 · 终端（会话快照 / 数据块 / 关闭通知）

use serde::Serialize;

/* ── 终端 ── */

/// 终端会话快照
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    /// 终端唯一 id（term-<毫秒时间戳>）
    pub(crate) id: String,
    /// 关联的连接 sessionId
    pub(crate) connection_id: String,
    /// 终端标题（默认 profile 名称）
    pub(crate) title: String,
    /// 当前行列数
    pub(crate) cols: u16,
    /// 当前行数
    pub(crate) rows: u16,
    /// 是否活跃（前端正在展示）
    pub(crate) active: bool,
}

/// 终端数据块（Rust 推送 → 前端渲染）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TerminalData {
    /// 关联的终端 id
    pub(crate) terminal_id: String,
    /// 所属连接 id（前端据此把工作区标记为活跃，防止看日志时被空闲断开误杀）
    pub(crate) connection_id: String,
    /// 数据内容（原始字节，含 ANSI 转义序列）
    pub(crate) data: String,
    /// 毫秒时间戳
    pub(crate) time: u64,
}

/// 终端 PTY 通道关闭通知。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TerminalClosed {
    /// 已关闭的终端 id。
    pub(crate) terminal_id: String,
    /// 所属连接会话 id（前端据此触发断线自动重连）
    pub(crate) connection_id: String,
}

/* ── 会话日志 ── */

/// 会话日志操作结果（开始 / 停止共用）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogActionResult {
    /// 日志文件绝对路径
    pub(crate) path: String,
    /// 已写入字节数
    pub(crate) bytes: u64,
}

/// 日志写盘失败通知（前端据此自动停止录制并提示，禁止静默）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogError {
    /// 对应终端 id
    pub(crate) terminal_id: String,
    /// 失败原因（含路径）
    pub(crate) message: String,
}
