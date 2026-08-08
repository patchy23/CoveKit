//! hosts 修改插件 · 数据结构（serde，与前端 plugins/hosts/contracts.ts 同步）

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsResult {
    pub(crate) ok: bool,
    pub(crate) content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
