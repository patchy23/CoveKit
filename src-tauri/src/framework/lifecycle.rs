//! 统一关闭协调（AR06 §9.2）
//!
//! 关闭只有这一个入口，按 `reason` 区分场景，并分成两个阶段：
//! - **prepare**：只询问、不清理，允许业务拒绝（未保存、任务进行中）——拒绝则应用不关闭；
//! - **dispose**：仅在允许关闭后执行，各模块清理自己的协议与进程，应用只汇总结果与总超时。
//!
//! 裁决是唯一的：[`compose_decision`] 把后端 blockers 与页面 owner 上报的 blockers 合成一次结论，
//! 前端不自行决定关闭，也不存在第二套协调器。
//!
//! 不变式：
//! - 业务协议清理由各模块提供（`ModuleLifecycle`），应用不替模块实现清理；
//! - 询问面与清理面用同一个 [`selected`] 过滤：页签关闭只碰声明了 `tab` 作用域且归属该工具的模块，
//!   绝不会误清全局资源；
//! - dispose 有总超时：超时按失败计入并继续退出，不无限等待（前台按钮禁用不是锁）；
//! - 钩子 panic 会被兜住并计入失败，绝不让单个模块的异常阻断整个退出流程；
//! - 根迁移/导入提交/空间激活/更新安装的互斥不在本模块，见 `context::maintenance_guard()`。
use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

/// 关闭原因：唯一入口据此区分场景（页签/退出/重启/更新/空间切换）
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

/// 关闭作用域：声明该模块在哪些关闭原因下需要被询问与清理。
///
/// 默认取「少清」：未声明 `tab` 的模块不随页签关闭清理。页签关闭误清全局资源
/// （连接、子进程）比晚清一次更难恢复，因此宁可要求模块显式开启。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseScopes {
    /// 单个工具页签关闭时是否参与（还需 `ModuleLifecycle::tool` 与目标工具一致）
    pub tab: bool,
    /// 退出/重启/更新/空间切换时是否参与
    pub exit: bool,
}

impl CloseScopes {
    /// 只随应用退出清理（页签关闭不碰它）
    pub const EXIT_ONLY: Self = Self {
        tab: false,
        exit: true,
    };
    /// 页签关闭与应用退出都参与
    pub const TAB_AND_EXIT: Self = Self {
        tab: true,
        exit: true,
    };
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
    /// 归属的工具 id：`Some` 时该模块的后端资源专属于这个工具，页签关闭按它筛选
    pub tool: Option<&'static str>,
    /// 参与哪些关闭原因（默认只随退出）
    pub scopes: CloseScopes,
    /// 关闭前询问（可选）：返回 Err 拒绝关闭
    pub prepare: Option<PrepareHook>,
    /// 允许关闭后的清理（可选）：返回失败描述
    pub dispose: Option<DisposeHook>,
}

impl ModuleLifecycle {
    /// 只随应用退出清理的模块（无页签级资源时的默认写法）
    pub fn exit_only(owner: &'static str) -> Self {
        Self {
            owner,
            tool: None,
            scopes: CloseScopes::EXIT_ONLY,
            prepare: None,
            dispose: None,
        }
    }

    /// 归属某个工具的模块（页签关闭按工具筛选；作用域仍由 `scopes` 决定）
    pub fn for_tool(owner: &'static str, tool: &'static str) -> Self {
        Self {
            owner,
            tool: Some(tool),
            ..Self::exit_only(owner)
        }
    }

    /// 追加关闭前询问钩子
    ///
    /// 当前只由测试装配：后端还没有需要拒绝关闭的模块（拒绝条件都在页面内 owner，
    /// 由 `app_request_close` 的 `blockers` 带上来）。真有后端拒绝场景时去掉 `cfg`。
    #[cfg(test)]
    pub fn with_prepare(mut self, hook: PrepareHook) -> Self {
        self.prepare = Some(hook);
        self
    }

    /// 声明「该工具页签关闭时也要参与」（必须同时指定 `tool`，否则页签关闭选不到它）
    ///
    /// 用于资源绑在工具页签上的模块：页签一关，界面就再也够不到这些资源
    /// （常驻子进程、活动连接），留着只会变成看不见的泄漏。
    pub fn with_tab_scope(mut self) -> Self {
        self.scopes = CloseScopes::TAB_AND_EXIT;
        self
    }

    /// 追加清理钩子
    pub fn with_dispose(mut self, hook: DisposeHook) -> Self {
        self.dispose = Some(hook);
        self
    }
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

/// 本次关闭是否要问/清这个模块。
///
/// - 页签关闭：必须是声明了 `tab` 作用域、且 `tool` 与目标工具一致的模块（目标未给则一个都不问）；
/// - 其余原因：声明了 `exit` 作用域的模块；页签专用模块不参与退出清理。
fn selected(hook: &ModuleLifecycle, reason: CloseReason, tool: Option<&str>) -> bool {
    match reason {
        CloseReason::Tab => match (hook.scopes.tab, hook.tool, tool) {
            (true, Some(owner_tool), Some(target)) => owner_tool == target,
            _ => false,
        },
        _ => hook.scopes.exit,
    }
}

/// prepare 阶段结果：`proceed=false` 时 `blockers` 为拒绝原因（按登记顺序）
#[derive(Debug, Default)]
pub struct PrepareOutcome {
    /// 是否允许继续关闭（存在拒绝原因时为 false）
    pub proceed: bool,
    /// 拒绝原因（形如 `owner: 原因`，按登记顺序）
    pub blockers: Vec<String>,
}

/// 关闭裁决：后端与页面两侧 blockers 合成后的唯一结论（前端据此决定展示与是否清理）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseDecision {
    /// 是否允许关闭（`false` = 有模块或页面 owner 拒绝，两侧都未清理）
    pub proceed: bool,
    /// 是否由用户显式强制（跳过拦截，但原因仍保留）
    pub forced: bool,
    /// 拒绝原因（形如 `owner: 原因`；`proceed=true` 时为空）
    pub blockers: Vec<String>,
    /// 清理失败（提交阶段；不阻断关闭，但必须让用户看到）
    pub failures: Vec<String>,
}

