//! 接口管理插件 · 数据结构（serde，与前端 plugins/http-ws/contracts.ts 的 ApiRecord 同步）
//! 本文件只声明类型，不包含任何逻辑；命令与持久化见 mod.rs。

use serde::Serialize;

/// 接口列表记录（一条 = 一个可保存/切换的 HTTP 或 WS 接口）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRecord {
    /// 主键 id（0 = 尚未持久化的新记录）
    pub(crate) id: i64,
    /// 接口类型：http / ws（决定前端加载到哪个面板视图）
    #[serde(rename = "type")]
    pub(crate) kind: String,
    /// 接口名称（用户命名，列表展示用）
    pub(crate) name: String,
    /// HTTP 方法或 WEBSOCKET（前端方法下拉的原值）
    pub(crate) method: String,
    /// 请求地址（http/https/ws/wss）
    pub(crate) url: String,
    /// Params 键值行的 JSON 字符串（前端 KvRow[] 序列化）
    pub(crate) params: String,
    /// Headers 键值行的 JSON 字符串（前端 KvRow[] 序列化）
    pub(crate) headers: String,
    /// 请求体模式：none / json / text
    pub(crate) body_mode: String,
    /// 请求体内容
    pub(crate) body: String,
    /// 最近保存时间（SQLite datetime，本地时间文本）
    pub(crate) updated_at: String,
}
