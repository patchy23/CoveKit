//! HTTP/WS 调试插件 · 门面
//! 命令函数（薄层）与插件装配在此；实现拆分：
//! - models.rs：serde 数据结构（与前端 plugins/http-ws/contracts.ts 同步）
//! - http.rs：HTTP 请求实现（reqwest）
//! - ws.rs：WebSocket 会话注册表与收发实现（tokio-tungstenite）
//! - persistence/：接口库持久化（原 plugins/api 模块，2026-09-12 归入本门面；
//!   数据文件仍为 `data/api.db`，命令名与前端契约不变）

pub(crate) mod http;
mod models;
pub(crate) mod persistence;
pub(crate) mod ws;

pub use ws::WsState;

use tauri::Manager;

// 模块静态清单：命令名、入库元数据与分派 handler 同源生成（AR07 §10.2）
// `feature: "http-ws"` 为前端稳定 feature id；`storage: "api"` 记录接口库历史存储键。
crate::patchybox_module! {
    owner: "http_ws",
    feature: "http-ws",
    storage: "api",
    commands: {
        http::http_request => "发送 HTTP 请求",
        ws::ws_connect => "建立 WebSocket 连接（支持自定义请求头）",
        ws::ws_send => "发送 WS 消息",
        ws::ws_recv => "拉取会话快照",
        ws::ws_close => "关闭 WS 会话",
        ws::ws_sessions => "全部 WS 会话",
        persistence::api_save => "保存/更新接口（id=0 新增）",
        persistence::api_list => "接口列表",
        persistence::api_delete => "删除接口",
        persistence::api_clear => "清空全部接口",
    },
}

/// 清理（生命周期钩子，页签关闭与退出都会调用）：断开全部 WS 会话并清空注册表
/// （幂等，无会话时不算失败）。
///
/// 页签作用域的理由：WS 会话挂在工具页签上，关掉页签后没有任何界面能看到或断开它，
/// 连接会一直挂着，所以关页签即断开。
fn on_dispose(
    app: Option<&tauri::AppHandle>,
    _reason: crate::framework::lifecycle::CloseReason,
) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    let state = app.state::<WsState>();
    ws::close_all_sessions(&state);
    Vec::new()
}

/// 插件注册：命令入库（清单生成）+ WS 会话与接口库 State（惰性初始化）
/// + 退出清理登记（唯一关闭入口，见 framework/lifecycle.rs）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    // 工具 id 用前端工具 id（`http-ws`），不是模块目录名 —— `reason=tab` 时按它筛 owner
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::for_tool(IPC_OWNER, "http-ws")
            .with_tab_scope()
            .with_dispose(on_dispose),
    );
    let builder = persistence::register_state(builder);
    builder.manage(WsState(std::sync::Mutex::new(
        std::collections::HashMap::new(),
    )))
}
