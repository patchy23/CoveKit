//! SSH 工具插件 · 数据结构（serde，与前端 plugins/ssh/contracts.ts 同步）
//! 本文件只声明类型，不包含任何逻辑；连接/终端/文件/监控等实现见各能力子模块。

use serde::{Deserialize, Serialize};

/* ── 通用 ── */

/// 操作结果（失败时 ok=false + error，不抛错给前端展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshActionResult {
    /// 操作是否成功
    pub(crate) ok: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 认证方式（密码 / 私钥 / 私钥+passphrase）
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    /// 用户名+密码
    Password,
    /// 用户名+私钥（无 passphrase）
    PrivateKey,
    /// 用户名+私钥+passphrase
    PrivateKeyWithPassphrase,
}

/// 服务器连接配置（凭证管理用，密码/密钥不直接存储，仅存 secretRef）
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    /// 唯一 id（profile-<毫秒时间戳>）
    pub(crate) id: String,
    /// 显示名称（如「生产服务器」）
    pub(crate) name: String,
    /// 主机地址
    pub(crate) host: String,
    /// SSH 端口（默认 22）
    pub(crate) port: u16,
    /// 登录用户名
    pub(crate) username: String,
    /// 认证方式
    pub(crate) auth_method: AuthMethod,
    /// 凭证引用（预留字段；密码/私钥由 credential.rs 加密落盘）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) secret_ref: Option<String>,
    /// 备注
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remark: Option<String>,
    /// 最后连接时间（毫秒时间戳）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_connected_at: Option<u64>,
}

/// 服务器连接状态
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 错误/断开
    Error,
    /// 重连中
    Reconnecting,
}

/// 服务器连接快照（侧栏列表展示用）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConnection {
    /// 关联的 ServerProfile id
    pub(crate) profile_id: String,
    /// 会话唯一 id（conn-<毫秒时间戳>）
    pub(crate) session_id: String,
    /// 当前状态
    pub(crate) status: ConnectionStatus,
    /// 已连接的服务器地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<String>,
    /// 延迟毫秒（ping 或 SSH 握手耗时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latency_ms: Option<u64>,
    /// 错误信息（status=error 时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// 建立连接的毫秒时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) connected_at: Option<u64>,
}

/* ── 终端 ── */

/// 终端会话快照
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    /// 终端唯一 id（term-<毫秒时间戳>）
    pub(crate) id: String,
    /// 关联的连接 sessionId
    pub(crate) connection_id: String,
    /// 终端标题（默认 profile 名称）
    pub(crate) title: String,
    /// 当前行列数
    pub(crate) cols: u16,
    /// 当前行数
    pub(crate) rows: u16,
    /// 是否活跃（前端正在展示）
    pub(crate) active: bool,
}

/// 终端数据块（Rust 推送 → 前端渲染）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TerminalData {
    /// 关联的终端 id
    pub(crate) terminal_id: String,
    /// 数据内容（原始字节，含 ANSI 转义序列）
    pub(crate) data: String,
    /// 毫秒时间戳
    pub(crate) time: u64,
}

/* ── 文件管理 ── */

/// 远程文件条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    /// 文件名
    pub(crate) name: String,
    /// 完整路径
    pub(crate) path: String,
    /// 是否目录
    pub(crate) is_dir: bool,
    /// 文件大小（字节，目录为 0）
    pub(crate) size: u64,
    /// 修改时间（毫秒时间戳）
    pub(crate) modified_at: u64,
    /// 权限字符串（如 drwxr-xr-x）
    pub(crate) permissions: String,
    /// 所有者
    pub(crate) owner: String,
    /// 所属组
    pub(crate) group: String,
}

