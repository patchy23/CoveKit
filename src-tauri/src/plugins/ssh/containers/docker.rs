//! SSH 插件 · Docker 容器管理
//! docker 命令输出解析为结构化容器列表；解析函数为纯函数（可单测）。
//! docker exec 复用 PTY 通道（交互式容器终端）。

use tauri::{AppHandle, State};
use tokio::sync::{mpsc, watch};

use crate::plugins::ssh::conn::{exec_collect, get_session, resource_id, shell_quote, SshState};
use crate::plugins::ssh::models::{
    DockerContainer, SshActionResult, SshDockerExecPayload, TerminalSession,
};
use crate::plugins::ssh::terminal::log;
use crate::plugins::ssh::terminal::TerminalState;

/// 解析 docker ps 行（纯函数）：tab 分隔 ID/NAME/IMAGE/STATUS/PORTS → DockerContainer
/// （--format 用 tab 分隔，STATUS 含空格也不受影响）
fn parse_container_line(line: &str) -> Option<DockerContainer> {
    let parts: Vec<&str> = line.split('\x09').collect();
    if parts.len() < 4 {
        return None;
    }
    Some(DockerContainer {
        id: parts[0].to_string(),
        name: parts[1].to_string(),
        image: parts[2].to_string(),
        status: normalize_status(parts[3]).to_string(),
        uptime: extract_uptime(parts[3]),
        ports: parts.get(4).map(|s| s.to_string()).unwrap_or_default(),
        created_at: 0,
    })
}

/// Docker 人类可读状态归一化为前端契约值。
fn normalize_status(status: &str) -> &'static str {
    let status = status.trim().to_ascii_lowercase();
    if status.starts_with("paused") || status.contains("(paused)") {
        "paused"
    } else if status.starts_with("up ") || status == "up" {
        "running"
    } else {
        "exited"
    }
}

/// 从 Docker Status 中提取本次运行时长，并去掉 health/paused 等括号状态。
fn extract_uptime(status: &str) -> String {
    let status = status.trim();
    if !status.to_ascii_lowercase().starts_with("up ") {
        return "—".to_string();
    }
    status[3..]
        .split(" (")
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("—")
        .to_string()
}

/// 按 Compose 标签可选过滤，查询不依赖 YAML 是否存在或当前工作目录。
fn container_list_command(compose_project: Option<&str>) -> Result<String, String> {
    let filter = match compose_project {
        Some(name) if name.is_empty() || name.chars().any(char::is_control) => {
            return Err("Compose 项目名不能为空或包含控制字符".into());
        }
        Some(name) => format!(
            " --filter {}",
            shell_quote(&format!("label=com.docker.compose.project={name}"))
        ),
        None => String::new(),
    };
    Ok(format!("docker ps -a --no-trunc{filter} --format '{{{{.ID}}}}\x09{{{{.Names}}}}\x09{{{{.Image}}}}\x09{{{{.Status}}}}\x09{{{{.Ports}}}}' 2>&1"))
}

/// 非空但无法解析的响应必须报错，不能把权限或 CLI 错误伪装成没有容器。
fn parse_container_list(out: &str) -> Result<Vec<DockerContainer>, String> {
    let mut containers = out
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            parse_container_line(line).ok_or_else(|| format!("Docker 容器列表格式异常：{line}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    containers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(containers)
}

/// 容器列表，省略 compose_project 时保持 Docker 页的全量查询行为。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    compose_project: Option<String>,
) -> Result<Vec<DockerContainer>, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let command = container_list_command(compose_project.as_deref())?;
    let out = exec_collect(&session, &command).await?;
    if out.contains("Cannot connect to the Docker daemon") {
        return Err("Docker 未运行或当前用户无权限".into());
    }
    parse_container_list(&out)
}

/// 容器操作（start/stop/restart/remove）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_action(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    container_id: String,
    action: String,
) -> Result<SshActionResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<SshActionResult, String> = async {
        let session = get_session(&ssh_state, &connection_id)?;
        let action = match action.as_str() {
            "start" => "start",
            "stop" => "stop",
            "restart" => "restart",
            "remove" => "rm -f",
            _ => return Err(format!("不支持的操作: {action}")),
        };
        // 成败以退出码为准（exec_collect 对非零退出码返回 Err，错误文案含 stderr 合并输出）。
        // 成功时 docker 在 stdout 回显容器 ID，那是正常输出不是错误，不能回填 error 字段。
        match exec_collect(
            &session,
            &format!("docker {action} {}", shell_quote(&container_id)),
        )
        .await
        {
            Ok(_) => Ok(SshActionResult {
                ok: true,
                error: None,
            }),
            Err(e) => Ok(SshActionResult {
                ok: false,
                error: Some(e),
            }),
        }
    }
    .await;
    match &result {
        Ok(value) if value.ok => ::log::info!(
            "操作完成 operation=ssh_docker_action elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Ok(_) => ::log::warn!(
            "操作未完成 operation=ssh_docker_action elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => ::log::warn!(
            "操作未完成 operation=ssh_docker_action elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 容器日志（最近 N 行）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_logs(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    container_id: String,
    lines: Option<u32>,
) -> Result<serde_json::Value, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let n = lines.unwrap_or(100).clamp(1, 2_000);
    let out = exec_collect(
        &session,
        &format!("docker logs --tail {n} {}", shell_quote(&container_id)),
    )
    .await?;
    Ok(serde_json::json!({ "ok": true, "logs": out }))
}

