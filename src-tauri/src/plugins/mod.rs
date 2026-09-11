// 插件清单（与前端 src/plugins/index.ts 一一对应，前端插件 ⇄ 后端插件同名同边界）
// 每个插件统一目录结构：<id>/mod.rs（门面）+ models.rs（serde）+ 能力子模块
// 新增插件 = plugins/<id>/ 目录 + 本文件一行 + lib.rs register/init 各一行。
// 框架级能力（设置/快捷键/窗口/数据管理）在 framework/，不属于插件。
pub mod api;
pub mod database;
pub mod dns;
pub mod frp;
pub mod hosts;
pub mod http_ws;
pub mod ssh;
pub mod tts;

use crate::framework::ipc_registry;

/// 判断命令是否属于业务插件（注册表精确匹配；未入库或归属 framework 均为否）。
pub(crate) fn is_command(command: &str) -> bool {
    matches!(ipc_registry::owner_of(command), Some(owner) if owner != "framework")
}

/// 按注册表归属者分派到插件私有 handler，避免 Builder::invoke_handler 互相覆盖。
/// 路由依据 register() 时的 owner 登记——新增插件只需照常登记命令，无需再维护前缀清单。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    match ipc_registry::owner_of(invoke.message.command()) {
        Some("http_ws") => http_ws::invoke_handler(invoke),
        Some("api") => api::invoke_handler(invoke),
        Some("database") => database::invoke_handler(invoke),
        Some("hosts") => hosts::invoke_handler(invoke),
        Some("dns") => dns::invoke_handler(invoke),
        Some("frp") => frp::invoke_handler(invoke),
        Some("ssh") => ssh::invoke_handler(invoke),
        Some("tts") => tts::invoke_handler(invoke),
        _ => false,
    }
}

/// 启动校验：注册表中每个插件 owner 在 invoke_handler 都有分派分支。
/// 在全部插件 register 之后调用；发现死命令（登记了但没有路由分支）立即 panic（fail-fast）。
pub(crate) fn validate_routing() {
    for entry in ipc_registry::snapshot() {
        if entry.owner == "framework" {
            continue;
        }
        let routable = matches!(
            entry.owner,
            "http_ws" | "api" | "database" | "hosts" | "dns" | "frp" | "ssh" | "tts"
        );
        assert!(
            routable,
            "IPC 命令 {} 的归属者 {} 没有路由分支（plugins/mod.rs invoke_handler 缺分支）",
            entry.name, entry.owner
        );
    }
}
