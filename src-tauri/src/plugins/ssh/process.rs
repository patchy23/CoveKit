//! SSH 插件 · 进程管理
//! ps 输出解析为结构化进程列表；解析函数为纯函数（可单测）。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, SshState};
use crate::plugins::ssh::models::{ProcessInfo, SshActionResult};

/// 解析 ps 行（纯函数）：
/// `PID USER %CPU %MEM RSS STARTED COMMAND` → ProcessInfo
fn parse_process_line(line: &str) -> Option<ProcessInfo> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }
    Some(ProcessInfo {
        pid: parts[0].parse().ok()?,
        user: parts[1].to_string(),
        cpu_percent: parts[2].parse().unwrap_or(0.0),
        memory_percent: parts[3].parse().unwrap_or(0.0),
        memory_bytes: parts[4].parse().unwrap_or(0) * 1024,
        started_at: 0,
        command: parts[5..].join(" "),
    })
}

/// 进程列表（按 CPU 排序）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_process_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    sort_by: Option<String>,
    keyword: Option<String>,
) -> Result<Vec<ProcessInfo>, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let out = exec_collect(&session, "ps -eo pid,user,%cpu,%mem,rss,args --sort=-%cpu").await?;
    let mut procs: Vec<ProcessInfo> = out.lines().skip(1).filter_map(parse_process_line).collect();
    if let Some(kw) = keyword {
        let kw = kw.to_lowercase();
        procs.retain(|p| {
            p.command.to_lowercase().contains(&kw) || p.user.to_lowercase().contains(&kw)
        });
    }
    match sort_by.as_deref() {
        Some("memory") => procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
        Some("pid") => procs.sort_by(|a, b| a.pid.cmp(&b.pid)),
        _ => procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
    }
    Ok(procs)
}

/// 结束进程（默认 SIGTERM，force 时 SIGKILL）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_process_kill(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    pid: u32,
    force: Option<bool>,
) -> Result<SshActionResult, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let cmd = if force.unwrap_or(false) {
        format!("kill -9 {pid} 2>&1")
    } else {
        format!("kill {pid} 2>&1")
    };
    let out = exec_collect(&session, &cmd).await?;
    Ok(SshActionResult {
        ok: out.trim().is_empty(),
        error: if out.trim().is_empty() {
            None
        } else {
            Some(out.trim().to_string())
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析进程行() {
        let p = parse_process_line("1234 root 1.5 0.2 65536 /usr/bin/nginx -g daemon off").unwrap();
        assert_eq!(p.pid, 1234);
        assert_eq!(p.user, "root");
        assert!((p.cpu_percent - 1.5).abs() < 0.001);
        assert_eq!(p.memory_bytes, 65536 * 1024);
        assert_eq!(p.command, "/usr/bin/nginx -g daemon off");
    }

    #[test]
    fn 忽略表头() {
        assert!(parse_process_line("PID USER %CPU %MEM RSS COMMAND").is_none());
    }
}
