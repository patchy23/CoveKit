//! 框架 · 长任务登记（可靠性 T11-1）
//!
//! 为什么需要：存储迁移这类框架长任务只往控制台打日志，界面既看不到「正在做什么」，
//! 也无法在离开页面后再回到当前状态。这里给出**最小**任务模型，只覆盖框架长任务
//! （存储迁移、更新），业务复杂状态机仍归插件，不在这层抽象里抢活。
//!
//! 容量规则（有界，避免长时间运行无限增长）：
//! - 活跃任务上限 [`MAX_ACTIVE_TASKS`]：达到上限时 `begin` 返回一个失败句柄
//!   （错误码 `task.capacity`），不登记、不覆盖已有任务；
//! - 已完成任务只保留最近 [`FINISHED_HISTORY_CAP`] 条，超出丢弃最旧的。
//!
//! 取消口径：框架长任务**当前都不可中途取消**（存储迁移中断会留下半成品，交给下次启动续跑），
//! 因此快照里如实声明 `cancellable = false` 与原因，界面据此禁用取消入口；
//! 真正需要可取消的长任务（更新下载/安装）由前端自己管理阶段，不在这层伪造取消成功。

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// 任务状态变化事件名（前端订阅后按 id 更新）
pub const TASK_EVENT: &str = "framework://task";

/// 活跃任务上限（框架自身并发很低，留出余量即可）
pub const MAX_ACTIVE_TASKS: usize = 8;

/// 已完成任务保留条数（诊断与「上次迁移结果」用得到）
pub const FINISHED_HISTORY_CAP: usize = 20;

/// 任务状态
///
/// 只保留真实产生的状态：登记即执行中（框架长任务是同步进入执行体的），
/// 结束只有成功与失败两种。可取消的长任务（更新下载）由前端自己管，
/// 不在这层伪造 `Pending` / `Cancelled`——没有产生者也没有消费者的状态只会误导界面。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskState {
    /// 执行中
    Running,
    /// 已成功
    Succeeded,
    /// 已失败（带错误码与说明）
    Failed,
}

impl TaskState {
    /// 是否已结束（不允许再变更状态）
    fn is_terminal(self) -> bool {
        matches!(self, TaskState::Succeeded | TaskState::Failed)
    }
}

/// 任务错误（稳定 code + 面向用户的 message）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    /// 稳定错误码（如 `storage.migrate.failed`），界面与诊断按它归类
    pub code: String,
    /// 面向用户的说明（可直接展示）
    pub message: String,
}

/// 任务快照（前端与诊断的统一视图）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    /// 任务 id（进程内唯一，形如 `t1`）
    pub id: String,
    /// 归属（框架模块名，如 `storage`）
    pub owner: String,
    /// 任务类型（如 `storage.migrate`）
    pub kind: String,
    /// 当前状态
    pub state: TaskState,
    /// 进度百分比（0..=100；不可估算时为 None，不假装 0 或 100）
    pub progress: Option<u8>,
    /// 是否允许取消
    pub cancellable: bool,
    /// 不可取消的原因（cancellable 为 true 时为空）
    pub cancellable_reason: Option<String>,
    /// 失败原因
    pub error: Option<TaskError>,
    /// 登记时间（epoch 毫秒）
    pub started_at: u64,
    /// 最近一次变更时间（epoch 毫秒）
    pub updated_at: u64,
}

/// 任务清单（命令返回值）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskList {
    /// 活跃任务（登记顺序）
    pub active: Vec<TaskSnapshot>,
    /// 已结束任务（最近在前）
    pub finished: Vec<TaskSnapshot>,
}

/// 内部登记项
#[derive(Debug, Clone)]
struct TaskRecord {
    snapshot: TaskSnapshot,
}

/// 登记表
#[derive(Debug, Default)]
struct Registry {
    /// 自增序号（生成 id）
    seq: u64,
    /// 活跃任务
    active: Vec<TaskRecord>,
    /// 已结束任务（最近在前）
    finished: VecDeque<TaskSnapshot>,
}

/// 全局登记表：进程内唯一（多窗口共用同一份任务视图）
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// 取锁：登记表只做短操作，锁中毒按「数据仍在」继续用内部值，不让一个 panic 永久废掉任务视图
fn lock() -> MutexGuard<'static, Registry> {
    match registry().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 当前时间（epoch 毫秒；系统时钟早于 1970 时退回 0）
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

