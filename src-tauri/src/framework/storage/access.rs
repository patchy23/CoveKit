//! 根迁移的业务准入与排空屏障。取消信号不释放租约，实际执行者退出才释放。

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Default)]
struct State {
    frozen: bool,
    active: usize,
}

struct Gate {
    state: Mutex<State>,
    changed: tokio::sync::Notify,
}

impl Gate {
    const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                frozen: false,
                active: 0,
            }),
            changed: tokio::sync::Notify::const_new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // 临界区只维护内存计数，保留中毒后的真实计数，不能清零假装已排空。
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn operation(&'static self) -> Result<Operation, String> {
        let mut state = self.state();
        if state.frozen {
            return Err("正在切换数据目录，请稍候重试".into());
        }
        state.active += 1;
        Ok(Operation(self))
    }

    fn retain(&'static self) -> Operation {
        self.state().active += 1;
        Operation(self)
    }

    fn freeze(&'static self) -> Result<FreezeGuard, String> {
        let mut state = self.state();
        if state.frozen {
            return Err("已有数据目录维护正在执行".into());
        }
        state.frozen = true;
        Ok(FreezeGuard(self))
    }
}

static GATE: Gate = Gate::new();

/// 一项真正执行中的操作；必须留在命令 future 或实际后台工作体内。
pub(crate) struct Operation(&'static Gate);

impl Drop for Operation {
    fn drop(&mut self) {
        let mut state = self.0.state();
        state.active -= 1;
        drop(state);
        self.0.changed.notify_waiters();
    }
}

/// 新业务的唯一准入；检查冻结与登记计数在同一临界区。
pub(crate) fn operation() -> Result<Operation, String> {
    GATE.operation()
}

/// 已获准的执行者派生后台工作；调用方须仍持租约，不能用于新业务准入。
pub(crate) fn retain() -> Operation {
    GATE.retain()
}

/// 维护控制与只读诊断无需业务租约；其余命令由模块清单统一接线。
pub(crate) fn dispatch(command: &str) -> Result<Option<Operation>, String> {
    if matches!(
        command,
        "storage_migrate_now"
            | "storage_recovery_action"
            | "storage_recovery_status"
            | "window_toggle"
            | "window_hide"
            | "framework_tasks"
            | "resource_monitor_snapshot"
    ) {
        Ok(None)
    } else {
        operation().map(Some)
    }
}

/// 冻结期间禁止新准入；失败或超时不自行解除，交由维护调用栈退出释放。
pub(crate) struct FreezeGuard(&'static Gate);

/// 维护入口取得唯一冻结权，后续业务必须等守卫释放才能进入。
pub(crate) fn freeze() -> Result<FreezeGuard, String> {
    GATE.freeze()
}

impl FreezeGuard {
    /// 先登记唤醒再检查计数，避免最后一个执行者退出与等待之间丢失通知。
    pub(crate) async fn wait_idle(&self, timeout: Duration) -> Result<(), String> {
        tokio::time::timeout(timeout, async {
            loop {
                let changed = self.0.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if self.0.state().active == 0 {
                    return;
                }
                changed.await;
            }
        })
        .await
        .map_err(|_| "后台操作尚未结束，数据目录未切换，请稍后重试".to_string())
    }
}

impl Drop for FreezeGuard {
    fn drop(&mut self) {
        self.0.state().frozen = false;
        self.0.changed.notify_waiters();
    }
}

/// 阻塞任务的租约进入工作闭包；外层 future 被取消也不能提前宣告排空。
pub(crate) fn spawn_blocking<F, R>(work: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let operation = retain();
    tokio::task::spawn_blocking(move || {
        let _operation = operation;
        work()
    })
}

/// 保持 Tauri 阻塞运行时的句柄类型和启动期运行时解析行为。
pub(crate) fn spawn_blocking_tauri<F, R>(work: F) -> tauri::async_runtime::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let operation = retain();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        work()
    })
}

/// 已准入操作派生的异步写盘任务，租约覆盖任务收尾而非仅覆盖 spawn 调用。
pub(crate) fn spawn<F>(work: F) -> tauri::async_runtime::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let operation = retain();
    tauri::async_runtime::spawn(async move {
        let _operation = operation;
        work.await
    })
}

/// 保持 Tokio 运行时句柄类型，供持有监控任务句柄的 owner 使用。
pub(crate) fn spawn_tokio<F>(work: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let operation = retain();
    tokio::spawn(async move {
        let _operation = operation;
        work.await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn freeze_waits_for_actual_worker_and_keeps_closed_after_timeout() {
        static GATE: Gate = Gate::new();
        let parent = GATE.operation().unwrap();
        let freeze = GATE.freeze().unwrap();
        assert!(GATE.operation().is_err());
        assert!(GATE.freeze().is_err());
        // 冻结之前已准入的操作仍可把收尾责任交给工作线程。
        let worker = GATE.retain();
        drop(parent);
        assert!(freeze.wait_idle(Duration::ZERO).await.is_err());
        assert!(GATE.operation().is_err());
        drop(worker);
        freeze.wait_idle(Duration::from_secs(1)).await.unwrap();
        assert!(GATE.operation().is_err());
        drop(freeze);
        assert!(GATE.operation().is_ok());
    }

    #[tokio::test]
    async fn abandoned_blocking_waiter_does_not_release_worker() {
        static GATE: Gate = Gate::new();
        let operation = GATE.operation().unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let work = tokio::task::spawn_blocking(move || {
            let _operation = operation;
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
        });
        started_rx.await.unwrap();
        work.abort();
        let freeze = GATE.freeze().unwrap();
        assert!(freeze.wait_idle(Duration::ZERO).await.is_err());
        finish_tx.send(()).unwrap();
        freeze.wait_idle(Duration::from_secs(1)).await.unwrap();
    }
}
