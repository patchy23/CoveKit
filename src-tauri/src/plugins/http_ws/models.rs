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
    /// 请求体（无请求体方法为 None）
    pub(crate) body: Option<String>,
    /// 超时毫秒数（默认 15000）
    pub(crate) timeout_ms: Option<u64>,
}

/// HTTP 响应结果（失败时 ok=false + error，不抛错——便于前端直接展示）
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

/// 单条 WS 消息（方向 + 内容 + 时间，前端气泡列表逐条渲染）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    /// 消息方向：sent（本端发送）/ received（服务端推送）
    pub(crate) direction: &'static str,
    /// 消息文本内容
    pub(crate) content: String,
    /// 收到/发送的毫秒时间戳
    pub(crate) time: u64,
}

/// WS 会话快照（前端 300ms 轮询拉取的完整状态）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsSession {
    /// 会话唯一 id（ws-<毫秒时间戳>）
    pub(crate) id: String,
    /// 连接地址
    pub(crate) url: String,
    /// 建立连接的毫秒时间戳
    pub(crate) connected_at: u64,
    /// 是否仍处于连接状态
    pub(crate) open: bool,
    /// 消息队列快照（上限 500 条，最旧的被丢弃）
    pub(crate) messages: Vec<WsMessage>,
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
