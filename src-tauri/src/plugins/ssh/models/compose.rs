//! Compose 项目与命令结果；与前端 SSH Compose 契约同步。

use serde::{Deserialize, Serialize};

/// Docker 查询的项目快照；配置顺序决定 Compose 合并顺序。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProject {
    /// Docker Compose 项目名称。
    pub(crate) name: String,
    /// Docker 查询返回的状态摘要。
    pub(crate) status: String,
    /// 有序的远程配置文件路径。
    pub(crate) config_files: Vec<String>,
}

/// 非交互 Compose 命令的实际退出码与分流输出。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeOutput {
    /// 远程进程实际退出码。
    pub(crate) exit_code: u32,
    /// 标准输出；项目清单仅从此解析。
    pub(crate) stdout: String,
    /// 标准错误与警告，界面原样展示。
    pub(crate) stderr: String,
}
