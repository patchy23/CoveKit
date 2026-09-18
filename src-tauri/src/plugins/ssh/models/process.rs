//! SSH 数据模型 · process

use serde::Serialize;

/* ── 进程管理 ── */

/// 进程条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// 进程 ID
    pub(crate) pid: u32,
    /// 运行用户
    pub(crate) user: String,
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存占用（字节）
    pub(crate) memory_bytes: u64,
    /// 启动时间（毫秒时间戳）
    pub(crate) started_at: u64,
    /// 完整命令行
    pub(crate) command: String,
}

/// 单个进程详情（`ps -fp` 输出解析）
///
/// 远端 ps 实现列序不同（procps / BSD），解析不出的字段一律省略；
/// `raw` 始终带原始输出，前端以它兜底展示。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDetail {
    /// 进程 ID（查询值回显）
    pub(crate) pid: u32,
    /// 远端是否存在该进程（解析到数据行才算存在）
    pub(crate) found: bool,
    /// 运行用户（BSD 列序下无此列）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user: Option<String>,
    /// 父进程 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ppid: Option<u32>,
    /// 控制终端（无终端为 `?`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tty: Option<String>,
    /// 启动时间列（ps 原样，如 `09:12`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started: Option<String>,
    /// 累计 CPU 时间（ps 原样，如 `00:01:23`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cpu_time: Option<String>,
    /// 完整命令行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) command: Option<String>,
    /// ps 原始输出（含报错文本，平台差异兜底）
    pub(crate) raw: String,
}
