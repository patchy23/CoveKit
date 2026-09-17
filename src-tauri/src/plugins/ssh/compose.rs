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
    let directory = project
        .working_dir
        .as_deref()
        .unwrap_or(if directory.is_empty() { "/" } else { directory });
    if !directory.starts_with('/') || directory.chars().any(char::is_control) {
        return Err("项目工作目录必须为远程绝对路径".into());
    }
    let args = match action {
        "up" => "up -d",
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "down" => "down",
        "pull" => "pull",
        "build" => "build",
        "ps" => "ps --all --format json",
        "logs" => "logs --no-color --tail 200",
        "config" => "config --quiet",
        "update" => "up -d --pull always",
        "recreate" => "up -d --force-recreate",
        "rebuild" => "up -d --build --force-recreate",
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
                working_dir: None,
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
    progress: Option<&tauri::ipc::Channel<(bool, Vec<u8>)>>,
    input: Option<&str>,
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
        // 草稿只通过 SSH stdin 传递，不出现在远程进程命令行中。
        if let Some(input) = input {
            channel
                .data(input.as_bytes())
                .await
                .map_err(|e| format!("发送校验草稿失败：{e}"))?;
            channel
                .eof()
                .await
                .map_err(|e| format!("结束校验输入失败：{e}"))?;
        }
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    stdout.extend_from_slice(&data);
                    if let Some(progress) = progress {
                        let _ = progress.send((false, data.to_vec()));
                    }
                }
                ChannelMsg::ExtendedData { data, .. } => {
                    stderr.extend_from_slice(&data);
                    if let Some(progress) = progress {
                        let _ = progress.send((true, data.to_vec()));
                    }
                }
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
        None,
        None,
    )
    .await?;
    if out.exit_code != 0 {
        return Err(format!(
            "Compose 查询失败（{}）：{}{}",
            out.exit_code, out.stderr, out.stdout
        ));
    }
    let mut projects = parse_projects(&out.stdout)?;
    if !projects.is_empty() {
        // 只查询 Docker 元数据，不扫描文件系统。ID 仅来自 Docker，引用标签不参与 shell 执行。
        let labels = execute(&ssh_state, &connection_id,
            "ids=$(docker ps -aq --filter label=com.docker.compose.project) || exit; if [ -n \"$ids\" ]; then docker inspect --format '{{json .Config.Labels}}' $ids; fi", 30, None, None).await?;
        if labels.exit_code != 0 {
            return Err(format!("读取 Compose 工作目录失败：{}", labels.stderr));
        }
        apply_working_dirs(&mut projects, &labels.stdout)?;
    }
    Ok(projects)
}

/// 恢复创建容器时的工作目录，避免接管后相对挂载位置发生变化。
fn apply_working_dirs(projects: &mut [ComposeProject], raw: &str) -> Result<(), String> {
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let labels: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("Compose 标签解析失败：{e}"))?;
        let name = labels
            .get("com.docker.compose.project")
            .and_then(|v| v.as_str());
        let directory = labels
            .get("com.docker.compose.project.working_dir")
            .and_then(|v| v.as_str());
        if let (Some(name), Some(directory)) = (name, directory) {
            if let Some(project) = projects.iter_mut().find(|p| p.name == name) {
                if project
                    .working_dir
                    .as_deref()
                    .is_some_and(|old| old != directory)
                {
                    return Err(format!(
                        "项目 {name} 的容器记录了不同工作目录，请先核对远端配置"
                    ));
                }
                project.working_dir = Some(directory.to_owned());
            }
        }
    }
    Ok(())
}

/// SFTP 初始目录作为远程用户默认目录，不使用客户端 HOME。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_compose_home(
    ssh_state: State<'_, SshState>,
    connection_id: String,
) -> Result<String, String> {
    get_sftp_session(&ssh_state, &connection_id)
        .await?
        .canonicalize(".")
        .await
        .map_err(|e| format!("读取远程主目录失败：{e}"))
}

/// 项目级编排操作；拆除不删除卷，不隐式附加 --remove-orphans。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_compose_action(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    project: ComposeProject,
    action: String,
    progress: Option<tauri::ipc::Channel<(bool, Vec<u8>)>>,
    draft_path: Option<String>,
    draft_content: Option<String>,
) -> Result<ComposeOutput, String> {
    let mut command = compose_command(&project, &action)?;
    if let (Some(path), Some(content)) = (&draft_path, &draft_content) {
        if action != "config" || !project.config_files.contains(path) || content.len() > MAX_CONFIG
        {
            return Err("仅配置校验可传入当前文件草稿，且不得超过 1 MiB".into());
        }
        // stdin 替换有序 -f 中的当前文件，不写盘；项目目录保持显式传入。
        command = command.replacen(&format!("-f {}", shell_quote(path)), "-f -", 1);
    } else if draft_path.is_some() || draft_content.is_some() {
        return Err("校验草稿的路径与内容必须同时提供".into());
    }
    execute(
        &ssh_state,
        &connection_id,
        &command,
        600,
        progress.as_ref(),
        draft_content.as_deref(),
    )
    .await
}

/// 创建项目目录与新的远程 YAML；独占创建拒绝覆盖，不创建 YAML 中引用的挂载文件。
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
    let (parent, _) = remote_path.rsplit_once('/').ok_or("配置路径无效")?;
    let parent = if parent.is_empty() { "/" } else { parent };
    let created = execute(
        &ssh_state,
        &connection_id,
        &format!("mkdir -p -- {}", shell_quote(parent)),
        30,
        None,
        None,
    )
    .await?;
    if created.exit_code != 0 {
        return Err(format!("创建项目目录失败：{}", created.stderr));
    }
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
            working_dir: Some("/srv/original".into()),
        };
        let command = compose_command(&project, "up").unwrap();
        assert!(command.contains(r#"-f '/opt/a b/compose.yaml' -f '/opt/a b/it'"'"'s.yaml'"#));
        assert!(command.ends_with("up -d"));
        assert!(command.contains("--project-directory '/srv/original'"));
        assert!(compose_command(&project, "down; id").is_err());
        assert!(!compose_command(&project, "down")
            .unwrap()
            .contains("--volumes"));
        project.name = "$(id)".into();
        assert!(compose_command(&project, "up").is_err());
        assert!(validate_path("relative.yaml").is_err());
    }

    #[test]
    fn working_directory_labels_survive_adoption_and_conflicts_fail() {
        let mut projects = parse_projects(
            r#"[{"Name":"app","Status":"running(1)","ConfigFiles":"/etc/stacks/app.yml"}]"#,
        )
        .unwrap();
        apply_working_dirs(&mut projects, r#"{"com.docker.compose.project":"app","com.docker.compose.project.working_dir":"/srv/app data"}"#).unwrap();
        assert!(compose_command(&projects[0], "up")
            .unwrap()
            .contains("--project-directory '/srv/app data'"));
        assert!(apply_working_dirs(&mut projects, r#"{"com.docker.compose.project":"app","com.docker.compose.project.working_dir":"/other"}"#).is_err());
        assert!(apply_working_dirs(&mut projects, "invalid").is_err());
        // 旧持久化记录仍可反序列化，缺省才回到首文件目录。
        let old: ComposeProject = serde_json::from_str(
            r#"{"name":"old","status":"","configFiles":["/opt/old/docker-compose.yml"]}"#,
        )
        .unwrap();
        assert!(compose_command(&old, "up")
            .unwrap()
            .contains("--project-directory '/opt/old'"));
    }
}
