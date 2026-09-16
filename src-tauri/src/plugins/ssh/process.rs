//! SSH 插件 · 进程管理
//! ps 输出解析为结构化进程列表与单进程详情；解析函数为纯函数（可单测）。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, get_session, SshState};
use crate::plugins::ssh::models::{ProcessDetail, ProcessInfo, SshActionResult};

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
    let session = get_session(&ssh_state, &connection_id)?;
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
    let session = get_session(&ssh_state, &connection_id)?;
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

/// 解析 `ps -fp` 输出为进程详情（纯函数）。
///
/// 远端 ps 实现列序不同，按「前两列是否等于查询的 pid」判定：
/// - procps（Linux）：`UID PID PPID C STIME TTY TIME CMD`
/// - BSD（macOS）：`PID TT STAT TIME COMMAND`
///
/// 判不出列序时只回原始输出、`found=false`，不猜字段语义。
fn parse_process_detail(pid: u32, out: &str) -> ProcessDetail {
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut detail = ProcessDetail {
        pid,
        found: false,
        user: None,
        ppid: None,
        tty: None,
        started: None,
        cpu_time: None,
        command: None,
        raw: lines.join("\n"),
    };
    let mut matched: Option<Vec<&str>> = None;
    for line in &lines {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 5 && cols[..2].iter().any(|c| c.parse::<u32>() == Ok(pid)) {
            matched = Some(cols);
            break;
        }
    }
    let Some(cols) = matched else {
        return detail;
    };
    if cols.len() >= 8 && cols[1].parse::<u32>().ok() == Some(pid) {
        // procps：UID PID PPID C STIME TTY TIME CMD
        detail.found = true;
        detail.user = Some(cols[0].to_string());
        detail.ppid = cols[2].parse().ok();
        detail.started = Some(cols[4].to_string());
        detail.tty = Some(cols[5].to_string());
        detail.cpu_time = Some(cols[6].to_string());
        detail.command = Some(cols[7..].join(" "));
    } else if cols[0].parse::<u32>().ok() == Some(pid) {
        // BSD：PID TT STAT TIME COMMAND
        detail.found = true;
        detail.tty = Some(cols[1].to_string());
        detail.cpu_time = Some(cols[3].to_string());
        detail.command = Some(cols[4..].join(" "));
    }
    detail
}

/// 单进程详情（`ps -fp`）
///
/// 进程不存在属于预期结果：命令末尾吞掉退出码（`exec_collect` 把非零退出码当错误），
/// 是否存在由解析结果 `found` 判定，ps 的报错文本留在 `raw` 里给用户看。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_process_detail(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    pid: u32,
) -> Result<ProcessDetail, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    // `-w` 关列宽截断以免 CMD 列被切；不支持 `-w` 的实现退回不带 `-w` 的调用
    let cmd = format!("ps -fp {pid} -w 2>&1 || ps -fp {pid} 2>&1 || true");
    let out = exec_collect(&session, &cmd).await?;
    Ok(parse_process_detail(pid, &out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_process_line() {
        let p = parse_process_line("1234 root 1.5 0.2 65536 /usr/bin/nginx -g daemon off").unwrap();
        assert_eq!(p.pid, 1234);
        assert_eq!(p.user, "root");
        assert!((p.cpu_percent - 1.5).abs() < 0.001);
        assert_eq!(p.memory_bytes, 65536 * 1024);
        assert_eq!(p.command, "/usr/bin/nginx -g daemon off");
    }

    #[test]
    fn ignores_header() {
        assert!(parse_process_line("PID USER %CPU %MEM RSS COMMAND").is_none());
    }

    #[test]
    fn parses_procps_detail() {
        let out = "UID        PID  PPID  C STIME TTY          TIME CMD\n\
                   root      1234     1  0 09:12 ?        00:01:23 /usr/bin/nginx -g daemon off\n";
        let d = parse_process_detail(1234, out);
        assert!(d.found);
        assert_eq!(d.user.as_deref(), Some("root"));
        assert_eq!(d.ppid, Some(1));
        assert_eq!(d.tty.as_deref(), Some("?"));
        assert_eq!(d.started.as_deref(), Some("09:12"));
        assert_eq!(d.cpu_time.as_deref(), Some("00:01:23"));
        assert_eq!(d.command.as_deref(), Some("/usr/bin/nginx -g daemon off"));
        assert!(d.raw.contains("/usr/bin/nginx"));
    }

    #[test]
    fn parses_bsd_detail() {
        let out = "  PID TT  STAT    TIME COMMAND\n 4321 ??  Ss   0:00.12 /usr/sbin/cupsd -l\n";
        let d = parse_process_detail(4321, out);
        assert!(d.found);
        assert_eq!(d.tty.as_deref(), Some("??"));
        assert_eq!(d.cpu_time.as_deref(), Some("0:00.12"));
        assert_eq!(d.command.as_deref(), Some("/usr/sbin/cupsd -l"));
        // BSD 列序没有用户与父进程列
        assert_eq!(d.user, None);
        assert_eq!(d.ppid, None);
    }

    #[test]
    fn reports_missing_process() {
        let empty = parse_process_detail(99999, "");
        assert!(!empty.found);
        assert!(empty.raw.is_empty());
        // ps 报错文本保留在 raw，供前端提示
        let err = parse_process_detail(99999, "ps: 99999: No such process\n");
        assert!(!err.found);
        assert_eq!(err.raw, "ps: 99999: No such process");
    }

    #[test]
    fn keeps_raw_when_layout_unknown() {
        let d = parse_process_detail(7, "7 some weird output\n");
        assert!(!d.found);
        assert_eq!(d.raw, "7 some weird output");
    }
}
