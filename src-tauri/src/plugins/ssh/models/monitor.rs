//! SSH 数据模型 · monitor

use serde::Serialize;

/* ── 资源监控 ── */

/// 监控数据点
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorData {
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存已用（字节）
    pub(crate) memory_used: u64,
    /// 内存总量（字节）
    pub(crate) memory_total: u64,
    /// 磁盘使用率 0-100
    pub(crate) disk_percent: f32,
    /// 磁盘已用（字节）
    pub(crate) disk_used: u64,
    /// 磁盘总量（字节）
    pub(crate) disk_total: u64,
    /// 网络上行速率（字节/秒）
    pub(crate) net_upload_bps: u64,
    /// 网络下行速率（字节/秒）
    pub(crate) net_download_bps: u64,
    /// 采样时间（毫秒时间戳）
    pub(crate) timestamp: u64,
}
