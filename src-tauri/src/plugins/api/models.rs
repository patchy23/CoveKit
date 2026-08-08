//! 接口管理插件 · 数据结构（serde，与前端 plugins/http-ws/contracts.ts 的 ApiRecord 同步）

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRecord {
    pub(crate) id: i64,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) params: String,
    pub(crate) headers: String,
    pub(crate) body_mode: String,
    pub(crate) body: String,
    pub(crate) updated_at: String,
}
