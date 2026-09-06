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
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    /// 用户名+密码
    Password,
    /// 用户名+私钥（无 passphrase）
    PrivateKey,
    /// 用户名+私钥+passphrase
    PrivateKeyWithPassphrase,
}

/// 服务器连接配置（凭证只存公共 Vault 的 credentialRef 引用，秘密永不入库/不落 profile）
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
    /// 公共 Vault 凭证引用；为空表示尚未保存凭证（连接时需要一次性凭证）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) credential_ref: Option<String>,
    /// 所属分组 id（为空 = 未分组；分组实体在 ssh_groups 表）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_id: Option<String>,
    /// 备注
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remark: Option<String>,
    /// 最后连接时间（毫秒时间戳）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_connected_at: Option<u64>,
}

/// 服务器分组
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshGroup {
    /// 唯一 id（group-<毫秒时间戳>）
    pub(crate) id: String,
    /// 分组名称
    pub(crate) name: String,
    /// 排序权重（创建顺序自增）
    pub(crate) sort_order: i64,
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
    /// 所属连接 id（前端据此把工作区标记为活跃，防止看日志时被空闲断开误杀）
    pub(crate) connection_id: String,
    /// 数据内容（原始字节，含 ANSI 转义序列）
    pub(crate) data: String,
    /// 毫秒时间戳
    pub(crate) time: u64,
}

/// 终端 PTY 通道关闭通知。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TerminalClosed {
    /// 已关闭的终端 id。
    pub(crate) terminal_id: String,
    /// 所属连接会话 id（前端据此触发断线自动重连）
    pub(crate) connection_id: String,
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
    /// 修改时间（毫秒；编辑器乐观锁基线）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) modified_at: Option<u64>,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 远程编辑保存结果（冲突时带当前 mtime 供前端决策）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditSaveResult {
    /// 保存是否成功
    pub(crate) ok: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// 远端文件已被他人修改（乐观锁冲突），未写入
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) conflict: Option<bool>,
    /// 冲突时远端当前 mtime（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_mtime: Option<u64>,
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

/// 一次性凭证覆盖（仅本次连接在内存中使用，不落任何存储）
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CredentialOverride {
    /// 密码（auth_method=Password 时使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 私钥内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
}

/// SSH 连接请求载荷：只传 profile id（后端自行读取配置并解析 Vault 凭证）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectPayload {
    /// 服务器配置 id
    pub(crate) profile_id: String,
    /// 一次性凭证覆盖（优先于已保存凭证；仅在内存中使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overrides: Option<CredentialOverride>,
}

/// 服务器保存载荷：配置 + 可选手工凭证（勾选保存时写入 Vault，返回带 credentialRef 的记录）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfileSavePayload {
    /// 服务器配置
    pub(crate) profile: ServerProfile,
    /// 保存到 Vault 的密码（手工密码认证时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) password: Option<String>,
    /// 保存到 Vault 的私钥内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key: Option<String>,
    /// 私钥 passphrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) passphrase: Option<String>,
    /// 是否把本次输入的凭证保存到 Vault（false = 仅更新配置，凭证保持原引用）
    #[serde(default)]
    pub(crate) save_credential: bool,
}

/// localStorage → 插件库一次性导入载荷（含旧手工凭证的明文迁移原始输入）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfileImportPayload {
    /// 服务器配置列表（localStorage 中的存量数据）
    pub(crate) profiles: Vec<ServerProfile>,
    /// 分组列表
    pub(crate) groups: Vec<SshGroup>,
}

/// 导入结果摘要（不含任何秘密）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshImportResult {
    /// 导入的服务器配置数
    pub(crate) imported_profiles: usize,
    /// 导入的分组数
    pub(crate) imported_groups: usize,
    /// 迁移进 Vault 的凭证数
    pub(crate) migrated_credentials: usize,
    /// 旧凭证文件存在但解密失败（配置已迁入，凭证需用户重新保存）
    pub(crate) legacy_credentials_failed: bool,
}

/* ── 主机密钥校验（首连确认 / 变更阻断） ── */

/// 已知主机条目（来自 patchyBox 私有 known_hosts 文件）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostEntry {
    /// 主机地址
    pub(crate) host: String,
    /// 端口
    pub(crate) port: u16,
    /// 公钥算法名（如 ssh-ed25519）
    pub(crate) algorithm: String,
    /// SHA256 指纹（SHA256:base64）
    pub(crate) fingerprint: String,
}

