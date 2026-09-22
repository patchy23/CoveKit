//! 应用资源只读快照：按进程树采集，不启动持续任务、不操作业务资源。

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use serde::Serialize;

/// 单进程原始计数；CPU 由消费方按同一进程身份的相邻样本计算。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSample {
    pub pid: u32,
    /// 启动标识防止 PID 复用造成 CPU 突增，不用于业务身份校验。
    pub identity: String,
    pub kind: &'static str,
    /// 工作集/RSS 字节；跨进程相加可能重复包含共享页。
    pub resident_bytes: u64,
    /// 私有工作集字节；不含共享页，系统不支持时为空。
    pub private_resident_bytes: Option<u64>,
    /// 私有提交字节；平台不可用时为空，不伪装为零。
    pub private_bytes: Option<u64>,
    /// 累计用户态与内核态 CPU 秒数，不包含等待时间。
    pub cpu_seconds: f64,
    pub threads: Option<u32>,
    pub handles: Option<u32>,
}

/// 本次快照；缺失进程和平台覆盖限制必须在界面可见。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    /// 展示内存的统一口径：Windows 私有工作集，macOS RSS。
    pub memory_metric: &'static str,
    /// 平台本身无法完整覆盖应用进程时为真。
    pub partial: bool,
    pub processes: Vec<ProcessSample>,
    pub logical_cpus: usize,
    pub coverage: &'static str,
    pub missing_processes: usize,
}

/// 父子关系闭包，避免仅按同名进程把其他应用的 WebView 纳入统计。
pub(super) fn descendants(root: u32, parents: &[(u32, u32)]) -> std::collections::BTreeSet<u32> {
    let mut found = std::collections::BTreeSet::from([root]);
    loop {
        let before = found.len();
        for &(pid, parent) in parents {
            if found.contains(&parent) {
                found.insert(pid);
            }
        }
        if before == found.len() {
            return found;
        }
    }
}

/// 读取本应用当前进程快照；不创建持续任务，调用结束即释放查询资源。
#[tauri::command]
pub async fn resource_monitor_snapshot() -> Result<ResourceSnapshot, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(windows::sample)
            .await
            .map_err(|error| format!("资源采样任务失败：{error}"))?
    }
    #[cfg(target_os = "macos")]
    {
        macos::sample().await
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err("当前平台不支持应用资源采样".into())
    }
}

#[cfg(test)]
mod tests {
    use super::descendants;

    #[test]
    fn descendants_excludes_other_apps_and_handles_out_of_order_rows() {
        let ids = descendants(10, &[(12, 11), (99, 1), (11, 10), (13, 99)]);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![10, 11, 12]);
    }
}
