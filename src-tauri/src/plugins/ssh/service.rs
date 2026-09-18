//! SSH 插件 · systemd 服务管理
//! 列表经 systemctl list-units 输出解析；解析函数为纯函数（可单测）。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, get_session, shell_quote, SshState};
use crate::plugins::ssh::models::{SshActionResult, SystemdService};

/// 解析 systemctl 输出行（纯函数）：`unit  load  active  sub  desc` → SystemdService
fn parse_service_line(line: &str) -> Option<SystemdService> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    // systemd 单位名带扩展名（.service），表头行（UNIT ...）与空行由此过滤
    if parts.len() < 5 || !parts[0].contains('.') {
        return None;
    }
    Some(SystemdService {
        name: parts[0].to_string(),
        description: parts.get(4..).map(|s| s.join(" ")).unwrap_or_default(),
        load_state: parts[1].to_string(),
        active_state: parts[2].to_string(),
        sub_state: parts[3].to_string(),
        enabled: false,
    })
}

/// 服务列表
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_service_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    filter: Option<String>,
) -> Result<Vec<SystemdService>, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let out = exec_collect(
        &session,
        "systemctl list-units --type=service --all --no-pager --plain",
    )
    .await?;
    let mut services: Vec<SystemdService> = out
        .lines()
        .filter_map(parse_service_line)
        .filter(|s| match filter.as_deref() {
            Some("active") => s.active_state == "active",
            Some("inactive") => s.active_state == "inactive",
            Some("failed") => s.active_state == "failed",
            _ => true,
        })
        .collect();
    // 开机自启状态：list-units 输出不含此列，单独查 list-unit-files 按名字归并；
    // 查询失败直接报错，不再静默显示全「否」
    let states = exec_collect(
        &session,
        "systemctl list-unit-files --type=service --no-pager --plain",
    )
    .await
    .map_err(|e| format!("读取服务启用状态失败: {e}"))?;
    let enabled_names: std::collections::HashSet<&str> = states
        .lines()
        .filter_map(|line| {
            // 行格式：`name.service enabled`（表头 UNIT FILE ... 不含 .service 被过滤）
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(name), Some("enabled")) if name.contains('.') => Some(name),
                _ => None,
            }
        })
        .collect();
    for service in &mut services {
        service.enabled = enabled_names.contains(service.name.as_str());
    }
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(services)
}

/// 服务操作（启动/停止/重启）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_service_action(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    service_name: String,
    action: String,
) -> Result<SshActionResult, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let action = match action.as_str() {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        _ => return Err(format!("不支持的操作: {action}")),
    };
    // 成败以退出码为准（exec_collect 对非零退出码返回 Err，错误文案含 stderr 合并输出），
    // 不再按 stdout 文本猜测
    match exec_collect(
        &session,
        &format!("systemctl {action} {}", shell_quote(&service_name)),
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

/// 服务日志（journalctl 最近 N 行）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_service_logs(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    service_name: String,
    lines: Option<u32>,
) -> Result<serde_json::Value, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let n = lines.unwrap_or(100).clamp(1, 2_000);
    let out = exec_collect(
        &session,
        &format!(
            "journalctl -u {} -n {n} --no-pager",
            shell_quote(&service_name)
        ),
    )
    .await?;
    Ok(serde_json::json!({ "ok": true, "logs": out }))
}

fn service_config_command(name: &str) -> Result<String, String> {
    if name.len() > 255
        || !name.ends_with(".service")
        || name.starts_with('-')
        || name.len() <= ".service".len()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:@-\\".contains(&byte))
    {
        return Err("服务名称无效，请从服务列表选择".into());
    }
    Ok(format!("systemctl --no-pager cat -- {}", shell_quote(name)))
}

/// 只读查看 systemd 主 unit 与 drop-in 配置，不提权、不修改文件。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_service_config(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    service_name: String,
) -> Result<String, String> {
    let command = service_config_command(&service_name)?;
    let session = get_session(&ssh_state, &connection_id)?;
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        exec_collect(&session, &command),
    )
    .await
    .map_err(|_| "读取服务配置超时，请重试".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_command_is_read_only_and_rejects_extra_shell_arguments() {
        assert_eq!(
            service_config_command("worker@one.service").unwrap(),
            "systemctl --no-pager cat -- 'worker@one.service'"
        );
        for invalid in [
            "-bad.service",
            "x;touch /tmp/test.service",
            "x\n.service",
            "../x.service",
            "*.service",
            ".service",
        ] {
            assert!(service_config_command(invalid).is_err());
        }
    }

    #[test]
    fn parses_service_line() {
        let s = parse_service_line("nginx.service loaded active running Nginx web server").unwrap();
        assert_eq!(s.name, "nginx.service");
        assert_eq!(s.active_state, "active");
        assert_eq!(s.sub_state, "running");
        assert_eq!(s.description, "Nginx web server");
    }

    #[test]
    fn ignores_header_and_empty_lines() {
        assert!(parse_service_line("UNIT LOAD ACTIVE SUB DESCRIPTION").is_none());
        assert!(parse_service_line("").is_none());
    }
}
