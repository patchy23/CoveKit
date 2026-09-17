//! SSH 远程管理插件 · 门面
//! 命令函数（薄层）与插件装配在此；实现按能力拆分：
//! - models.rs：serde 数据结构（与前端 plugins/ssh/contracts.ts 同步）
//! - conn.rs：连接会话注册表（russh 客户端 + 分阶段事件 + 主机密钥人工确认）
//! - host_keys.rs：已知主机解析/删除/替换（私有 known_hosts 文件）
//! - store.rs：服务器配置与分组持久化（ssh.db，PluginDb 骨架）
//! - terminal.rs：PTY 终端通道（事件推送）
//! - log.rs：终端会话日志（ANSI 剥离后旁路落盘）
//! - file.rs / edit.rs / monitor.rs / system_info.rs / service.rs / process.rs / docker.rs：其余能力

mod close_hooks; // 关闭清理钩子（登记到 framework/lifecycle，退出时由框架协调调用）
pub(crate) mod conn; // conn/ 目录：会话注册表 + 连接/重连（能力域下沉，引用路径经 mod.rs pub use 保持不变）
mod credential_refs;
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
#[allow(dead_code)] // 导出适配在 C4 命令层接入前只有测试调用方（接入后删掉本行）
mod transfer; // 导出/导入适配（数据集声明、按 id 导出、记录体检）
pub(crate) mod tunnel;

use crate::plugins::ssh::conn::{HostKeyState, SshState};
use crate::plugins::ssh::store::ProfileState;
use crate::plugins::ssh::terminal::TerminalState;
use crate::plugins::ssh::tunnel::TunnelState;

// 模块静态清单：命令名、入库元数据与分派 handler 同源生成（AR07 §10.2）
crate::patchybox_module! {
    owner: "ssh",
    feature: "ssh",
    commands: {
        conn::reconnect::ssh_disconnect => "断开连接并清理会话",
        conn::reconnect::ssh_reconnect => "重新连接（新会话替换旧会话）",
        conn::ssh_connections => "全部会话快照（侧栏轮询）",
        conn::ssh_host_key_respond => "应答主机密钥确认（首连/指纹变更）",
        conn::ssh_known_host_list => "已知主机列表（含 SHA256 指纹）",
        conn::ssh_known_host_delete => "删除已知主机条目",
        store::profiles::ssh_profile_list => "服务器配置列表",
        store::bookmarks::ssh_bookmark_list => "目录书签列表",
        store::bookmarks::ssh_bookmark_add => "新增目录书签",
        store::bookmarks::ssh_bookmark_delete => "删除目录书签",
        store::profiles::ssh_profile_save => "新增/更新服务器配置（凭证入 Vault）",
        store::profiles::ssh_profile_delete => "删除服务器配置",
        store::profiles::ssh_group_list => "服务器分组列表",
        store::profiles::ssh_group_save => "新增/更新分组",
        store::profiles::ssh_group_delete => "删除分组（组内配置移回未分组）",
        store::tunnels::ssh_tunnel_list => "某服务器的隧道配置列表",
        store::tunnels::ssh_tunnel_save => "新增/更新隧道配置",
        tunnel::lifecycle::ssh_tunnel_start => "启动隧道（绑定到指定连接）",
        tunnel::lifecycle::ssh_tunnel_stop => "停止隧道",
        tunnel::lifecycle::ssh_tunnels => "某连接下的隧道运行时快照",
        tunnel::lifecycle::ssh_tunnel_delete => "删除隧道配置",
        terminal::ssh_terminal_open => "打开 PTY 终端通道（xterm）",
        terminal::ssh_terminal_write => "写入终端数据（键盘输入）",
        terminal::ssh_terminal_resize => "调整终端窗口大小",
        terminal::ssh_terminal_close => "关闭终端通道",
        terminal::ssh_terminal_list => "某连接下的全部终端会话",
        log::ssh_terminal_log_stop => "停止录制并返回日志路径与字节数",
        log::ssh_terminal_log_open_dir => "打开 SSH 终端日志目录",
        sftp::browse::ssh_file_list => "远程目录列表（SFTP）",
        sftp::transfer::ssh_file_upload => "上传文件（进度事件推送）",
        sftp::transfer::ssh_file_download => "下载文件（进度事件推送）",
        sftp::ops::ssh_file_delete => "删除远程文件/目录",
        sftp::ops::ssh_file_rename => "重命名远程文件/目录",
        sftp::ops::ssh_file_mkdir => "新建远程目录",
        sftp::ops::ssh_file_create => "新建远程空文件",
        sftp::ops::ssh_file_chmod => "修改远程权限（含安全策略）",
        sftp::browse::ssh_local_list => "本地目录列表（双栏文件管理）",
        sftp::browse::ssh_local_create => "本地新建文件/目录",
        sftp::browse::ssh_local_delete => "本地删除文件/目录",
        sftp::browse::ssh_local_rename => "本地重命名/移动",
        sftp::transfer::ssh_file_download_recursive => "递归下载远程目录",
        sftp::ops::ssh_transfer_cancel => "取消传输任务",
        edit::ssh_edit_open => "打开远程文件（下载内容）",
        edit::ssh_edit_save => "保存远程文件（上传回写）",
        monitor::ssh_monitor_get => "资源监控数据（CPU/内存/磁盘/网络）",
        system_info::ssh_system_info_get => "远程系统信息与磁盘分区明细",
        service::ssh_service_list => "systemd 服务列表",
        service::ssh_service_action => "服务启动/停止/重启",
        service::ssh_service_logs => "服务日志（journalctl）",
        service::ssh_service_config => "查看 systemd 服务配置",
        process::ssh_process_list => "进程列表",
        process::ssh_process_kill => "结束进程",
        process::ssh_process_detail => "进程详情（ps -fp）",
        docker::ssh_docker_list => "Docker 容器列表",
        docker::ssh_docker_action => "容器启动/停止/重启/删除",
        docker::ssh_docker_logs => "容器日志",
        docker::ssh_docker_exec => "进入容器终端（PTY）",
        conn::connect::ssh_connect => "建立 SSH 连接（按 profileId 取配置，凭证在 Rust 侧解析）",
        log::ssh_terminal_log_start => "开始录制终端日志（幂等，已在录制则返回当前文件）",
    },
}

/// 插件注册：命令 + 会话/终端/主机密钥/配置库 State + IPC 命令入库
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    // 凭证引用自报：框架删除凭证前据此判断还有哪些服务器在用
    credential_refs::register_provider();
    // 导出适配登记：框架做导入导出时需要知道本插件有哪些数据集、按什么粒度勾选（sync L2）
    transfer::register();
    // 关闭清理登记（AR06）：会话/终端/隧道/传输由本模块自己清，框架只协调、超时与汇总
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::exit_only(IPC_OWNER)
            .with_dispose(close_hooks::on_dispose),
    );
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
