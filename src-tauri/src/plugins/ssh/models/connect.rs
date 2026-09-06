//! SSH 数据模型 · 连接阶段事件与结构化结果

use super::common::ServerConnection;
use serde::{Deserialize, Serialize};

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
