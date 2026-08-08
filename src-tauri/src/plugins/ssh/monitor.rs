//! SSH 插件 · 资源监控（CPU/内存/磁盘/网络）
//! 一条组合命令采集（top/free/df/net），解析为结构化数据；
//! 解析函数为纯函数（parse_*），可独立单测。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, SshState};
use crate::plugins::ssh::models::MonitorData;

/// 采集命令（一次 exec 完成四项采集）
const COLLECT_CMD: &str =
    "top -bn1 | head -3; echo ---; free -b; echo ---; df -B1 /; echo ---; cat /proc/net/dev";

/// 解析 top 首行 CPU 使用率（纯函数）：`%Cpu(s): 12.3 us, ...` → 12.3
fn parse_cpu(line: &str) -> f64 {
    let Some(pos) = line.find("us,") else {
        return 0.0;
    };
    let before = &line[..pos];
    let Some(colon) = before.rfind(':') else {
        return 0.0;
    };
    before[colon + 1..].trim().parse().unwrap_or(0.0)
}

/// 解析 free -b 内存行（纯函数）：`Mem:   total  used  free` → (total, used)
fn parse_mem(line: &str) -> (u64, u64) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let total = parts[1].parse().unwrap_or(0);
        let used = parts[2].parse().unwrap_or(0);
        (total, used)
    } else {
        (0, 0)
    }
}

/// 解析 df -B1 磁盘行（纯函数）：`/dev/sda1  total used avail ...` → (total, used)
fn parse_disk(line: &str) -> (u64, u64) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let total = parts[1].parse().unwrap_or(0);
        let used = parts[2].parse().unwrap_or(0);
        (total, used)
    } else {
        (0, 0)
    }
}

/// 解析 /proc/net/dev（纯函数）：汇总所有接口收发字节
/// 输入为去掉头部两行的行列表，返回 (下行速率占位用累计下行, 上行累计)
fn parse_net(lines: &[&str]) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in lines {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let parts: Vec<&str> = line[colon + 1..].split_whitespace().collect();
        if parts.len() >= 9 {
            rx += parts[0].parse().unwrap_or(0);
            tx += parts[8].parse().unwrap_or(0);
        }
    }
    (rx, tx)
}

/// 组合输出 → MonitorData（纯函数，供命令与单测共用）
fn parse_monitor_output(out: &str) -> MonitorData {
    let sections: Vec<&str> = out.split("---").collect();
    let cpu = parse_cpu(sections.first().unwrap_or(&""));
    let (mem_total, mem_used) = sections
        .get(1)
        .and_then(|s| s.lines().find(|l| l.starts_with("Mem:")))
        .map(parse_mem)
        .unwrap_or((0, 0));
    let (disk_total, disk_used) = sections
        .get(2)
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with('/') && !l.starts_with("//"))
        })
        .map(parse_disk)
        .unwrap_or((0, 0));
    let (_net_rx, _net_tx) = sections
        .get(3)
        .map(|s| {
            let lines: Vec<&str> = s.lines().skip(2).collect();
            parse_net(&lines)
        })
        .unwrap_or((0, 0));

    let mem_pct = if mem_total > 0 {
        (mem_used as f64 / mem_total as f64 * 100.0).round()
    } else {
        0.0
    };
    let disk_pct = if disk_total > 0 {
        (disk_used as f64 / disk_total as f64 * 100.0).round()
    } else {
        0.0
    };

    MonitorData {
        cpu_percent: cpu as f32,
        memory_percent: mem_pct as f32,
        memory_used: mem_used,
        memory_total: mem_total,
        disk_percent: disk_pct as f32,
        disk_used,
        disk_total,
        net_upload_bps: 0,
        net_download_bps: 0,
        timestamp: crate::plugins::ssh::conn::now_ms(),
    }
}

/// 获取监控数据（轮询调用，前端 3s 间隔）
#[tauri::command]
pub async fn ssh_monitor_get(
    ssh_state: State<'_, SshState>,
    connection_id: String,
) -> Result<MonitorData, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let out = exec_collect(&session, COLLECT_CMD).await?;
    Ok(parse_monitor_output(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析cpu使用率() {
        let v = parse_cpu("%Cpu(s): 12.3 us,  0.5 sy,  0.0 ni, 86.8 id,  0.0 wa");
        assert!((v - 12.3).abs() < 0.01);
    }

    #[test]
    fn 解析内存与磁盘() {
        let (t, u) = parse_mem("Mem:   16000000000  8000000000  8000000000");
        assert_eq!((t, u), (16000000000, 8000000000));
        let (t, u) = parse_disk("/dev/sda1  500000000000  250000000000  250000000000");
        assert_eq!((t, u), (500000000000, 250000000000));
    }

    #[test]
    fn 解析网络计数() {
        let lines = ["  eth0: 100 2 0 0 0 0 0 0 50 1 0 0 0 0 0 0"];
        let (rx, tx) = parse_net(&lines);
        assert_eq!(rx, 100);
        assert_eq!(tx, 50);
    }

    #[test]
    fn 组合输出解析为监控数据() {
        let out = format!(
            "%Cpu(s):  5.0 us,  0.0 sy, 95.0 id\n---\nMem:   100 40 60\n---\n/dev/sda1  200 80 120\n---\nInter-| Receive\n eth0: 10 0 0 0 0 0 0 0 5 0 0 0 0 0 0 0"
        );
        let d = parse_monitor_output(&out);
        assert!((d.cpu_percent - 5.0).abs() < 0.01);
        assert_eq!(d.memory_total, 100);
        assert_eq!(d.memory_used, 40);
        assert_eq!(d.disk_total, 200);
        assert_eq!(d.disk_used, 80);
    }
}
