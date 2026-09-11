//! SSH 远程管理插件 · 门面
//! 命令函数（薄层）与插件装配在此；实现按能力拆分：
//! - models.rs：serde 数据结构（与前端 plugins/ssh/contracts.ts 同步）
//! - conn.rs：连接会话注册表（russh 客户端 + 分阶段事件 + 主机密钥人工确认）
//! - host_keys.rs：已知主机解析/删除/替换（私有 known_hosts 文件）
//! - store.rs：服务器配置与分组持久化（ssh.db，PluginDb 骨架）
//! - credential.rs：旧版手工凭证读取与归档（已迁移 Vault，仅存档兼容）
//! - terminal.rs：PTY 终端通道（事件推送）
//! - log.rs：终端会话日志（ANSI 剥离后旁路落盘）
//! - file.rs / edit.rs / monitor.rs / system_info.rs / service.rs / process.rs / docker.rs：其余能力

pub(crate) mod conn; // conn/ 目录：会话注册表 + 连接/重连（能力域下沉，引用路径经 mod.rs pub use 保持不变）
pub(crate) mod credential;
pub(crate) mod docker;
pub(crate) mod edit;
pub(crate) mod host_keys;
pub(crate) mod log;
mod models; // models/ 目录：serde 数据结构按域拆分
pub(crate) mod monitor;
pub(crate) mod process;
pub(crate) mod service;
pub(crate) mod sftp; // sftp/ 目录：文件浏览/传输/递归下载
pub(crate) mod store;
pub(crate) mod system_info;
pub(crate) mod terminal;
pub(crate) mod tunnel;

use crate::plugins::ssh::conn::{HostKeyState, SshState};
use crate::plugins::ssh::store::ProfileState;
use crate::plugins::ssh::terminal::TerminalState;
use crate::plugins::ssh::tunnel::TunnelState;

/// 分派 SSH 插件全部命令；应用级 Builder 仅安装一个总 handler。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        conn::connect::ssh_connect,
        conn::reconnect::ssh_disconnect,
        conn::reconnect::ssh_reconnect,
        conn::ssh_connections,
        conn::ssh_host_key_respond,
        conn::ssh_known_host_list,
        conn::ssh_known_host_delete,
        store::bookmarks::ssh_bookmark_add,
        store::bookmarks::ssh_bookmark_delete,
        store::bookmarks::ssh_bookmark_list,
        store::profiles::ssh_profile_list,
        store::profiles::ssh_profile_save,
        store::profiles::ssh_profile_delete,
        store::profiles::ssh_profile_import,
        store::profiles::ssh_group_list,
        store::profiles::ssh_group_save,
        store::profiles::ssh_group_delete,
        store::tunnels::ssh_tunnel_list,
        store::tunnels::ssh_tunnel_save,
        tunnel::lifecycle::ssh_tunnel_start,
        tunnel::lifecycle::ssh_tunnel_stop,
        tunnel::lifecycle::ssh_tunnels,
        tunnel::lifecycle::ssh_tunnel_delete,
        terminal::ssh_terminal_open,
        terminal::ssh_terminal_write,
        terminal::ssh_terminal_resize,
        terminal::ssh_terminal_close,
        terminal::ssh_terminal_list,
        log::ssh_terminal_log_start,
        log::ssh_terminal_log_stop,
        sftp::browse::ssh_file_list,
        sftp::browse::ssh_local_create,
        sftp::browse::ssh_local_delete,
        sftp::browse::ssh_local_list,
        sftp::browse::ssh_local_rename,
        sftp::transfer::ssh_file_upload,
        sftp::transfer::ssh_file_download,
        sftp::ops::ssh_file_delete,
        sftp::ops::ssh_file_rename,
        sftp::ops::ssh_file_chmod,
        sftp::ops::ssh_file_create,
        sftp::ops::ssh_file_mkdir,
        sftp::transfer::ssh_file_download_recursive,
        sftp::ops::ssh_transfer_cancel,
        edit::ssh_edit_open,
        edit::ssh_edit_save,
        monitor::ssh_monitor_get,
        system_info::ssh_system_info_get,
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

