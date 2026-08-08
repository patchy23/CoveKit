//! HTTP/WS 调试插件 · WebSocket 会话（tokio-tungstenite）
//! 会话注册表：id → 后台读写任务（tokio::select）+ 消息队列（上限 500 条）
//! 建立连接支持自定义请求头（握手注入 Upgrade）；send 走 mpsc channel。

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Manager, State};
use tokio::sync::mpsc;

use crate::plugins::http_ws::models::{WsActionResult, WsMessage, WsSession};

pub(crate) struct WsSessionHandle {
    url: String,
    connected_at: u64,
    open: bool,
    queue: Mutex<VecDeque<WsMessage>>,
    tx: Option<mpsc::UnboundedSender<String>>,
}

/// WS 会话注册表（State 注入）
pub struct WsState(pub(crate) Mutex<HashMap<String, WsSessionHandle>>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

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
