//! SSH 插件 · 端口隧道（-L/-R/-D）
//! 配置存 ssh.db（随 profile）；运行句柄存 TunnelState（desired_running 支撑断线自动恢复）。
//! 模块划分：ftargets = -R 目标表；lifecycle = 命令与连接生命周期联动；forward = 数据面。

pub(crate) mod forward;
pub(crate) mod ftargets;
pub(crate) mod lifecycle;

pub use ftargets::{new_forward_targets, ForwardTargets};
pub(crate) use lifecycle::{resume_for_profile, stop_session_tunnels};

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::plugins::ssh::models::{TunnelConfig, TunnelRuntime, TunnelStatus};
use crate::plugins::ssh::store::{self, ProfileState};

/// with_db 的裸数据版（ProfileState 不经 State 包装时使用；逻辑与 store::with_db 一致）
pub(crate) fn with_db_data<T>(
    app: &AppHandle,
    state: &ProfileState,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let arc = {
        let mut slot = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(db) = slot.as_ref() {
            db.clone()
        } else {
            let db = Arc::new(crate::framework::store::PluginDb::open(
                app,
                "ssh",
                store::MIGRATIONS,
            )?);
            *slot = Some(db.clone());
            db
        }
    };
    arc.with_conn(f)
}

/// 隧道运行时注册表：tunnelId → 句柄（断开后保留，承载 desired_running 供重连恢复）
pub struct TunnelState(pub Mutex<HashMap<String, Arc<TunnelHandle>>>);

/// 单条隧道运行时句柄
pub(crate) struct TunnelHandle {
    /// 隧道配置（运行期不可变；改配置 = 停止后重新启动）
    pub(crate) config: TunnelConfig,
    /// 绑定的连接会话 id（重连后更新）
    pub(crate) connection_id: Mutex<String>,
    /// 当前状态
    pub(crate) status: Mutex<TunnelStatus>,
    /// 异常信息（status=error 时）
    pub(crate) error: Mutex<Option<String>>,
    /// 活动连接计数（-R 与 handler 回调共享）
    pub(crate) connections: Arc<AtomicU64>,
    /// 期望运行：start=true / 用户 stop=false；断线不改写
    pub(crate) desired_running: AtomicBool,
    /// 停止信号：发送 true 或 drop 都使监听任务退出
    pub(crate) cancel_tx: Mutex<Option<watch::Sender<bool>>>,
    /// 本地监听任务（remote 类型为 None）
    pub(crate) listener: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl TunnelHandle {
    /// 构造运行时快照
    fn runtime(&self) -> TunnelRuntime {
        TunnelRuntime {
            config: self.config.clone(),
            status: self
                .status
                .lock()
                .map(|s| *s)
                .unwrap_or(TunnelStatus::Stopped),
            connections: self.connections.load(Ordering::Relaxed),
            error: self.error.lock().ok().and_then(|e| e.clone()),
        }
    }

    /// 更新状态并推送事件（尽力而为）
    fn set_status(&self, app: &AppHandle, status: TunnelStatus, error: Option<String>) {
        if let Ok(mut slot) = self.status.lock() {
            *slot = status;
        }
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
        let _ = app.emit("ssh://tunnel-status", &self.runtime());
    }
}

#[cfg(test)]
mod tests {
    use crate::plugins::ssh::models::TunnelType;

    #[test]
    fn tunnel_type_roundtrip() {
        for t in [TunnelType::Local, TunnelType::Remote, TunnelType::Dynamic] {
            assert_eq!(TunnelType::from_str(t.as_str()), t);
        }
        assert_eq!(TunnelType::from_str("bogus"), TunnelType::Local);
    }
}