/// 文件列表结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListResult {
    /// 查询是否成功
    pub(crate) ok: bool,
    /// 当前路径
    pub(crate) path: String,
    /// 父路径（根目录为 null）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_path: Option<String>,
    /// 文件列表
    pub(crate) files: Vec<RemoteFile>,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 文件传输进度
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferProgress {
    /// 传输 id
    pub(crate) transfer_id: String,
    /// 所属 SSH 连接会话 id
    pub(crate) connection_id: String,
    /// 本地路径
    pub(crate) local_path: String,
    /// 远程路径
    pub(crate) remote_path: String,
    /// 已传输字节
    pub(crate) transferred: u64,
    /// 总字节
    pub(crate) total: u64,
    /// 是否完成
    pub(crate) done: bool,
    /// 是否失败
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/* ── 远程编辑 ── */

/// 远程文件内容
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileContent {
    /// 读取是否成功
    pub(crate) ok: bool,
    /// 文件路径
    pub(crate) path: String,
    /// 文件内容（文本）
    pub(crate) content: String,
    /// 文件大小（字节）
    pub(crate) size: u64,
    /// 编码（如 UTF-8 / GBK）
    pub(crate) encoding: String,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/* ── 资源监控 ── */

/// 监控数据点
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorData {
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存已用（字节）
    pub(crate) memory_used: u64,
    /// 内存总量（字节）
    pub(crate) memory_total: u64,
    /// 磁盘使用率 0-100
    pub(crate) disk_percent: f32,
    /// 磁盘已用（字节）
    pub(crate) disk_used: u64,
    /// 磁盘总量（字节）
    pub(crate) disk_total: u64,
    /// 网络上行速率（字节/秒）
    pub(crate) net_upload_bps: u64,
    /// 网络下行速率（字节/秒）
    pub(crate) net_download_bps: u64,
    /// 采样时间（毫秒时间戳）
    pub(crate) timestamp: u64,
}

/* ── 服务管理 ── */

/// systemd 服务条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemdService {
    /// 服务名（如 nginx.service）
    pub(crate) name: String,
    /// 描述
    pub(crate) description: String,
    /// 加载状态（loaded / not-found / masked）
    pub(crate) load_state: String,
    /// 活动状态（active / inactive / failed）
    pub(crate) active_state: String,
    /// 子状态（running / dead / exited）
    pub(crate) sub_state: String,
    /// 是否开机自启
    pub(crate) enabled: bool,
}

/* ── 进程管理 ── */

/// 进程条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// 进程 ID
    pub(crate) pid: u32,
    /// 运行用户
    pub(crate) user: String,
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存占用（字节）
    pub(crate) memory_bytes: u64,
    /// 启动时间（毫秒时间戳）
    pub(crate) started_at: u64,
    /// 完整命令行
    pub(crate) command: String,
}

/* ── Docker ── */

/// Docker 容器条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    /// 容器 ID（短）
    pub(crate) id: String,
    /// 容器名
    pub(crate) name: String,
    /// 镜像名
    pub(crate) image: String,
    /// 状态（running / exited / paused）
    pub(crate) status: String,
    /// 本次运行持续时间（来自 docker ps Status）
    pub(crate) uptime: String,
    /// 端口映射（如 "80:80,443:443"）
    pub(crate) ports: String,
    /// 创建时间（毫秒时间戳）
    pub(crate) created_at: u64,
}

/// Docker 日志条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 契约类型：docker 日志列表，当前版本未构造（v1 返回原始文本）
pub struct DockerLog {
    /// 容器 ID
    pub(crate) container_id: String,
    /// 日志内容
    pub(crate) content: String,
    /// 时间戳
    pub(crate) time: u64,
}

/* ── 连接请求载荷（前端 ssh_connect 入参） ── */

/// SSH 连接请求载荷
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectPayload {
    /// 服务器配置
    pub(crate) profile: ServerProfile,
    /// 密码（auth_method=Password 时必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 私钥内容（auth_method=PrivateKey/PrivateKeyWithPassphrase 时必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase（auth_method=PrivateKeyWithPassphrase 时必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
}

/// SSH 凭证保存载荷（与契约 Payloads.ssh_credential_save 对应）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshCredentialSavePayload {
    /// 服务器配置（id 作为凭证 key）
    pub(crate) profile: ServerProfile,
    /// 密码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 私钥内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
}

/// Docker 交互终端命令入参；打包为 payload 以保持 IPC 契约稳定并规避参数过多。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshDockerExecPayload {
    /// SSH 连接会话 id
    pub(crate) connection_id: String,
    /// Docker 容器 id
    pub(crate) container_id: String,
    /// 容器内 shell，只允许 /bin/sh 或 /bin/bash
    pub(crate) shell: String,
    /// PTY 列数
    pub(crate) cols: u32,
    /// PTY 行数
    pub(crate) rows: u32,
}
