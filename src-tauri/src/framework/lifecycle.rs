//! 统一关闭协调与维护入口（AR06 §9.2）
//!
//! 关闭只有这一个入口，按 `reason` 区分场景，并分成两个阶段：
//! - **prepare**：只询问、不清理，允许业务拒绝（未保存、任务进行中）——拒绝则应用不关闭；
//! - **dispose**：仅在允许关闭后执行，各模块清理自己的协议与进程，应用只汇总结果与总超时。
//!
//! 不变式：
//! - 业务协议清理由各模块提供（`ModuleLifecycle`），应用不替模块实现清理；
//! - dispose 有总超时：超时按失败计入并继续退出，不无限等待（前台按钮禁用不是锁）；
//! - 钩子 panic 会被兜住并计入失败，绝不让单个模块的异常阻断整个退出流程；
//! - 根迁移/导入提交/空间激活/更新安装的互斥不在本模块，见 `context::maintenance_guard()`。
use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tauri::AppHandle;

/// 关闭原因：唯一入口据此区分场景（页签/退出/重启/更新/空间切换）
///
/// 契约枚举：应用自身产生 `Exit`（托盘/界面退出）；`update` / `restart` / `space-switch`
/// 由更新安装与空间激活路径传入；`tab` 供工具页签关闭复用同一套 prepare/dispose 语义。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// 关闭单个工具页签（可能伴随未保存内容）
    Tab,
    /// 退出应用
    Exit,
    /// 重启应用（更新安装或布局切换后）
    Restart,
    /// 安装更新前退出
    Update,
    /// 切换数据空间
    SpaceSwitch,
}

impl CloseReason {
    /// 稳定字符串（日志与前端传递用；新增变体不得复用旧字符串）
    pub fn code(self) -> &'static str {
        match self {
            Self::Tab => "tab",
            Self::Exit => "exit",
            Self::Restart => "restart",
            Self::Update => "update",
            Self::SpaceSwitch => "space-switch",
        }
    }

    /// 由稳定字符串解析（未知值返回 None，调用方给明确报错，不猜默认值）
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "tab" => Some(Self::Tab),
            "exit" => Some(Self::Exit),
            "restart" => Some(Self::Restart),
            "update" => Some(Self::Update),
            "space-switch" => Some(Self::SpaceSwitch),
            _ => None,
        }
    }
}

/// prepare 钩子：`Err(原因)` = 拒绝关闭，原因文案可直接展示给用户。
///
/// `app` 在生产路径上总是 `Some`；`None` 只出现在不持有 `AppHandle` 的单元测试里，
/// 钩子实现需容忍该情况（直接返回 Ok / 空清理结果即可）。
pub type PrepareHook = fn(Option<&AppHandle>, CloseReason) -> Result<(), String>;
/// dispose 钩子：返回清理失败描述（空 = 成功）。不得 panic（panic 会被兜住并计入失败）。
pub type DisposeHook = fn(Option<&AppHandle>, CloseReason) -> Vec<String>;

/// 模块生命周期登记项：清理由模块自己提供，应用只协调
#[derive(Clone, Copy)]
pub struct ModuleLifecycle {
    /// 归属者 id（与 IPC owner 一致，失败信息里用于定位模块）
    pub owner: &'static str,
    /// 关闭前询问（可选）：返回 Err 拒绝关闭
    pub prepare: Option<PrepareHook>,
    /// 允许关闭后的清理（可选）：返回失败描述
    pub dispose: Option<DisposeHook>,
}

