//! SSH 插件 · systemd 服务管理
//! 列表与分类信息通过一次 systemctl show 获取；解析函数为纯函数（可单测）。

use tauri::State;

use crate::plugins::ssh::conn::{exec_collect, get_session, shell_quote, SshState};
use crate::plugins::ssh::models::{SshActionResult, SystemdService};

/// 限定属性减少传输量；glob 由 systemctl 展开，覆盖已加载的运行和停止服务。
const SERVICE_LIST_COMMAND: &str = "systemctl show '*.service' --all --no-pager --property=Id,Description,LoadState,ActiveState,SubState,UnitFileState,FragmentPath,DropInPaths";

/// 服务列表
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_service_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    filter: Option<String>,
) -> Result<Vec<SystemdService>, String> {
    let session = get_session(&ssh_state, &connection_id)?;
    let out = exec_collect(&session, SERVICE_LIST_COMMAND).await?;
    let mut services: Vec<SystemdService> = parse_service_properties(&out)?
        .into_iter()
        .filter(|s| match filter.as_deref() {
            Some("active") => s.active_state == "active",
            Some("inactive") => s.active_state == "inactive",
            Some("failed") => s.active_state == "failed",
            _ => true,
        })
        .collect();
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(services)
}

/// 属性输出以空行分块，顺序不固定；必需状态缺失时报错，不伪装为空列表。
fn parse_service_properties(output: &str) -> Result<Vec<SystemdService>, String> {
    let normalized = output.replace("\r\n", "\n");
    let mut services = Vec::new();
    for block in normalized
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
    {
        let fields: std::collections::HashMap<_, _> = block
            .lines()
            .filter_map(|line| line.trim().split_once('='))
            .collect();
        let required = |key: &str| -> Result<&str, String> {
            fields
                .get(key)
                .copied()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("服务列表缺少 {key} 属性，请检查远端 systemctl 输出"))
        };
        let name = required("Id")?;
        if !name.ends_with(".service") {
            continue;
        }
        services.push(SystemdService {
            name: name.to_string(),
            description: fields
                .get("Description")
                .copied()
                .unwrap_or_default()
                .to_string(),
            load_state: required("LoadState")?.to_string(),
            active_state: required("ActiveState")?.to_string(),
            sub_state: required("SubState")?.to_string(),
            // 保留既有 DTO，不再为此字段单独遍历全部 unit 文件。
            enabled: fields
                .get("UnitFileState")
                .is_some_and(|state| *state == "enabled"),
            fragment_path: fields
                .get("FragmentPath")
                .filter(|path| !path.is_empty())
                .map(|path| (*path).to_string()),
            has_overrides: fields
                .get("DropInPaths")
                .is_some_and(|paths| !paths.is_empty()),
        });
    }
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
    fn parses_service_properties_in_any_order() {
        let services = parse_service_properties("Description=Nginx web server a=b\nSubState=running\nId=nginx.service\nActiveState=active\nLoadState=loaded\nUnitFileState=enabled\nFragmentPath=/usr/lib/systemd/system/nginx.service\nDropInPaths=\n").unwrap();
        let s = &services[0];
        assert_eq!(s.name, "nginx.service");
        assert_eq!(s.active_state, "active");
        assert_eq!(s.sub_state, "running");
        assert_eq!(s.description, "Nginx web server a=b");
        assert!(s.enabled);
        assert!(!s.has_overrides);
        assert_eq!(
            s.fragment_path.as_deref(),
            Some("/usr/lib/systemd/system/nginx.service")
        );
    }

    #[test]
    fn multiple_services_preserve_inactive_failed_and_unknown_sources() {
        let services = parse_service_properties("Id=cron.service\r\nLoadState=loaded\r\nActiveState=inactive\r\nSubState=dead\r\nFragmentPath=/lib/systemd/system/cron.service\r\nDropInPaths=/etc/systemd/system/cron.service.d/custom.conf\r\n\r\nId=worker.service\r\nLoadState=not-found\r\nActiveState=failed\r\nSubState=failed\r\nFragmentPath=\r\nUnitFileState=disabled\r\n").unwrap();
        assert_eq!(services.len(), 2);
        assert_eq!(
            services[0].fragment_path.as_deref(),
            Some("/lib/systemd/system/cron.service")
        );
        assert!(services[0].has_overrides);
        assert_eq!(services[0].active_state, "inactive");
        assert!(services[1].fragment_path.is_none());
        assert_eq!(services[1].active_state, "failed");
        assert!(!services[1].enabled);
    }

    #[test]
    fn empty_list_is_valid_but_malformed_properties_fail() {
        assert!(parse_service_properties("\n\n").unwrap().is_empty());
        assert!(parse_service_properties("Id=broken.service\nDescription=Incomplete").is_err());
        assert!(parse_service_properties("unexpected output").is_err());
    }
}
