//! SSH 数据模型 · 远程系统信息与磁盘分区明细（与前端 contracts/monitor.ts 逐字段对应）

use serde::Serialize;

/// 系统信息采集结果（`ssh_system_info_get` 出参：信息 + 磁盘两部分）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSystemInfoResult {
    /// 系统信息（主机名/发行版/内核/运行时长/负载/核数）
    pub(crate) info: SshSystemInfo,
    /// 磁盘分区明细（保持 df 原始顺序，排序由前端完成）
    pub(crate) disks: Vec<SshDiskEntry>,
}

/// 远程系统信息（缺失字段留空字符串或 0，前端统一显示 `-`）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSystemInfo {
    /// 主机名（`hostname`，取首个非空行）
    pub(crate) hostname: String,
    /// 发行版名称（`/etc/os-release` 的 PRETTY_NAME，回退 NAME）
    pub(crate) os_name: String,
    /// 内核信息（`uname -srm`，形如 `Linux 5.15.0-91-generic x86_64`）
    pub(crate) kernel: String,
    /// 运行时长文本（从 `uptime` 的 `up` 段提取，如 `42 days, 20:05`）
    pub(crate) uptime_text: String,
    /// 1/5/15 分钟平均负载；uptime 段缺失时为全 0
    pub(crate) load_avg: [f64; 3],
    /// 逻辑 CPU 核数（`nproc`）；命令不可用时为 0
    pub(crate) cpu_cores: u32,
}

/// 磁盘分区条目（对应 `df -hlPT` 的一条数据行）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiskEntry {
    /// 文件系统名（`/dev/sda1`、`overlay`、`tmpfs` 等）
    pub(crate) filesystem: String,
    /// 文件系统类型（`ext4`、`xfs`、`tmpfs`、`overlay`）
    pub(crate) fs_type: String,
    /// 总量（`df -h` 人类可读原文，如 `50G`）
    pub(crate) size_text: String,
    /// 已用（人类可读原文，如 `12G`）
    pub(crate) used_text: String,
    /// 可用（人类可读原文，如 `35G`）
    pub(crate) avail_text: String,
    /// 使用率百分比 0-100（df 的 `-` 视为 0）
    pub(crate) use_percent: f64,
    /// 挂载点（含空格的路径原样保留）
    pub(crate) mount_point: String,
}
