//! 框架 · 关闭协商命令面（页签关闭与退出共用，AR06 §9.2）
//!
//! 唯一入口的前端面：页面关闭页签、请求退出都走这两个命令，而不是各自 `app.exit`。
//! 分两段与 `lifecycle` 对齐，裁决只发生在 `lifecycle::compose_decision`：
//! - `app_request_close`：**只裁决**。问后端模块（按 `reason` 与 `toolId` 过滤），把结果与
//!   页面 owner 上报的 blockers 合成一次结论；被拒绝时**不清理也不退出**，原因交回前端展示，
//!   由用户决定「重试 / 放弃并关闭 / 取消」。
//! - `app_commit_close`：**只在裁决通过后调用**。页签关闭执行该工具作用域的清理；
//!   退出族只发起退出，进程级清理仍由 `RunEvent::Exit` 统一执行（唯一清理点，不重复清）。
//! - `app_force_exit`：用户显式强退。置强制标记后退出，prepare 不再拦截；
//!   清理阶段的总超时照旧生效，因此**可能有模块来不及清理**，这一点必须对用户如实说明。
//!
//! 不变式：
//! - 强制标记只允许由用户显式动作设置（本模块的命令），业务代码不得自己置位；
//! - 拒绝退出时主窗口会被唤到前台，否则托盘退出场景下用户看不到任何提示；
//! - 未知 reason 字符串直接报错，不猜默认值；页签关闭必须带 `toolId`，不回落成「问全部模块」。

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::framework::lifecycle::{self, CloseDecision, CloseReason};

/// 前端订阅的拒绝事件名（退出被业务拦下时携带原因）
pub const EXIT_VETO_EVENT: &str = "app://exit-vetoed";

/// 关闭请求（页面发起）
///
/// 字段扁平而不是嵌套对象：前端传 `{ reason, toolId, blockers }` 即可，
/// 参数名与契约字段一一对应，避免「包了一层却对不上」这类只在运行期暴露的错误。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseRequest {
    /// 关闭原因代码（`tab` / `exit` / `restart` / `update` / `space-switch`）；缺省按 `exit`
    #[serde(default)]
    pub reason: Option<String>,
    /// 页签关闭时的工具 id（`reason=tab` 必填；其他原因忽略）
    #[serde(default)]
    pub tool_id: Option<String>,
    /// 页面 owner 的拒绝原因（前端 prepare 结果）；与后端 blockers 合成唯一裁决
    #[serde(default)]
    pub blockers: Vec<String>,
    /// 用户已确认放弃（页签的「放弃并关闭」）：跳过拦截，但清理照做、失败照报
    #[serde(default)]
    pub force: bool,
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

/// 解析一次关闭请求：返回（原因，工具 id）。
///
/// 页签关闭必须带工具 id：缺失时宁可报错，也不能猜成「问一遍全部模块」——
/// 那正是页签关闭误清全局资源的入口。
pub fn parse_request(request: &CloseRequest) -> Result<(CloseReason, Option<String>), String> {
    let reason = parse_reason(request.reason.as_deref())?;
    let tool = request
        .tool_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if reason == CloseReason::Tab && tool.is_none() {
        return Err("页签关闭必须带 toolId（不猜要关哪个工具的模块）".into());
    }
    Ok((reason, tool))
}

/// 把被拒绝的退出告知前端并把主窗口唤到前台。
///
/// 为什么必须展示窗口：托盘菜单退出时窗口通常处于隐藏状态，只写日志等于「点了没反应」。
/// 公开给 `lib.rs` 的 `ExitRequested` 复用：OS/托盘发起的退出与被拒绝时的提示必须一致。
pub(crate) fn surface_veto(app: &AppHandle, decision: &CloseDecision) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Err(error) = app.emit(EXIT_VETO_EVENT, decision.clone()) {
        log::warn!(
            "退出被拒绝但事件上报失败: {error_type}",
            error_type = std::any::type_name_of_val(&error)
        );
    }
}

