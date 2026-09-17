//! Docker Compose 管理：仅查询 Docker 项目清单，不扫描目录。
//! 操作白名单、显式项目与配置顺序，退出码不得从输出文本推测。

use std::time::Duration;

use russh::ChannelMsg;
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use tauri::State;
use tokio::io::AsyncWriteExt;

use super::conn::{get_session, get_sftp_session, shell_quote, SshState};
use super::models::{ComposeOutput, ComposeProject};

const MAX_OUTPUT: usize = 2 * 1024 * 1024;
const MAX_CONFIG: usize = 1024 * 1024;

/// 远端为 POSIX 路径，不使用客户端平台的 Path 分隔规则。
fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.ends_with('/') || path.chars().any(char::is_control) {
        return Err("请使用远程配置文件的绝对路径，不能包含控制字符".into());
    }
    Ok(())
}

fn compose_command(project: &ComposeProject, action: &str) -> Result<String, String> {
    let name = &project.name;
    if name.is_empty()
        || !name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err("项目名只能以小写字母或数字开头，包含小写字母、数字、下划线和短横线".into());
    }
    if project.config_files.is_empty() || project.config_files.len() > 32 {
        return Err("项目必须包含 1 至 32 个配置文件".into());
    }
    let mut files = String::new();
    for path in &project.config_files {
        validate_path(path)?;
        files.push_str(&format!(" -f {}", shell_quote(path)));
    }
    let first = &project.config_files[0];
    let (directory, _) = first.rsplit_once('/').ok_or("配置路径无效")?;
    let directory = if directory.is_empty() { "/" } else { directory };
    let args = match action {
        "up" => "up -d",
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "down" => "down",
        "pull" => "pull",
        "build" => "build",
        "ps" => "ps --all",
        "logs" => "logs --no-color --tail 200",
        "config" => "config --quiet",
        _ => return Err("不支持的 Compose 操作".into()),
    };
    Ok(format!(
        "cd {} && docker compose --ansi never --project-directory {} -p {}{} {}",
        shell_quote(directory),
        shell_quote(directory),
        shell_quote(name),
        files,
        args
    ))
}

/// stdout 单独解析，Docker 在 stderr 输出警告不能污染 JSON。
fn parse_projects(raw: &str) -> Result<Vec<ComposeProject>, String> {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(raw).map_err(|e| format!("Compose 项目列表不是有效 JSON：{e}"))?;
    rows.into_iter()
        .map(|row| {
            let name = row
                .get("Name")
                .and_then(|v| v.as_str())
                .ok_or("Compose 项目缺少名称")?;
            let status = row
                .get("Status")
                .and_then(|v| v.as_str())
                .ok_or("Compose 项目缺少状态")?;
            let config = row
                .get("ConfigFiles")
                .and_then(|v| v.as_str())
                .ok_or("Compose 项目缺少配置文件信息")?;
            Ok(ComposeProject {
                name: name.into(),
                status: status.into(),
                config_files: config
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
        })
        .collect()
}

/// 有界命令收集：EOF 不是退出状态；等待 Close 后仍无退出码必须报错。
async fn execute(
    state: &State<'_, SshState>,
    id: &str,
    command: &str,
    seconds: u64,
) -> Result<ComposeOutput, String> {
    let session = get_session(state, id)?;
    let mut channel = tokio::time::timeout(Duration::from_secs(15), session.channel_open_session())
        .await
        .map_err(|_| "打开 Compose 通道超时".to_string())?
        .map_err(|e| format!("打开 Compose 通道失败：{e}"))?;
    let result = tokio::time::timeout(Duration::from_secs(seconds), async {
        channel
            .exec(true, command)
            .await
            .map_err(|e| format!("Compose 执行失败：{e}"))?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                ChannelMsg::Close => break,
                _ => {}
            }
            if stdout.len() + stderr.len() > MAX_OUTPUT {
                return Err("Compose 输出超过 2 MiB，已停止接收；远端操作状态请重新查询".into());
            }
        }
        Ok(ComposeOutput {
            exit_code: exit_code.ok_or("Compose 通道关闭但未收到退出码，请重新查询项目状态")?,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    })
    .await;
    // 无论命令结果如何都释放本次通道；不能声称关闭通道已撤销远端部署。
    let _ = tokio::time::timeout(Duration::from_secs(2), channel.close()).await;
    result.map_err(|_| "Compose 操作等待超时；远端可能仍在执行，请查询状态后再操作".to_string())?
}

/// 查询含已停止容器的 Compose 项目，禁止目录扫描。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_compose_list(
    ssh_state: State<'_, SshState>,
    connection_id: String,
) -> Result<Vec<ComposeProject>, String> {
    let out = execute(
        &ssh_state,
        &connection_id,
        "docker compose ls --all --format json",
        30,
    )
    .await?;
    if out.exit_code != 0 {
        return Err(format!(
            "Compose 查询失败（{}）：{}{}",
            out.exit_code, out.stderr, out.stdout
        ));
    }
    parse_projects(&out.stdout)
}

/// 项目级编排操作；拆除不删除卷，不隐式附加 --remove-orphans。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_compose_action(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    project: ComposeProject,
    action: String,
) -> Result<ComposeOutput, String> {
    let command = compose_command(&project, &action)?;
    execute(&ssh_state, &connection_id, &command, 600).await
}

