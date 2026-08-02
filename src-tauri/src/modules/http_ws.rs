//! HTTP/WS 调试模块（第二批）
//! - HTTP：reqwest 异步请求（方法/头/体/超时，返回状态/头/体/耗时）
//! - WebSocket：tokio-tungstenite 长连接会话（id → 后台读写任务 + 消息队列）
//!
//! 契约见前端 src/core/ipc/contracts.ts（唯一事实源）。

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tokio::sync::mpsc;

/* ── HTTP ── */

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestPayload {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    ok: bool,
    status: u16,
    status_text: String,
    headers: Vec<(String, String)>,
    body: String,
    body_size: usize,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn parse_method(m: &str) -> reqwest::Method {
    match m.to_uppercase().as_str() {
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "PATCH" => reqwest::Method::PATCH,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    }
}

/// 发送 HTTP 请求（失败返回 ok=false + error，不抛错——便于前端展示）
#[tauri::command]
pub async fn http_request(payload: HttpRequestPayload) -> Result<HttpResponseResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(payload.timeout_ms.unwrap_or(15_000)))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.request(parse_method(&payload.method), &payload.url);
    for (k, v) in &payload.headers {
        if !k.trim().is_empty() {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    if let Some(body) = &payload.body {
        if !body.is_empty() {
            req = req.body(body.clone());
        }
    }

    let start = Instant::now();
    let resp = req.send().await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let status_text = r.status().canonical_reason().unwrap_or("").to_string();
            let headers = r
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<Vec<_>>();
            let body = r.text().await.unwrap_or_default();
            let body_size = body.len();
            Ok(HttpResponseResult {
                ok: status < 400,
                status,
                status_text,
                headers,
                body,
                body_size,
                duration_ms,
                error: None,
            })
        }
        Err(e) => Ok(HttpResponseResult {
            ok: false,
            status: 0,
            status_text: String::new(),
            headers: Vec::new(),
            body: String::new(),
            body_size: 0,
            duration_ms,
            error: Some(e.to_string()),
        }),
    }
}

/* ── WebSocket ── */

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    direction: &'static str,
    content: String,
    time: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsSession {
    id: String,
    url: String,
    connected_at: u64,
    open: bool,
    messages: Vec<WsMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsActionResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub(crate) struct WsSessionHandle {
    url: String,
    connected_at: u64,
    open: bool,
    queue: Mutex<VecDeque<WsMessage>>,
    tx: Option<mpsc::UnboundedSender<String>>,
}

/// WS 会话注册表（State 注入）
pub struct WsState(pub(crate) Mutex<HashMap<String, WsSessionHandle>>);

fn snapshot(map: &HashMap<String, WsSessionHandle>, id: &str) -> Option<WsSession> {
    let h = map.get(id)?;
    let messages = h.queue.lock().ok()?.iter().cloned().collect();
    Some(WsSession {
        id: id.to_string(),
        url: h.url.clone(),
        connected_at: h.connected_at,
        open: h.open,
        messages,
    })
}

/// 建立 WS 连接并启动后台读写任务（消息进队列，send 走 channel）
#[tauri::command]
pub async fn ws_connect(
    app: AppHandle,
    state: State<'_, WsState>,
    url: String,
    headers: Option<Vec<(String, String)>>,
) -> Result<WsSession, String> {
    let mut req =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url.clone())
            .map_err(|e| e.to_string())?;
    if let Some(hs) = headers {
        for (k, v) in hs {
            if !k.trim().is_empty() {
                let name = k
                    .parse::<tokio_tungstenite::tungstenite::http::HeaderName>()
                    .map_err(|e| e.to_string())?;
                let value = v
                    .parse::<tokio_tungstenite::tungstenite::http::HeaderValue>()
                    .map_err(|e| e.to_string())?;
                req.headers_mut().insert(name, value);
            }
        }
    }

    let (ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .map_err(|e| e.to_string())?;
    let (mut write, mut read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let id = format!("ws-{}", now_ms());
    let connected_at = now_ms();

    {
        let mut map = state.0.lock().map_err(|e| e.to_string())?;
        map.insert(
            id.clone(),
            WsSessionHandle {
                url: url.clone(),
                connected_at,
                open: true,
                queue: Mutex::new(VecDeque::new()),
                tx: Some(tx),
            },
        );
    }

    let app2 = app.clone();
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                // 前端发送
                out = rx.recv() => {
                    match out {
                        Some(text) => {
                            if write.send(tokio_tungstenite::tungstenite::Message::Text(text)).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                // 服务端推送
                frame = read.next() => {
                    match frame {
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) => {
                            if let Ok(m) = app2.state::<WsState>().0.lock() {
                                if let Some(h) = m.get(&task_id) {
                                    if let Ok(mut q) = h.queue.lock() {
                                        q.push_back(WsMessage { direction: "received", content: t, time: now_ms() });
                                        if q.len() > 500 {
                                            q.pop_front();
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
            }
        }
        // 连接关闭：标记 open=false 并丢弃发送端
        if let Ok(mut m) = app2.state::<WsState>().0.lock() {
            if let Some(h) = m.get_mut(&task_id) {
                h.open = false;
                h.tx = None;
            }
        }
    });

    let map = state.0.lock().map_err(|e| e.to_string())?;
    Ok(snapshot(&map, &id).ok_or("会话创建失败")?)
}

/// 发送文本消息（写入后台 channel；发送记录由前端本地追加，服务端回显会再次收到）
#[tauri::command]
pub async fn ws_send(
    state: State<'_, WsState>,
    id: String,
    message: String,
) -> Result<WsActionResult, String> {
    let map = state.0.lock().map_err(|e| e.to_string())?;
    match map.get(&id).and_then(|h| h.tx.as_ref()) {
        Some(tx) => {
            tx.send(message).map_err(|e| e.to_string())?;
            Ok(WsActionResult {
                ok: true,
                message: None,
            })
        }
        None => Ok(WsActionResult {
            ok: false,
            message: Some("会话不存在或已断开".into()),
        }),
    }
}

/// 拉取会话快照（前端轮询展示消息）
#[tauri::command]
pub async fn ws_recv(state: State<'_, WsState>, id: String) -> Result<WsSession, String> {
    let map = state.0.lock().map_err(|e| e.to_string())?;
    snapshot(&map, &id).ok_or_else(|| "会话不存在".into())
}

/// 关闭会话（断开连接并清理）
#[tauri::command]
pub async fn ws_close(state: State<'_, WsState>, id: String) -> Result<WsActionResult, String> {
    let mut map = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(h) = map.get_mut(&id) {
        h.open = false;
        h.tx = None; // drop 发送端 → 后台 select 收到 None → 断开
    }
    map.remove(&id);
    Ok(WsActionResult {
        ok: true,
        message: None,
    })
}

/// 全部会话快照（含历史消息）
#[tauri::command]
pub async fn ws_sessions(state: State<'_, WsState>) -> Result<Vec<WsSession>, String> {
    let map = state.0.lock().map_err(|e| e.to_string())?;
    Ok(map.keys().filter_map(|k| snapshot(&map, k)).collect())
}