/// 请求关闭（prepare 阶段）：只裁决，不清理也不退出
///
/// 返回 `proceed=false` 表示被业务或页面 owner 拒绝：进程/页签仍在，前端应展示 `blockers`。
///
/// 参数**扁平**（不是 `request: {...}`）：Tauri 按参数名取实参（Rust `snake_case` ↔ 前端
/// `camelCase`），前端传 `{ reason, toolId, blockers, force }` 即可，与 `contracts.ts` 的声明一一对应。
/// 曾因写成单参结构体导致运行期 `missing required key request`：编译期与契约表都看不出来，
/// 只有真机走一次关闭协商才暴露。
#[tauri::command]
pub fn app_request_close(
    app: AppHandle,
    reason: Option<String>,
    tool_id: Option<String>,
    blockers: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<CloseDecision, String> {
    let request = CloseRequest {
        reason,
        tool_id,
        blockers: blockers.unwrap_or_default(),
        force: force.unwrap_or(false),
    };
    let (reason, tool) = parse_request(&request)?;
    let outcome = match tool.as_deref() {
        Some(tool) => lifecycle::prepare_close_tool(&app, tool, reason),
        None => lifecycle::prepare_close(&app, reason),
    };
    let forced = request.force || lifecycle::force_requested();
    let decision = lifecycle::compose_decision(forced, request.blockers, outcome);
    if !decision.proceed && reason != CloseReason::Tab {
        // 页签关闭由页面自己弹确认框；退出族才需要唤窗口 + 事件，否则托盘退出等于「点了没反应」
        surface_veto(&app, &decision);
    }
    Ok(decision)
}

/// 提交关闭（dispose 阶段）：**只在 `app_request_close` 通过后调用**
///
/// - 页签关闭：执行该工具作用域的清理（没有声明 `tab` 作用域的模块就不会被碰）；
/// - 退出族：只发起退出，进程级清理由 `RunEvent::Exit` 统一执行。
///
/// 参数扁平的理由与 `app_request_close` 相同（Tauri 按参数名取实参）。
#[tauri::command]
pub fn app_commit_close(
    app: AppHandle,
    reason: Option<String>,
    tool_id: Option<String>,
    blockers: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<CloseDecision, String> {
    let request = CloseRequest {
        reason,
        tool_id,
        blockers: blockers.unwrap_or_default(),
        force: force.unwrap_or(false),
    };
    let (reason, tool) = parse_request(&request)?;
    let forced = request.force || lifecycle::force_requested();
    if reason == CloseReason::Tab {
        let tool = tool.unwrap_or_default();
        let outcome = lifecycle::dispose_tool(&app, &tool, reason);
        return Ok(CloseDecision {
            proceed: true,
            forced,
            blockers: Vec::new(),
            failures: outcome.failures,
        });
    }
    app.exit(0);
    Ok(CloseDecision {
        proceed: true,
        forced,
        blockers: Vec::new(),
        failures: Vec::new(),
    })
}

/// 退出裁决：托盘菜单退出与系统退出（`RunEvent::ExitRequested`）共用这一处。
///
/// 只裁决、不退出：调用方按 `proceed` 决定是 `app.exit(0)` 还是阻止退出。
/// 被拒绝时唤窗口并抛事件，由页面给出重试 / 强制退出 / 取消三个出路。
///
/// 为什么托盘退出也要走这里：原来直接 `app.exit(0)` 会绕过 prepare，
/// 未保存内容与进行中的任务都拦不住，「唯一关闭入口」就成了空话。
pub fn decide_exit(app: &AppHandle) -> CloseDecision {
    let outcome = lifecycle::prepare_close(app, CloseReason::Exit);
    let decision = lifecycle::compose_decision(lifecycle::force_requested(), Vec::new(), outcome);
    if !decision.proceed {
        log::info!("退出被阻止 blockers={}", decision.blockers.len());
        surface_veto(app, &decision);
    }
    decision
}

/// 托盘菜单「退出」：裁决通过才退出
pub fn quit_from_tray(app: &AppHandle) {
    if decide_exit(app).proceed {
        app.exit(0);
    }
}

/// 用户显式强制退出：跳过业务拦截，清理阶段仍受总超时约束
#[tauri::command]
pub fn app_force_exit(app: AppHandle) -> CloseDecision {
    lifecycle::set_force_exit();
    let decision = CloseDecision {
        proceed: true,
        forced: true,
        blockers: Vec::new(),
        failures: Vec::new(),
    };
    app.exit(0);
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::lifecycle::PrepareOutcome;

    /// 关闭原因：缺省=exit，已知值解析成功，未知值报错而不是照常退出
    #[test]
    fn parse_reason_is_strict() {
        assert_eq!(parse_reason(None), Ok(CloseReason::Exit));
        assert_eq!(parse_reason(Some("")), Ok(CloseReason::Exit));
        assert_eq!(parse_reason(Some("update")), Ok(CloseReason::Update));
        assert!(parse_reason(Some("quit-now")).is_err());
    }

    /// 页签关闭缺 toolId 直接报错：不猜「问全部模块」（那是误清全局资源的入口）
    #[test]
    fn parse_request_requires_tool_for_tab() {
        let missing = CloseRequest {
            reason: Some("tab".into()),
            tool_id: None,
            blockers: Vec::new(),
            force: false,
        };
        assert!(parse_request(&missing).is_err());
        let blank = CloseRequest {
            reason: Some("tab".into()),
            tool_id: Some("   ".into()),
            ..missing
        };
        assert!(parse_request(&blank).is_err(), "空白 toolId 不算给了工具");
        let given = CloseRequest {
            reason: Some("tab".into()),
            tool_id: Some("ssh".into()),
            blockers: vec!["ui.editor: 有未保存内容".into()],
            force: false,
        };
        assert_eq!(
            parse_request(&given),
            Ok((CloseReason::Tab, Some("ssh".to_string())))
        );
    }

    /// 退出族不带 toolId 也合法（清理面由作用域决定）
    #[test]
    fn parse_request_allows_exit_without_tool() {
        let request = CloseRequest {
            reason: None,
            tool_id: None,
            blockers: Vec::new(),
            force: false,
        };
        assert_eq!(parse_request(&request), Ok((CloseReason::Exit, None)));
    }

    /// 裁决合成复用 lifecycle 的纯函数：强制退出即使有拒绝原因也放行，但原因保留给诊断
    #[test]
    fn forced_decision_keeps_reasons() {
        let outcome = PrepareOutcome {
            proceed: false,
            blockers: vec!["frp: 1 个档案正在运行".into()],
        };
        let decision = lifecycle::compose_decision(true, Vec::new(), outcome);
        assert!(decision.proceed);
        assert!(decision.forced);
        assert_eq!(decision.blockers.len(), 1);
    }
}
