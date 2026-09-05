//! SSH 远程管理插件 · 门面
//! 命令函数（薄层）与插件装配在此；实现按能力拆分：
//! - models.rs：serde 数据结构（与前端 plugins/ssh/contracts.ts 同步）
//! - conn.rs：连接会话注册表（russh 客户端 + 状态事件）
//! - terminal.rs：PTY 终端通道（事件推送）
//! - file.rs / credential.rs / edit.rs / monitor.rs / service.rs / process.rs / docker.rs：其余能力

pub(crate) mod conn;
pub(crate) mod credential;
pub(crate) mod docker;
pub(crate) mod edit;
pub(crate) mod file;
mod models;
pub(crate) mod monitor;
pub(crate) mod process;
pub(crate) mod service;
pub(crate) mod terminal;

use crate::plugins::ssh::conn::SshState;
use crate::plugins::ssh::terminal::TerminalState;

/// 分派 SSH 插件全部命令；应用级 Builder 仅安装一个总 handler。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        conn::ssh_connect,
        conn::ssh_disconnect,
        conn::ssh_reconnect,
        conn::ssh_connections,
        terminal::ssh_terminal_open,
        terminal::ssh_terminal_write,
        terminal::ssh_terminal_resize,
        terminal::ssh_terminal_close,
        terminal::ssh_terminal_list,
        file::ssh_file_list,
        file::ssh_file_upload,
        file::ssh_file_download,
        file::ssh_file_delete,
        file::ssh_file_rename,
        credential::ssh_credential_save,
        credential::ssh_credential_get,
        credential::ssh_credential_delete,
        edit::ssh_edit_open,
        edit::ssh_edit_save,
        monitor::ssh_monitor_get,
        service::ssh_service_list,
        service::ssh_service_action,
        service::ssh_service_logs,
        process::ssh_process_list,
        process::ssh_process_kill,
        docker::ssh_docker_list,
        docker::ssh_docker_action,
        docker::ssh_docker_logs,
        docker::ssh_docker_exec,
    ];
    handler(invoke)
}

/// 插件注册：命令 + 会话/终端 State + IPC 命令入库
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(
        "ssh",
        &[
            ("ssh_connect", "建立 SSH 连接（密码/私钥认证）"),
            ("ssh_disconnect", "断开连接并清理会话"),
            ("ssh_reconnect", "重新连接（新会话替换旧会话）"),
            ("ssh_connections", "全部会话快照（侧栏轮询）"),
            ("ssh_terminal_open", "打开 PTY 终端通道（xterm）"),
            ("ssh_terminal_write", "写入终端数据（键盘输入）"),
            ("ssh_terminal_resize", "调整终端窗口大小"),
            ("ssh_terminal_close", "关闭终端通道"),
            ("ssh_terminal_list", "某连接下的全部终端会话"),
            ("ssh_file_list", "远程目录列表（SFTP）"),
            ("ssh_file_upload", "上传文件（进度事件推送）"),
            ("ssh_file_download", "下载文件（进度事件推送）"),
            ("ssh_file_delete", "删除远程文件/目录"),
            ("ssh_file_rename", "重命名远程文件/目录"),
            ("ssh_credential_save", "保存凭证（AES-GCM 加密）"),
            ("ssh_credential_get", "读取凭证（解密返回）"),
            ("ssh_credential_delete", "删除凭证"),
            ("ssh_edit_open", "打开远程文件（下载内容）"),
            ("ssh_edit_save", "保存远程文件（上传回写）"),
            ("ssh_monitor_get", "资源监控数据（CPU/内存/磁盘/网络）"),
            ("ssh_service_list", "systemd 服务列表"),
            ("ssh_service_action", "服务启动/停止/重启"),
            ("ssh_service_logs", "服务日志（journalctl）"),
            ("ssh_process_list", "进程列表"),
            ("ssh_process_kill", "结束进程"),
            ("ssh_docker_list", "Docker 容器列表"),
            ("ssh_docker_action", "容器启动/停止/重启/删除"),
            ("ssh_docker_logs", "容器日志"),
            ("ssh_docker_exec", "进入容器终端（PTY）"),
        ],
    )
    .expect("IPC 命令重复注册");
    builder
        .manage(SshState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
        .manage(TerminalState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
        .manage(monitor::MonitorState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
}