/// 创建新的远程 YAML，SFTP 独占创建保证存在时拒绝覆盖；父目录须已存在。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_compose_create(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    content: String,
) -> Result<(), String> {
    validate_path(&remote_path)?;
    if content.is_empty() || content.len() > MAX_CONFIG {
        return Err("Compose 配置内容不能为空且不得超过 1 MiB".into());
    }
    let sftp = get_sftp_session(&ssh_state, &connection_id).await?;
    let mut file = sftp
        .open_with_flags_and_attributes(
            &remote_path,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            FileAttributes {
                permissions: Some(0o600),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("新建配置失败，请检查父目录、权限及同名文件：{e}"))?;
    let result = async {
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        file.shutdown().await
    }
    .await;
    if let Err(error) = result {
        return match sftp.remove_file(&remote_path).await {
            Ok(_) => Err(format!("写入失败，已清理本次新建文件：{error}")),
            Err(cleanup) => Err(format!(
                "写入失败：{error}；清理失败：{cleanup}，请检查 {remote_path}"
            )),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_json_keeps_file_order_and_stopped_projects() {
        let projects = parse_projects(r#"[{"Name":"app","Status":"exited(2)","ConfigFiles":"/opt/a/compose.yaml,/opt/a/prod.yaml"}]"#).unwrap();
        assert_eq!(
            projects[0].config_files,
            ["/opt/a/compose.yaml", "/opt/a/prod.yaml"]
        );
        assert!(parse_projects("permission denied").is_err());
        assert!(parse_projects("[{}]").is_err());
        assert!(parse_projects("[]").unwrap().is_empty());
    }

    #[test]
    fn command_quotes_paths_and_rejects_injected_actions() {
        let mut project = ComposeProject {
            name: "app".into(),
            status: String::new(),
            config_files: vec!["/opt/a b/compose.yaml".into(), "/opt/a b/it's.yaml".into()],
        };
        let command = compose_command(&project, "up").unwrap();
        assert!(command.contains(r#"-f '/opt/a b/compose.yaml' -f '/opt/a b/it'"'"'s.yaml'"#));
        assert!(command.ends_with("up -d"));
        assert!(compose_command(&project, "down; id").is_err());
        assert!(!compose_command(&project, "down")
            .unwrap()
            .contains("--volumes"));
        project.name = "$(id)".into();
        assert!(compose_command(&project, "up").is_err());
        assert!(validate_path("relative.yaml").is_err());
    }
}
