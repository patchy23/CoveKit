//! 框架 · 退出协商命令（可靠性 T10-3）
//!
//! 唯一退出入口的前端面：界面/托盘/更新安装都走这里，而不是各自 `app.exit`。
//! 分两段与 `lifecycle` 对齐：
//! - `app_request_exit`：先问（prepare），被拒绝时**不退出**并把拒绝原因交回前端展示，
//!   由用户决定「重试」还是「强制退出」；允许时才真正退出，清理仍走 `RunEvent::Exit`。
//! - `app_force_exit`：用户显式强退。置强制标记后退出，prepare 不再拦截；
//!   清理阶段的总超时照旧生效，因此**可能有模块来不及清理**，这一点必须对用户如实说明。
//!
//! 不变式：
//! - 强制标记只允许由用户显式动作设置（本模块的命令），业务代码不得自己置位；
//! - 拒绝退出时主窗口会被唤到前台，否则托盘退出场景下用户看不到任何提示；
//! - 未知 reason 字符串直接报错，不猜默认值。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::framework::lifecycle::{self, CloseReason};

/// 前端订阅的拒绝事件名（退出被业务拦下时携带原因）
pub const EXIT_VETO_EVENT: &str = "app://exit-vetoed";

/// 退出请求结果（前端据此决定展示提示还是关闭界面）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitDecision {
    /// 是否已经进入退出流程（`false` = 被业务拒绝，进程仍在运行）
    pub started: bool,
    /// 是否由用户显式强退触发（界面需要说明「清理可能不完整」）
    pub forced: bool,
    /// 拒绝原因（形如 `owner: 原因`；`started=true` 时为空）
    pub blockers: Vec<String>,
}

/// 前端传入的退出参数
///
/// 用扁平 `reason` 而不是嵌套对象：前端传 `{ reason: "exit" }` 即可，
/// 参数名与契约字段一一对应，避免「包了一层却对不上」这类只在运行期暴露的错误。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitRequest {
    /// 关闭原因代码（`exit` / `restart` / `update` / `space-switch`）；缺省按 `exit`
    #[serde(default)]
    pub reason: Option<String>,
}

/// 决定一次退出请求的结果（纯函数，便于用例覆盖各分支）
///
/// 强制退出时不再拦截，但仍把拒绝原因原样带出——诊断需要知道「谁本来不同意」。
pub fn decide(forced: bool, outcome: lifecycle::PrepareOutcome) -> ExitDecision {
    if forced {
        return ExitDecision {
            started: true,
            forced: true,
            blockers: outcome.blockers,
        };
    }
    ExitDecision {
        started: outcome.proceed,
        forced: false,
        blockers: if outcome.proceed {
            Vec::new()
        } else {
            outcome.blockers
        },
    }
}

/// 解析关闭原因：未知值报错（不回落默认，避免「传错参数却照常退出」）
pub fn parse_reason(reason: Option<&str>) -> Result<CloseReason, String> {
    match reason {
        None | Some("") => Ok(CloseReason::Exit),
        Some(code) => CloseReason::from_code(code).ok_or_else(|| {
            format!("未知的关闭原因 `{code}`（支持 exit/restart/update/tab/space-switch）")
        }),
    }
}

/// 把被拒绝的退出告知前端并把主窗口唤到前台。
///
/// 为什么必须展示窗口：托盘菜单退出时窗口通常处于隐藏状态，只写日志等于「点了没反应」。
/// 公开给 `lib.rs` 的 `ExitRequested` 复用：OS/托盘发起的退出与被拒绝时的提示必须一致。
pub(crate) fn surface_veto(app: &AppHandle, decision: &ExitDecision) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Err(error) = app.emit(EXIT_VETO_EVENT, decision.clone()) {
        eprintln!("[lifecycle] 退出被拒绝但事件上报失败: {error}");
    }
}

/// 请求退出（界面/托盘/更新安装共用入口）
///
/// 返回 `started=false` 表示被业务拒绝：进程仍在运行，前端应展示 `blockers`。
#[tauri::command]
pub fn app_request_exit(
    app: AppHandle,
    request: Option<ExitRequest>,
) -> Result<ExitDecision, String> {
    let reason = parse_reason(request.as_ref().and_then(|r| r.reason.as_deref()))?;
    let outcome = lifecycle::prepare_close(&app, reason);
    let decision = decide(lifecycle::force_requested(), outcome);
    if decision.started {
        app.exit(0);
    } else {
        surface_veto(&app, &decision);
    }
    Ok(decision)
}

/// 用户显式强制退出：跳过业务拦截，清理阶段仍受总超时约束
#[tauri::command]
pub fn app_force_exit(app: AppHandle) -> ExitDecision {
    lifecycle::set_force_exit();
    let decision = ExitDecision {
        started: true,
        forced: true,
        blockers: Vec::new(),
    };
    app.exit(0);
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 允许关闭：进入退出流程且不携带拒绝原因
    #[test]
    fn decide_starts_when_allowed() {
        let outcome = lifecycle::PrepareOutcome {
            proceed: true,
            blockers: Vec::new(),
        };
        let decision = decide(false, outcome);
        assert!(decision.started);
        assert!(!decision.forced);
        assert!(decision.blockers.is_empty());
    }

    /// 被拒绝：不进入退出流程，原因原样带出给用户看
    #[test]
    fn decide_keeps_exit_when_blocked() {
        let outcome = lifecycle::PrepareOutcome {
            proceed: false,
            blockers: vec!["ssh: 2 个会话仍在连接".into()],
        };
        let decision = decide(false, outcome);
        assert!(!decision.started, "被拒绝时不能让进程退出");
        assert_eq!(decision.blockers, vec!["ssh: 2 个会话仍在连接".to_string()]);
    }

    /// 强制退出：即使有拒绝原因也退出，但拒绝原因仍要出现在结果里（诊断口径）
    #[test]
    fn decide_forces_through_vetoes() {
        let outcome = lifecycle::PrepareOutcome {
            proceed: false,
            blockers: vec!["frp: 1 个档案正在运行".into()],
        };
        let decision = decide(true, outcome);
        assert!(decision.started);
        assert!(decision.forced);
        assert_eq!(decision.blockers.len(), 1);
    }

    /// 关闭原因：缺省=exit，已知值解析成功，未知值报错而不是照常退出
    #[test]
    fn parse_reason_is_strict() {
        assert_eq!(parse_reason(None), Ok(CloseReason::Exit));
        assert_eq!(parse_reason(Some("")), Ok(CloseReason::Exit));
        assert_eq!(parse_reason(Some("update")), Ok(CloseReason::Update));
        assert!(parse_reason(Some("quit-now")).is_err());
    }
}