/// 长任务句柄：登记、报进度、结束
#[derive(Debug, Clone)]
pub struct TaskHandle {
    /// 任务 id
    id: String,
    /// 是否因容量上限被拒绝登记（此时所有变更都是空操作）
    rejected: bool,
}

impl TaskHandle {
    /// 是否被容量上限拒绝登记
    pub fn is_rejected(&self) -> bool {
        self.rejected
    }

    /// 变更登记项并推送事件（拒绝登记的句柄为空操作）
    fn mutate<F>(&self, app: Option<&AppHandle>, change: F)
    where
        F: FnOnce(&mut TaskRecord),
    {
        if self.rejected {
            return;
        }
        let snapshot = {
            let mut guard = lock();
            let Some(record) = guard
                .active
                .iter_mut()
                .find(|item| item.snapshot.id == self.id)
            else {
                return;
            };
            // 已结束的任务不再接收变更：晚到的进度不该把结果改回运行中
            if record.snapshot.state.is_terminal() {
                return;
            }
            change(record);
            record.snapshot.updated_at = now_ms();
            record.snapshot.clone()
        };
        emit(app, &snapshot);
    }

    /// 上报进度（超过 100 截断，便于调用方直接用「已复制/总量」算出百分比）
    pub fn progress(&self, app: Option<&AppHandle>, percent: u64) {
        let value = percent.min(100) as u8;
        self.mutate(app, |record| {
            record.snapshot.progress = Some(value);
        });
    }

    /// 标记成功
    pub fn succeed(&self, app: Option<&AppHandle>) {
        self.finish(app, TaskState::Succeeded, None);
    }