/// 进入容器终端（PTY exec；复用终端事件通道）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_docker_exec(
    app: AppHandle,
    state: State<'_, TerminalState>,
    ssh_state: State<'_, SshState>,
    payload: SshDockerExecPayload,
) -> Result<TerminalSession, String> {
    let SshDockerExecPayload {
        connection_id,
        container_id,
        shell,
        cols,
        rows,
    } = payload;
    if cols == 0 || rows == 0 || cols > u16::MAX as u32 || rows > u16::MAX as u32 {
        return Err("终端行列数必须在 1..=65535 范围内".into());
    }
    let session = get_session(&ssh_state, &connection_id)?;

    let shell = match shell.as_str() {
        "/bin/sh" => "/bin/sh",
        "/bin/bash" => "/bin/bash",
        _ => return Err("容器终端仅支持 /bin/sh 或 /bin/bash".into()),
    };
    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(e) => {
            crate::plugins::ssh::conn::mark_session_closed(
                ssh_state.inner(),
                &connection_id,
                &session,
            );
            return Err(format!("打开通道失败: {e}"));
        }
    };
    channel
        .request_pty(false, "xterm", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY 请求失败: {e}"))?;
    channel
        .exec(
            false,
            format!("docker exec -it {} {shell}", shell_quote(&container_id)),
        )
        .await
        .map_err(|e| format!("进入容器失败: {e}"))?;

    let terminal_id = resource_id("docker-term");
    let (tx, rx) = mpsc::channel::<crate::plugins::ssh::terminal::TerminalCmd>(128);
    let (cancel, cancel_rx) = watch::channel(false);
    // 会话日志共享状态（docker 终端不开录制，占位以复用通道任务签名）
    let log = log::new_shared();

    state.0.lock().map_err(|e| e.to_string())?.insert(
        terminal_id.clone(),
        crate::plugins::ssh::terminal::TerminalHandle {
            connection_id: connection_id.clone(),
            title: format!("docker:{container_id}"),
            cols,
            rows,
            active: true,
            tx,
            cancel,
            log: log.clone(),
        },
    );

    // 后台读写任务（复用 terminal/ 公共实现）
    crate::plugins::ssh::terminal::spawn_channel_task(
        app.clone(),
        terminal_id.clone(),
        connection_id.clone(),
        channel,
        rx,
        cancel_rx,
        log,
    );

    Ok(TerminalSession {
        id: terminal_id,
        connection_id,
        title: format!("docker:{container_id}"),
        cols: cols as u16,
        rows: rows as u16,
        active: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_filter_is_quoted_and_default_keeps_all_containers() {
        assert!(!container_list_command(None).unwrap().contains("--filter"));
        let name = "project'; touch /tmp/invalid; '";
        let command = container_list_command(Some(name)).unwrap();
        assert!(command.contains(&format!(
            "--filter {}",
            shell_quote(&format!("label=com.docker.compose.project={name}"))
        )));
        assert!(command.contains("--no-trunc"));
        assert!(container_list_command(Some("")).is_err());
        assert!(container_list_command(Some("bad\nname")).is_err());
    }

    #[test]
    fn invalid_output_is_not_an_empty_container_list() {
        assert!(parse_container_list("permission denied").is_err());
        assert!(parse_container_list("").unwrap().is_empty());
        assert_eq!(
            parse_container_list("abc\tweb\tnginx\tUp 2 hours\t80/tcp\n")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn parses_container_line() {
        let c = parse_container_line(
            "abc123def456\x09mynginx\x09nginx:latest\x09Up 2 hours\x090.0.0.0:80->80/tcp",
        )
        .unwrap();
        assert_eq!(c.id, "abc123def456");
        assert_eq!(c.name, "mynginx");
        assert_eq!(c.image, "nginx:latest");
        assert_eq!(c.status, "running");
        assert_eq!(c.uptime, "2 hours");
        assert_eq!(c.ports, "0.0.0.0:80->80/tcp");
    }

    #[test]
    fn ignores_empty_line() {
        assert!(parse_container_line("").is_none());
        assert!(parse_container_line("CONTAINER ID IMAGE COMMAND").is_none());
    }

    #[test]
    fn container_status_normalization() {
        assert_eq!(normalize_status("Up 2 hours"), "running");
        assert_eq!(normalize_status("Up 2 hours (Paused)"), "paused");
        assert_eq!(normalize_status("Exited (0) 1 minute ago"), "exited");
        assert_eq!(normalize_status("Paused"), "paused");
        assert_eq!(
            extract_uptime("Up Less than a second"),
            "Less than a second"
        );
        assert_eq!(extract_uptime("Up 2 hours (healthy)"), "2 hours");
        assert_eq!(extract_uptime("Exited (0) 1 minute ago"), "—");
    }
}
