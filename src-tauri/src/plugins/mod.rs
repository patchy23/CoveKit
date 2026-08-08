// 插件清单（与前端 src/plugins/index.ts 一一对应，前端插件 ⇄ 后端插件同名同边界）
// 每个插件自包含：命令 + State（register 自注册）与启动初始化（init，可选）。
// 新增插件 = plugins/<id>.rs（或 <id>/mod.rs 目录）+ 本文件一行 + lib.rs 装配一行。
pub mod api;
pub mod clipboard;
pub mod color;
pub mod db;
pub mod hosts;
pub mod http_ws;
pub mod settings;
