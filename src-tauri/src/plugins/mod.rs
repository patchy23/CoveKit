// 插件清单（与前端 src/plugins/index.ts 一一对应，前端插件 ⇄ 后端插件同名同边界）
// 每个插件统一目录结构：<id>/mod.rs（门面）+ models.rs（serde）+ 能力子模块
// 新增插件 = plugins/<id>/ 目录 + 本文件一行 + lib.rs register/init 各一行。
// 框架级能力（设置/快捷键/窗口/数据管理）在 framework/，不属于插件。
pub mod api;
pub mod database;
pub mod dns;
pub mod hosts;
pub mod http_ws;
pub mod ssh;
pub mod tts;

/// 判断命令是否属于业务插件，供唯一的应用级 invoke_handler 路由。
pub(crate) fn is_command(command: &str) -> bool {
    command.starts_with("http_")
        || command.starts_with("ws_")
        || command.starts_with("api_")
        || command.starts_with("dbc_")
        || command.starts_with("hosts_")
        || command.starts_with("dns_")
        || command.starts_with("ssh_")
        || command.starts_with("tts_")
}

/// 按插件命令前缀分派到插件私有 handler，避免 Builder::invoke_handler 互相覆盖。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    match invoke.message.command() {
        command if command.starts_with("http_") || command.starts_with("ws_") => {
            http_ws::invoke_handler(invoke)
        }
        command if command.starts_with("api_") => api::invoke_handler(invoke),
        command if command.starts_with("dbc_") => database::invoke_handler(invoke),
        command if command.starts_with("hosts_") => hosts::invoke_handler(invoke),
        command if command.starts_with("dns_") => dns::invoke_handler(invoke),
        command if command.starts_with("ssh_") => ssh::invoke_handler(invoke),
        command if command.starts_with("tts_") => tts::invoke_handler(invoke),
        _ => false,
    }
}
