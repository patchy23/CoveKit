//! hosts 修改插件 · 数据结构（serde，与前端 plugins/hosts/contracts.ts 同步）
//! 本文件只声明类型，不包含任何逻辑；读取/提权写入见 mod.rs。

use serde::Serialize;

/// hosts 读取/保存的统一结果（ok=false 时 error 携带原因，供前端直接展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsResult {
    /// 操作是否成功
    pub(crate) ok: bool,
    /// 保存成功后回读的完整 hosts 内容（失败时为空）
    pub(crate) content: String,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