/// dispose 阶段总超时：超时按失败计入，不无限等待
pub const DISPOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// 用户显式强退标记（进程级）：置位后 prepare 不再拦截，清理阶段仍受总超时约束。
///
/// 只允许由用户显式动作（`exit::app_force_exit`）置位——业务代码不得自行置位，
/// 否则「未保存内容」这类拦截会被内部路径悄悄绕过。
static FORCE_EXIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 设置强退标记（用户显式动作；重复调用无副作用）
pub fn set_force_exit() {
    FORCE_EXIT.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// 是否处于用户强退流程
pub fn force_requested() -> bool {
    FORCE_EXIT.load(std::sync::atomic::Ordering::SeqCst)
}

/// 测试期复位强退标记（生产不调用）：用例之间必须互不污染
#[cfg(test)]
pub(crate) fn clear_force_for_test() {
    FORCE_EXIT.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// 已登记的模块钩子（进程级，启动期一次性登记）
static HOOKS: Mutex<Vec<ModuleLifecycle>> = Mutex::new(Vec::new());

/// 取钩子表；锁中毒只说明此前有钩子 panic 被兜住，表本身仍可读
fn hooks() -> MutexGuard<'static, Vec<ModuleLifecycle>> {
    match HOOKS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 登记模块生命周期钩子（启动期每模块一次；同 owner 再次登记按覆盖处理，不 panic）
pub fn register(spec: ModuleLifecycle) {
    let mut table = hooks();
    if let Some(existing) = table.iter_mut().find(|item| item.owner == spec.owner) {
        *existing = spec;
        return;
    }
    table.push(spec);
}

/// 已登记钩子的 owner 列表（诊断与测试）
#[cfg(test)]
pub fn registered_owners() -> Vec<&'static str> {
    hooks().iter().map(|item| item.owner).collect()
}

/// 测试期清空钩子表（生产不调用）：用例之间必须互不污染
#[cfg(test)]
pub(crate) fn clear_for_test() {
    hooks().clear();
}

/// prepare 阶段结果：`proceed=false` 时 `blockers` 为拒绝原因（按登记顺序）
#[derive(Debug, Default)]
pub struct PrepareOutcome {
    /// 是否允许继续关闭（存在拒绝原因时为 false）
    pub proceed: bool,
    /// 拒绝原因（形如 `owner: 原因`，按登记顺序）
    pub blockers: Vec<String>,
}

/// prepare 阶段：询问全部模块，任一拒绝则不允许关闭（不做任何清理）
pub fn prepare_close(app: &AppHandle, reason: CloseReason) -> PrepareOutcome {
    prepare_close_with(Some(app), reason)
}

/// prepare 阶段的实现（`app` 可为 None，供单元测试驱动协调逻辑）
pub fn prepare_close_with(app: Option<&AppHandle>, reason: CloseReason) -> PrepareOutcome {
    let planned: Vec<ModuleLifecycle> = hooks().iter().copied().collect();
    let mut outcome = PrepareOutcome {
        proceed: true,
        blockers: Vec::new(),
    };
    for hook in planned {
        if let Some(prepare) = hook.prepare {
            let answer = std::panic::catch_unwind(AssertUnwindSafe(|| prepare(app, reason)));
            match answer {
                Ok(Ok(())) => {}
                Ok(Err(reason_text)) => {
                    outcome
                        .blockers
                        .push(format!("{}: {reason_text}", hook.owner));
                }
                Err(_) => outcome.blockers.push(format!(
                    "{}: 关闭前询问异常（已兜住，按拒绝处理）",
                    hook.owner
                )),
            }
        }
    }
    outcome.proceed = outcome.blockers.is_empty();
    outcome
}

/// dispose 阶段结果
#[derive(Debug, Default)]
pub struct DisposeOutcome {
    /// 清理失败描述（含超时与钩子异常），空 = 全部成功
    pub failures: Vec<String>,
    /// 是否因总超时中断
    pub timed_out: bool,
    /// 实际跑完的模块数
    pub ran: usize,
}

/// dispose 阶段：按登记顺序执行模块清理，受总超时约束
pub fn dispose(app: &AppHandle, reason: CloseReason) -> DisposeOutcome {
    dispose_with_timeout(Some(app), reason, DISPOSE_TIMEOUT)
}

/// dispose 阶段的实现（超时可注入，供测试构造超时场景）
pub fn dispose_with_timeout(
    app: Option<&AppHandle>,
    reason: CloseReason,
    timeout: Duration,
) -> DisposeOutcome {
    let planned: Vec<ModuleLifecycle> = hooks()
        .iter()
        .copied()
        .filter(|hook| hook.dispose.is_some())
        .collect();
    if planned.is_empty() {
        return DisposeOutcome::default();
    }
    // 钩子是 fn 指针（可 Send + Copy），放到独立线程里跑：同步钩子若卡住，主线程仍能按总超时收口
    let (tx, rx) = std::sync::mpsc::channel();
    let app = app.cloned();
    std::thread::spawn(move || {
        let mut failures: Vec<String> = Vec::new();
        let mut ran = 0usize;
        for hook in planned {
            let Some(dispose) = hook.dispose else {
                continue;
            };
            match std::panic::catch_unwind(AssertUnwindSafe(|| dispose(app.as_ref(), reason))) {
                Ok(list) => {
                    ran += 1;
                    failures.extend(list.into_iter().map(|msg| format!("{}: {msg}", hook.owner)));
                }
                Err(_) => {
                    failures.push(format!("{}: 清理钩子异常（已兜住，计入失败）", hook.owner))
                }
            }
        }
        let _ = tx.send((ran, failures));
    });
    match rx.recv_timeout(timeout) {
        Ok((ran, failures)) => DisposeOutcome {
            failures,
            timed_out: false,
            ran,
        },
        Err(_) => DisposeOutcome {
            failures: vec![format!(
                "关闭清理总超时（{} ms）：未完成的模块按清理失败处理",
                timeout.as_millis()
            )],
            timed_out: true,
            ran: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用例串行锁：钩子表是进程级静态，libtest 默认并行跑会互相污染（计数与登记互相干扰）
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 每个用例入口：先串行、再清表，保证断言只看到自己登记的钩子
    fn isolated() -> MutexGuard<'static, ()> {
        let guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_for_test();
        guard
    }

    /// 测试用 owner：登记后替换，避免污染其他用例的语义
    const OWNER_OK: &str = "__test_ok__";
    const OWNER_VETO: &str = "__test_veto__";
    const OWNER_SLOW: &str = "__test_slow__";

    fn ok_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        Vec::new()
    }

    fn veto_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Result<(), String> {
        Err("有未保存内容".into())
    }

    fn slow_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        std::thread::sleep(Duration::from_millis(300));
        Vec::new()
    }

    fn failing_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        vec!["断开连接失败".into()]
    }

    /// 强退标记：只由显式动作置位，读到 true 后由用户流程负责复位（用例内复位以免污染他人）
    #[test]
    fn force_flag_round_trip() {
        let _guard = isolated();
        clear_force_for_test();
        assert!(!force_requested(), "默认不处于强退流程");
        set_force_exit();
        assert!(force_requested());
        clear_force_for_test();
        assert!(!force_requested());
    }

    /// 关闭原因：稳定字符串与解析互为逆运算；未知值不猜默认
    #[test]
    fn close_reason_codes_round_trip() {
        for reason in [
            CloseReason::Tab,
            CloseReason::Exit,
            CloseReason::Restart,
            CloseReason::Update,
            CloseReason::SpaceSwitch,
        ] {
            assert_eq!(CloseReason::from_code(reason.code()), Some(reason));
        }
        assert_eq!(CloseReason::from_code("unknown"), None);
    }

    /// prepare：任一模块拒绝即不允许关闭，原因带上模块 owner；清理阶段不执行
    #[test]
    fn prepare_can_be_rejected() {
        let _guard = isolated();
        register(ModuleLifecycle {
            owner: OWNER_OK,
            prepare: None,
            dispose: None,
        });
        register(ModuleLifecycle {
            owner: OWNER_VETO,
            prepare: Some(veto_hook),
            dispose: None,
        });
        let outcome = prepare_close_with(None, CloseReason::Exit);
        assert!(!outcome.proceed, "有拒绝原因时不应继续关闭");
        assert_eq!(outcome.blockers.len(), 1);
        assert!(outcome.blockers[0].starts_with(OWNER_VETO));
    }

    /// dispose：按失败描述收集，成功钩子计入 ran；未登记 dispose 的模块不参与
    #[test]
    fn dispose_collects_failures_with_owner() {
        let _guard = isolated();
        register(ModuleLifecycle {
            owner: OWNER_OK,
            prepare: None,
            dispose: Some(ok_hook),
        });
        register(ModuleLifecycle {
            owner: "__test_fail__",
            prepare: None,
            dispose: Some(failing_hook),
        });
        let outcome = dispose_with_timeout(None, CloseReason::Exit, Duration::from_secs(2));
        assert!(!outcome.timed_out);
        assert_eq!(outcome.ran, 2, "两个 dispose 钩子都应跑完");
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("断开连接失败"));
        assert!(outcome.failures[0].contains("__test_fail__"));
    }

    /// dispose 总超时：慢钩子不阻塞退出，超时计入失败并可诊断
    #[test]
    fn dispose_times_out_without_blocking_exit() {
        let _guard = isolated();
        register(ModuleLifecycle {
            owner: OWNER_SLOW,
            prepare: None,
            dispose: Some(slow_hook),
        });
        let outcome = dispose_with_timeout(None, CloseReason::Exit, Duration::from_millis(50));
        assert!(outcome.timed_out, "超过总超时应标记超时");
        assert_eq!(outcome.ran, 0);
        assert!(outcome.failures[0].contains("总超时"));
    }

    /// 同 owner 重复登记按覆盖：不 panic、不重复执行
    #[test]
    fn register_replaces_same_owner() {
        let _guard = isolated();
        register(ModuleLifecycle {
            owner: "__test_dup__",
            prepare: None,
            dispose: Some(ok_hook),
        });
        register(ModuleLifecycle {
            owner: "__test_dup__",
            prepare: None,
            dispose: Some(failing_hook),
        });
        let owners = registered_owners();
        assert_eq!(
            owners.iter().filter(|o| **o == "__test_dup__").count(),
            1,
            "同 owner 只保留一条登记"
        );
    }
}
