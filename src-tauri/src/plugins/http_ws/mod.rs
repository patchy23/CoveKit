//! HTTP/WS 调试插件 · 门面
//! 命令函数（薄层）与插件装配在此；实现拆分：
//! - models.rs：serde 数据结构（与前端 plugins/http-ws/contracts.ts 同步）
//! - http.rs：HTTP 请求实现（reqwest）
//! - ws.rs：WebSocket 会话注册表与收发实现（tokio-tungstenite）

mod http;
mod models;
mod ws;

pub use ws::WsState;

/// 插件注册：命令 + WS 会话 State（惰性初始化）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        ("http_request", "发送 HTTP 请求"),
        ("ws_connect", "建立 WebSocket 连接（支持自定义请求头）"),
        ("ws_send", "发送 WS 消息"),
        ("ws_recv", "拉取会话快照"),
        ("ws_close", "关闭 WS 会话"),
        ("ws_sessions", "全部 WS 会话"),
    ])
    .expect("IPC 命令重复注册");
    builder
        .invoke_handler(tauri::generate_handler![
            http::http_request,
            ws::ws_connect,
            ws::ws_send,
            ws::ws_recv,
            ws::ws_close,
            ws::ws_sessions
        ])
        .manage(WsState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
}