    /// 标记失败（code 稳定、message 面向用户）
    pub fn fail(&self, app: Option<&AppHandle>, code: &str, message: &str) {
        self.finish(
            app,
            TaskState::Failed,
            Some(TaskError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        );
    }

    /// 结束任务并移入历史
    fn finish(&self, app: Option<&AppHandle>, state: TaskState, error: Option<TaskError>) {
        if self.rejected {
            return;
        }
        let snapshot = {
            let mut guard = lock();
            let Some(index) = guard
                .active
                .iter()
                .position(|item| item.snapshot.id == self.id)
            else {
                return;
            };
            let mut record = guard.active.remove(index);
            if record.snapshot.state.is_terminal() {
                return;
            }
            record.snapshot.state = state;
            record.snapshot.error = error;
            record.snapshot.updated_at = now_ms();
            if state == TaskState::Succeeded {
                // 成功时不保留进度之外的噪音：进度停在 100 更有信息量
                record.snapshot.progress = Some(100);
            }
            while guard.finished.len() >= FINISHED_HISTORY_CAP {
                guard.finished.pop_back();
            }
            let snapshot = record.snapshot.clone();
            guard.finished.push_front(snapshot.clone());
            snapshot
        };
        emit(app, &snapshot);
    }
}

/// 推送任务快照（前端未挂载时发不出去不是错误：前端挂载后会拉全量清单）
fn emit(app: Option<&AppHandle>, snapshot: &TaskSnapshot) {
    let Some(app) = app else {
        return;
    };
    if let Err(error) = app.emit(TASK_EVENT, snapshot) {
        eprintln!("[tasks] 任务事件发送失败：{error}");
    }
}

/// 登记一个长任务
///
/// @param app 用于推送事件的句柄（后台线程可传 None）
/// @param owner 归属模块（如 `storage`）
/// @param kind 任务类型（如 `storage.migrate`）
/// @param cancellable 是否允许取消；不允许时给出原因
pub fn begin(
    app: Option<&AppHandle>,
    owner: &str,
    kind: &str,
    cancellable: bool,
    cancellable_reason: Option<&str>,
) -> TaskHandle {
    let mut guard = lock();
    guard.seq += 1;
    let id = format!("t{}", guard.seq);
    if guard.active.len() >= MAX_ACTIVE_TASKS {
        // 容量上限：不登记、不覆盖，调用方通过 is_rejected + 失败句柄的语义知道没记上
        eprintln!("[tasks] 活跃任务已达上限 {MAX_ACTIVE_TASKS}，{kind} 未登记");
        return TaskHandle { id, rejected: true };
    }
    let now = now_ms();
    let snapshot = TaskSnapshot {
        id: id.clone(),
        owner: owner.to_string(),
        kind: kind.to_string(),
        state: TaskState::Running,
        progress: None,
        cancellable,
        cancellable_reason: if cancellable {
            None
        } else {
            cancellable_reason.map(str::to_string)
        },
        error: None,
        started_at: now,
        updated_at: now,
    };
    guard.active.push(TaskRecord {
        snapshot: snapshot.clone(),
    });
    drop(guard);
    emit(app, &snapshot);
    TaskHandle {
        id,
        rejected: false,
    }
}

/// 当前任务清单（活跃在前，历史最近在前）
pub fn list() -> TaskList {
    let guard = lock();
    TaskList {
        active: guard
            .active
            .iter()
            .map(|item| item.snapshot.clone())
            .collect(),
        finished: guard.finished.iter().cloned().collect(),
    }
}

/// 查询任务清单（前端订阅事件之外的全量拉取，用于进入界面时对齐状态）
#[tauri::command]
pub fn framework_tasks() -> TaskList {
    list()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用例之间共享全局登记表，逐个清理避免串味
    fn reset() {
        let mut guard = lock();
        guard.active.clear();
        guard.finished.clear();
        guard.seq = 0;
    }

    #[test]
    fn begin_reports_running_without_fake_progress() {
        reset();
        let _handle = begin(
            None,
            "storage",
            "storage.migrate",
            false,
            Some("迁移不能中断"),
        );
        let list = list();
        assert_eq!(list.active.len(), 1);
        let task = &list.active[0];
        assert_eq!(task.id, list.active[0].id);
        assert_eq!(task.state, TaskState::Running);
        assert_eq!(task.progress, None, "不可估算时必须为空，不能塞 0");
        assert!(!task.cancellable);
        assert_eq!(task.cancellable_reason.as_deref(), Some("迁移不能中断"));
    }

    #[test]
    fn progress_is_clamped_and_finish_moves_to_history() {
        reset();
        let handle = begin(None, "storage", "storage.migrate", false, None);
        handle.progress(None, 42);
        assert_eq!(list().active[0].progress, Some(42));
        handle.progress(None, 250);
        assert_eq!(list().active[0].progress, Some(100));

        handle.succeed(None);
        let list = list();
        assert!(list.active.is_empty());
        assert_eq!(list.finished.len(), 1);
        assert_eq!(list.finished[0].state, TaskState::Succeeded);
        assert_eq!(list.finished[0].progress, Some(100));
    }

    #[test]
    fn failure_keeps_code_and_message() {
        reset();
        let handle = begin(None, "storage", "storage.migrate", false, None);
        handle.fail(None, "storage.migrate.failed", "复制校验失败");
        let task = &list().finished[0];
        assert_eq!(task.state, TaskState::Failed);
        let error = task.error.as_ref().expect("失败任务必须带错误");
        assert_eq!(error.code, "storage.migrate.failed");
        assert_eq!(error.message, "复制校验失败");
    }

    #[test]
    fn late_progress_after_finish_does_not_resurrect_task() {
        reset();
        let handle = begin(None, "storage", "storage.migrate", false, None);
        handle.fail(None, "storage.migrate.failed", "失败");
        handle.progress(None, 80);
        let list = list();
        assert!(list.active.is_empty(), "已结束任务不能被进度改回活跃");
        assert_eq!(list.finished[0].progress, None);
        assert_eq!(list.finished[0].state, TaskState::Failed);
    }

    #[test]
    fn finished_history_is_bounded_and_newest_first() {
        reset();
        for index in 0..FINISHED_HISTORY_CAP + 5 {
            let handle = begin(None, "storage", "storage.migrate", false, None);
            handle.progress(None, index as u64);
            handle.succeed(None);
        }
        let list = list();
        assert_eq!(list.finished.len(), FINISHED_HISTORY_CAP);
        // 最近完成的在前面
        assert!(list.finished[0].started_at >= list.finished[1].started_at);
    }

    #[test]
    fn active_tasks_are_capped_without_overwriting() {
        reset();
        for _ in 0..MAX_ACTIVE_TASKS {
            let handle = begin(None, "storage", "storage.migrate", false, None);
            assert!(!handle.is_rejected());
        }
        let rejected = begin(None, "storage", "storage.migrate", false, None);
        assert!(rejected.is_rejected(), "达到上限时必须拒绝登记");
        assert_eq!(list().active.len(), MAX_ACTIVE_TASKS);
        // 被拒绝的句柄是空操作，不影响已有任务
        rejected.progress(None, 50);
        rejected.fail(None, "task.capacity", "超出上限");
        assert_eq!(list().active.len(), MAX_ACTIVE_TASKS);
    }
}
