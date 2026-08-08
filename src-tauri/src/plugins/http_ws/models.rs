//! HTTP/WS 调试插件 · 数据结构（serde，与前端 plugins/http-ws/contracts.ts 同步）

use serde::Serialize;

/* ── HTTP ── */

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestPayload {
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Option<String>,
    pub(crate) timeout_ms: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    pub(crate) ok: bool,
    pub(crate) status: u16,
    pub(crate) status_text: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: String,
    pub(crate) body_size: usize,
    pub(crate) duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/* ── WebSocket ── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    pub(crate) direction: &'static str,
    pub(crate) content: String,
    pub(crate) time: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsSession {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) connected_at: u64,
    pub(crate) open: bool,
    pub(crate) messages: Vec<WsMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsActionResult {
    pub(crate) ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}
