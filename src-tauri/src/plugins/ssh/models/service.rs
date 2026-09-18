//! SSH 数据模型 · service

use serde::Serialize;

/* ── 服务管理 ── */

/// systemd 服务条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemdService {
    /// unit 主文件路径；用于区分系统安装与用户自定义服务。
    pub(crate) fragment_path: Option<String>,
    /// 有 drop-in 定制时保留在业务列表中。
    pub(crate) has_overrides: bool,
    /// 服务名（如 nginx.service）
    pub(crate) name: String,
    /// 描述
    pub(crate) description: String,
    /// 加载状态（loaded / not-found / masked）
    pub(crate) load_state: String,
    /// 活动状态（active / inactive / failed）
    pub(crate) active_state: String,
    /// 子状态（running / dead / exited）
    pub(crate) sub_state: String,
    /// 是否开机自启
    pub(crate) enabled: bool,
}