/// 主机密钥人工确认请求（后端在握手回调中推送，等待前端 ssh_host_key_respond）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyVerifyRequest {
    /// 本次连接尝试的唯一请求 id
    pub(crate) request_id: String,
    /// 类型：unknown = 首次连接；mismatch = 与已保存指纹不一致
    pub(crate) kind: String,
    /// 主机地址
    pub(crate) host: String,
    /// 端口
    pub(crate) port: u16,
    /// 公钥算法名
    pub(crate) algorithm: String,
    /// 服务器公钥 SHA256 指纹
    pub(crate) fingerprint: String,
    /// kind=mismatch 时已保存的指纹列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) saved_fingerprints: Vec<String>,
}

/// 用户对主机密钥的决定（respond 命令入参）
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HostKeyDecision {
    /// 仅本次信任（不写入 known_hosts）
    TrustOnce,
    /// 保存并连接（写入 known_hosts）
    TrustSave,
    /// 取消连接
    Cancel,
    /// 仅 kind=mismatch：确认替换已保存指纹（前端已二次确认）
    Replace,
}

/* ── 连接阶段事件与结构化结果 ── */

/// 连接阶段（按 SSH 建链顺序）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectStage {
    /// 本次连接尝试的唯一请求 id（对应 ssh_connect 返回值中的 requestId）
    pub(crate) request_id: String,
    /// 所属服务器配置 id（前端据此把进度关联到工作区）
    pub(crate) profile_id: String,
    /// 阶段：resolve / tcp / handshake / verify / auth / session
    pub(crate) stage: String,
    /// 阶段状态：start / ok / fail
    pub(crate) status: String,
    /// 附加信息（失败原因等）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}

/// 稳定错误码（前端据此分支展示，message 为中文用户文案）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectError {
    /// 错误码（如 AUTH_FAILED / TCP_TIMEOUT / HOST_KEY_MISMATCH）
    pub(crate) code: String,
    /// 中文用户文案
    pub(crate) message: String,
    /// 可复制的技術详情（已脱敏）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
}

/// ssh_connect / ssh_reconnect 的结构化返回：业务失败不抛 IPC 异常
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectOutcome {
    /// 连接是否成功
    pub(crate) ok: bool,
    /// 成功时的连接快照
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) connection: Option<ServerConnection>,
    /// 本次连接尝试的请求 id（关联 connect-stage 事件；成功时也有）
    pub(crate) request_id: String,
    /// 失败时的结构化错误
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<SshConnectError>,
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

/* ── SSH 隧道 ── */

/// 隧道类型
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TunnelType {
    /// 本地转发（-L）：本地监听 → 经 SSH → 目标主机
    Local,
    /// 远程转发（-R）：服务端监听 → 经 SSH → 本机侧目标
    Remote,
    /// 动态 SOCKS5（-D）：本地 SOCKS5 代理，目标由客户端请求指定
    Dynamic,
}

impl TunnelType {
    /// 存储字符串
    pub fn as_str(self) -> &'static str {
        match self {
            TunnelType::Local => "local",
            TunnelType::Remote => "remote",
            TunnelType::Dynamic => "dynamic",
        }
    }

    /// 存储字符串 → 类型（未知值兜底为本地转发）
    pub fn from_str(value: &str) -> Self {
        match value {
            "remote" => TunnelType::Remote,
            "dynamic" => TunnelType::Dynamic,
            _ => TunnelType::Local,
        }
    }
}

/// 隧道配置（随 profile 存插件库）
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConfig {
    /// 唯一 id（tun-<毫秒时间戳>）
    pub(crate) id: String,
    /// 所属服务器配置 id
    pub(crate) profile_id: String,
    /// 显示名称
    pub(crate) name: String,
    /// 隧道类型
    pub(crate) tunnel_type: TunnelType,
    /// 监听地址（local/dynamic 为本机侧；remote 为服务端侧；默认 127.0.0.1）
    pub(crate) listen_host: String,
    /// 监听端口
    pub(crate) listen_port: u16,
    /// 目标主机（dynamic 类型为空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_host: Option<String>,
    /// 目标端口（dynamic 类型为空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_port: Option<u16>,
    /// 连接建立后自动启动
    pub(crate) auto_start: bool,
}

/// 隧道运行状态
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TunnelStatus {
    /// 已停止
    Stopped,
    /// 启动中（监听/转发请求进行中）
    Starting,
    /// 运行中
    Running,
    /// 异常（监听失败/转发断开等）
    Error,
}

/// 隧道运行时快照（前端展示）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRuntime {
    /// 隧道配置
    #[serde(flatten)]
    pub(crate) config: TunnelConfig,
    /// 当前状态
    pub(crate) status: TunnelStatus,
    /// 活动连接数
    pub(crate) connections: u64,
    /// 异常信息（status=error 时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
