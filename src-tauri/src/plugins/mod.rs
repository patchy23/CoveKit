// 插件清单（与前端 src/plugins/index.ts 一一对应，前端插件 ⇄ 后端插件同名同边界）
// 每个插件统一目录结构：<id>/mod.rs（门面）+ models.rs（serde）+ 能力子模块
// 新增插件 = plugins/<id>/ 目录 + 本文件一行 + lib.rs register/init 各一行。
// 框架级能力（设置/快捷键/窗口/数据管理）在 framework/，不属于插件。
pub mod api;
pub mod db;
pub mod dns;
pub mod hosts;
pub mod http_ws;