/// 合成一次关闭裁决（纯函数，便于用例覆盖各分支）。
///
/// 前端上报的 blockers 与后端 blockers 平权：任一侧拒绝即 `proceed=false`，
/// 且此时两侧都不执行清理。强制关闭（用户显式确认放弃）跳过拦截，但原因照原样带出——诊断需要知道「谁本来不同意」。
pub fn compose_decision(
    forced: bool,
    frontend_blockers: Vec<String>,
    outcome: PrepareOutcome,
) -> CloseDecision {
    let mut blockers = frontend_blockers;
    blockers.extend(outcome.blockers);
    if forced {
        return CloseDecision {
            proceed: true,
            forced: true,
            blockers,
            failures: Vec::new(),
        };
    }
    CloseDecision {
        proceed: outcome.proceed && blockers.is_empty(),
        forced: false,
        blockers: if outcome.proceed && blockers.is_empty() {
            Vec::new()
        } else {
            blockers
        },
        failures: Vec::new(),
    }
}

/// prepare 阶段：询问全部随退出清理的模块，任一拒绝则不允许关闭（不做任何清理）
pub fn prepare_close(app: &AppHandle, reason: CloseReason) -> PrepareOutcome {
    prepare_close_with(Some(app), reason, None)
}

/// 页签关闭的 prepare 阶段：只问声明了 `tab` 作用域且归属该工具的模块
pub fn prepare_close_tool(app: &AppHandle, tool: &str, reason: CloseReason) -> PrepareOutcome {
    prepare_close_with(Some(app), reason, Some(tool))
}

/// prepare 阶段的实现（`app` 可为 None，供单元测试驱动协调逻辑）
pub fn prepare_close_with(
    app: Option<&AppHandle>,
    reason: CloseReason,
    tool: Option<&str>,
) -> PrepareOutcome {
    let planned: Vec<ModuleLifecycle> = hooks()
        .iter()
        .copied()
        .filter(|hook| selected(hook, reason, tool))
        .collect();
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
    /// 实际跑完清理的模块（诊断与「页签关闭没碰全局资源」的断言依据）
    pub owners: Vec<&'static str>,
}

/// dispose 阶段：按登记顺序执行随退出清理的模块，受总超时约束
pub fn dispose(app: &AppHandle, reason: CloseReason) -> DisposeOutcome {
    dispose_with_timeout(Some(app), reason, None, DISPOSE_TIMEOUT)
}

/// 页签关闭的清理阶段：只清声明了 `tab` 作用域且归属该工具的模块
pub fn dispose_tool(app: &AppHandle, tool: &str, reason: CloseReason) -> DisposeOutcome {
    dispose_with_timeout(Some(app), reason, Some(tool), DISPOSE_TIMEOUT)
}

/// dispose 阶段的实现（超时可注入，供测试构造超时场景）
pub fn dispose_with_timeout(
    app: Option<&AppHandle>,
    reason: CloseReason,
    tool: Option<&str>,
    timeout: Duration,
) -> DisposeOutcome {
    let planned: Vec<ModuleLifecycle> = hooks()
        .iter()
        .copied()
        .filter(|hook| selected(hook, reason, tool) && hook.dispose.is_some())
        .collect();
    if planned.is_empty() {
        return DisposeOutcome::default();
    }
    // 钩子是 fn 指针（可 Send + Copy），放到独立线程里跑：同步钩子若卡住，主线程仍能按总超时收口
    let (tx, rx) = std::sync::mpsc::channel();
    let app = app.cloned();
    std::thread::spawn(move || {
        let mut failures: Vec<String> = Vec::new();
        let mut owners: Vec<&'static str> = Vec::new();
        for hook in planned {
            let Some(dispose) = hook.dispose else {
                continue;
            };
            match std::panic::catch_unwind(AssertUnwindSafe(|| dispose(app.as_ref(), reason))) {
                Ok(list) => {
                    owners.push(hook.owner);
                    if !list.is_empty() {
                        log::error!("模块资源清理失败 owner={} count={}", hook.owner, list.len());
                    } else {
                        log::info!("模块资源清理完成 owner={}", hook.owner);
                    }
                    failures.extend(list.into_iter().map(|msg| format!("{}: {msg}", hook.owner)));
                }
                Err(_) => {
                    log::error!("模块资源清理异常 owner={}", hook.owner);
                    failures.push(format!("{}: 清理钩子异常（已兜住，计入失败）", hook.owner))
                }
            }
        }
        let _ = tx.send((owners, failures));
    });
    match rx.recv_timeout(timeout) {
        Ok((owners, failures)) => DisposeOutcome {
            failures,
            timed_out: false,
            owners,
        },
        Err(_) => {
            log::error!("资源清理未在预算内完成 timeout_ms={}", timeout.as_millis());
            DisposeOutcome {
                failures: vec![format!(
                    "关闭清理总超时（{} ms）：未完成的模块按清理失败处理",
                    timeout.as_millis()
                )],
                timed_out: true,
                owners: Vec::new(),
            }
        }
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
