//! HTTP/WS 调试插件 · 数据结构（serde，与前端 plugins/http-ws/contracts.ts 同步）
//! 本文件只声明类型，不包含任何逻辑；HTTP 实现见 http.rs，WS 会话见 ws.rs。

use serde::Serialize;

/* ── HTTP ── */

/// HTTP 请求载荷（前端 HttpPanel 发送时的完整入参）
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestPayload {
    /// HTTP 方法（GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS）
    pub(crate) method: String,
    /// 请求地址（含 Params 拼接后的完整 URL）
    pub(crate) url: String,
    /// 请求头键值对（空 key 的行会被忽略）
    pub(crate) headers: Vec<(String, String)>,
    /// 用户选择无请求体时为 None，不按方法丢弃已填写的内容。
    pub(crate) body: Option<String>,
    /// 超时毫秒数（默认 15000）
    pub(crate) timeout_ms: Option<u64>,
    /// 临时认证或凭证库引用，秘密不写入接口库。
    pub(crate) auth: Option<RequestAuth>,
}

/// HTTP、SSE 与 WebSocket 共用认证载荷。
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuth {
    /// none / basic / bearer。
    pub(crate) mode: String,
    /// 为空时使用当前页签提供的临时值。
    #[serde(default)]
    pub(crate) credential_id: String,
    /// 临时用户名。
    pub(crate) username: Option<String>,
    /// 临时密码或 token。
    pub(crate) secret: Option<String>,
}

/// SSE 完整事件；多行 data 用换行连接，id 跨事件保留。
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SseEvent {
    /// 事件名称，缺省 message。
    pub(crate) event: String,
    /// 最近的事件 ID。
    pub(crate) id: String,
    /// 未经 JSON 解析的原文。
    pub(crate) data: String,
    /// 服务端建议的重试间隔，仅展示、不自动重连。
    pub(crate) retry: Option<u64>,
}

/// 长连接向所属前端页签发送的状态与事件。
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SseUpdate {
    /// 握手与媒体类型检查成功。
    Connected {
        /// HTTP 握手状态码。
        status: u16,
        /// 握手响应头。
        headers: Vec<(String, String)>,
    },
    /// 完整事件。
    Event {
        /// 已解析的事件原文与元数据。
        event: SseEvent,
    },
    /// 服务端正常结束。
    Closed,
    /// 握手、读取或解析失败。
    Error {
        /// 面向用户的错误信息，不包含认证秘密。
        message: String,
    },
}

/// HTTP 完整响应；传输与读取失败通过命令错误返回。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    /// 请求是否成功（status < 400）
    pub(crate) ok: bool,
    /// HTTP 状态码（网络失败时为 0）
    pub(crate) status: u16,
    /// 状态文本（如 OK / Not Found）
    pub(crate) status_text: String,
    /// 响应头键值对
    pub(crate) headers: Vec<(String, String)>,
    /// 响应体文本（网络失败时为空）
    pub(crate) body: String,
    /// 响应体字节数
    pub(crate) body_size: usize,
    /// 请求总耗时毫秒
    pub(crate) duration_ms: u64,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/* ── WebSocket ── */

/// 单条 WS 消息，前端日志按序号保持身份。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    /// 会话内单调递增序号，清空视图后不重新出现旧消息。
    pub(crate) seq: u64,
    /// 消息方向：sent（本端发送）/ received（服务端推送）
    pub(crate) direction: &'static str,
    /// 消息文本内容
    pub(crate) content: String,
    /// 收到/发送的毫秒时间戳
    pub(crate) time: u64,
}

/// WS 会话快照，前端按可见性调整轮询频率。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsSession {
    /// 会话唯一 UUID。
    pub(crate) id: String,
    /// 连接地址
    pub(crate) url: String,
    /// 建立连接的毫秒时间戳
    pub(crate) connected_at: u64,
    /// 是否仍处于连接状态
    pub(crate) open: bool,
    /// 消息队列快照（上限 500 条，最旧的被丢弃）
    pub(crate) messages: Vec<WsMessage>,
    /// 超过队列容量后丢弃的历史消息数。
    pub(crate) dropped: u64,
    /// 后台读写失败原因。
    pub(crate) error: Option<String>,
}

/// WS 会话操作结果（send/close 的通用返回）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsActionResult {
    /// 操作是否成功
    pub(crate) ok: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}
