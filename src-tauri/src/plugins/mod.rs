//! 插件清单（与前端 src/plugins/index.ts 一一对应，前端插件 ⇄ 后端插件同名同边界）
//! 每个插件统一目录结构：<id>/mod.rs（门面）+ models.rs（serde）+ 能力子模块
//! 新增插件 = plugins/<id>/ 目录 + 本文件 `patchybox_routes!` 一行（路由分支、装配顺序与
//! 启动校验都由该行生成，不必再手写 match 或 owner 清单）。
//! 框架级能力（设置/窗口/数据管理）在 framework/，不属于插件。

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

// 路由与装配清单（AR07 §10.2）：`owner => 模块` 一行同时给出
// 路由分支、装配顺序与启动校验数据源；owner 字面量必须与模块 `patchybox_module!` 声明一致。
crate::patchybox_routes! {
    "http_ws" => http_ws,
    "database" => database,
    "hosts" => hosts,
    "dns" => dns,
    "frp" => frp,
    "ssh" => ssh,
    "tts" => tts,
}
