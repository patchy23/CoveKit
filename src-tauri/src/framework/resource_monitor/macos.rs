//! macOS 系统进程快照适配；WebKit XPC 未必属于进程树，因此明确声明覆盖边界。

use super::{descendants, ProcessSample, ResourceSnapshot};

fn cpu_seconds(text: &str) -> Result<f64, String> {
    let mut seconds = 0.0;
    for part in text.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().map_err(|_| "CPU 时间格式无法识别")?;
    }
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("CPU 时间无效".into());
    }
    Ok(seconds)
}

/// 使用系统 ps 的固定字段与 C locale；不解析命令正文，不启动 shell。
pub(super) async fn sample() -> Result<ResourceSnapshot, String> {
    let child = tokio::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,rss=,time=,lstart=,comm="])
        .env("LC_ALL", "C")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("无法启动系统资源查询：{e}"))?;
    let sampler_pid = child.id();
    // 超时只终止本次创建的 ps；不查杀其他进程，不留下无归属的查询任务。
    let output = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait_with_output())
        .await
        .map_err(|_| "系统资源查询超时，请重试")?
        .map_err(|e| format!("系统资源查询失败：{e}"))?;
    if !output.status.success() {
        return Err("系统资源查询返回失败，请重试".into());
    }
    let text = String::from_utf8(output.stdout).map_err(|_| "系统资源查询编码无法识别")?;
    let root = std::process::id();
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 9 {
            return Err("系统资源查询字段不完整".into());
        }
        let pid: u32 = fields[0].parse().map_err(|_| "进程编号无效")?;
        if Some(pid) == sampler_pid {
            continue;
        }
        let parent: u32 = fields[1].parse().map_err(|_| "父进程编号无效")?;
        let resident_kib: u64 = fields[2].parse().map_err(|_| "进程内存格式无效")?;
        let identity = fields[4..9].join(" ");
        let started = chrono::NaiveDateTime::parse_from_str(&identity, "%a %b %e %T %Y")
            .map_err(|_| "进程启动时间格式无法识别")?;
        rows.push((
            parent,
            started,
            ProcessSample {
                pid,
                identity,
                kind: if pid == root { "main" } else { "child" },
                resident_bytes: resident_kib.checked_mul(1024).ok_or("内存计数溢出")?,
                private_bytes: None,
                cpu_seconds: cpu_seconds(fields[3])?,
                threads: None,
                handles: None,
            },
        ));
    }
    if !rows.iter().any(|(_, _, row)| row.pid == root) {
        return Err("无法读取应用主进程资源".into());
    }
    let starts: std::collections::HashMap<_, _> = rows
        .iter()
        .map(|(_, started, row)| (row.pid, started))
        .collect();
    let parents: Vec<_> = rows
        .iter()
        .filter_map(|(parent, started, row)| {
            starts
                .get(parent)
                .filter(|parent_started| **parent_started <= started)
                .map(|_| (row.pid, *parent))
        })
        .collect();
    let selected = descendants(root, &parents);
    Ok(ResourceSnapshot {
        partial: true,
        processes: rows.into_iter().filter(|(_, _, row)| selected.contains(&row.pid)).map(|(_, _, row)| row).collect(),
        logical_cpus: std::thread::available_parallelism().map_err(|e| format!("CPU 核心数读取失败：{e}"))?.get(),
        coverage: "仅统计主进程及可归属的子进程；macOS 的 WebKit XPC 进程未完整覆盖，此数值不是应用完整总量。内存为 RSS 合计，共享页可能重复计算。",
        missing_processes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::cpu_seconds;
    #[test]
    fn parses_cumulative_cpu_and_rejects_invalid_samples() {
        assert_eq!(cpu_seconds("02:03.50").unwrap(), 123.5);
        assert_eq!(cpu_seconds("1:02:03").unwrap(), 3723.0);
        assert!(cpu_seconds("nan").is_err());
        assert!(cpu_seconds("-1").is_err());
    }
}
