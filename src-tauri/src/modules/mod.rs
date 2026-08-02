// 业务模块（第一批 + 第二批首批）：settings / clipboard / color / http_ws / db / hosts
// 模块边界按第二批形状划分（架构 §4.3），第二批只填实现不改框架。
pub mod clipboard;
pub mod color;
pub mod db;
pub mod hosts;
pub mod http_ws;
pub mod settings;