/// 插件注册：命令 + 会话/终端/主机密钥/配置库 State + IPC 命令入库
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(
        "ssh",
        &[
            (
                "ssh_connect",
                "建立 SSH 连接（按 profileId 解析配置与凭证）",
            ),
            ("ssh_disconnect", "断开连接并清理会话"),
            ("ssh_reconnect", "重新连接（新会话替换旧会话）"),
            ("ssh_connections", "全部会话快照（侧栏轮询）"),
            ("ssh_host_key_respond", "应答主机密钥确认（首连/指纹变更）"),
            ("ssh_known_host_list", "已知主机列表（含 SHA256 指纹）"),
            ("ssh_known_host_delete", "删除已知主机条目"),
            ("ssh_profile_list", "服务器配置列表"),
            ("ssh_bookmark_list", "目录书签列表"),
            ("ssh_bookmark_add", "新增目录书签"),
            ("ssh_bookmark_delete", "删除目录书签"),
            ("ssh_profile_save", "新增/更新服务器配置（凭证入 Vault）"),
            ("ssh_profile_delete", "删除服务器配置"),
            (
                "ssh_profile_import",
                "localStorage 存量配置与凭证一次性导入",
            ),
            ("ssh_group_list", "服务器分组列表"),
            ("ssh_group_save", "新增/更新分组"),
            ("ssh_group_delete", "删除分组（组内配置移回未分组）"),
            ("ssh_tunnel_list", "某服务器的隧道配置列表"),
            ("ssh_tunnel_save", "新增/更新隧道配置"),
            ("ssh_tunnel_start", "启动隧道（绑定到指定连接）"),
            ("ssh_tunnel_stop", "停止隧道"),
            ("ssh_tunnels", "某连接下的隧道运行时快照"),
            ("ssh_tunnel_delete", "删除隧道配置"),
            ("ssh_terminal_open", "打开 PTY 终端通道（xterm）"),
            ("ssh_terminal_write", "写入终端数据（键盘输入）"),
            ("ssh_terminal_resize", "调整终端窗口大小"),
            ("ssh_terminal_close", "关闭终端通道"),
            ("ssh_terminal_list", "某连接下的全部终端会话"),
            (
                "ssh_terminal_log_start",
                "开始录制终端会话日志（ANSI 剥离后落盘）",
            ),
            ("ssh_terminal_log_stop", "停止录制并返回日志路径与字节数"),
            ("ssh_file_list", "远程目录列表（SFTP）"),
            ("ssh_file_upload", "上传文件（进度事件推送）"),
            ("ssh_file_download", "下载文件（进度事件推送）"),
            ("ssh_file_delete", "删除远程文件/目录"),
            ("ssh_file_rename", "重命名远程文件/目录"),
            ("ssh_file_mkdir", "新建远程目录"),
            ("ssh_file_create", "新建远程空文件"),
            ("ssh_file_chmod", "修改远程权限（含安全策略）"),
            ("ssh_local_list", "本地目录列表（双栏文件管理）"),
            ("ssh_local_create", "本地新建文件/目录"),
            ("ssh_local_delete", "本地删除文件/目录"),
            ("ssh_local_rename", "本地重命名/移动"),
            ("ssh_file_download_recursive", "递归下载远程目录"),
            ("ssh_transfer_cancel", "取消传输任务"),
            ("ssh_edit_open", "打开远程文件（下载内容）"),
            ("ssh_edit_save", "保存远程文件（上传回写）"),
            ("ssh_monitor_get", "资源监控数据（CPU/内存/磁盘/网络）"),
            ("ssh_system_info_get", "远程系统信息与磁盘分区明细"),
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
        .manage(HostKeyState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
        .manage(TunnelState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
        .manage(sftp::TransferState(std::sync::Arc::new(
            std::sync::Mutex::new(std::collections::HashMap::new()),
        )))
        .manage(ProfileState(std::sync::Mutex::new(None)))
        .manage(monitor::MonitorState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
}
