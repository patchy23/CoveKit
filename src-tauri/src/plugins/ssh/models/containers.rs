//! SSH 数据模型 · containers

use serde::Serialize;

/* ── Docker ── */

/// Docker 容器条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    /// 容器 ID（短）
    pub(crate) id: String,
    /// 容器名
    pub(crate) name: String,
    /// 镜像名
    pub(crate) image: String,
    /// 状态（running / exited / paused）
    pub(crate) status: String,
    /// 本次运行持续时间（来自 docker ps Status）
    pub(crate) uptime: String,
    /// 端口映射（如 "80:80,443:443"）
    pub(crate) ports: String,
    /// 创建时间（毫秒时间戳）
    pub(crate) created_at: u64,
}

/// Docker 日志条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 契约类型：docker 日志列表，当前版本未构造（v1 返回原始文本）
pub struct DockerLog {
    /// 容器 ID
    pub(crate) container_id: String,
    /// 日志内容
    pub(crate) content: String,
    /// 时间戳
    pub(crate) time: u64,
}
