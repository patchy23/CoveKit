//! 导入导出会话上下文（§5.3 的 `inspectId` / `planId` 与取消标志）
//!
//! 为什么要有这一层：解密后的清单不能交给前端保管（前端持有等于清单可被篡改，
//! 而且密码与明文都不该离开 Rust），所以命令之间用**进程内上下文**传递：
//! - `inspectId`：绑定文件路径、内容摘要与解密后的清单，超时即失效；**不缓存密码**，
//!   提交时必须再次输入密码（§9 末段）。
//! - `planId`：绑定已解析的导入计划（含用户选择），提交只认这份结构。
//!
//! 提交前会用这里存的路径与摘要**重新读一遍文件**：文件被换过就是另一份包，
//! 不能拿旧预览的结论去写数据（这是「密码再次提供」之外真正起作用的那道校验）。
//!
//! 取消标志是进程内单例：导出与导入都受维护互斥保护，同一时刻只有一次传输在跑。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::import::ImportPlan;
use super::types::PackageManifest;

/// 会话有效期：超时后必须重新选择文件并输入密码（有界上下文，不无限留存）
pub(crate) const SESSION_TTL: Duration = Duration::from_secs(30 * 60);

/// 一次 `inspect` 的结果
pub(crate) struct InspectContext {
    /// 数据包路径（提交前重新读它做复核）
    pub file_path: PathBuf,
    /// 文件内容摘要（复核用：文件被换过就拒绝复用这份上下文）
    pub file_digest: String,
    /// 解密并校验过的清单
    pub manifest: PackageManifest,
    /// 建立时间（判超时用）
    pub created_at: Instant,
}

/// 一次 `plan` 的结果
pub(crate) struct PlanContext {
    /// 已解析的导入计划
    pub plan: ImportPlan,
    /// 来源包路径（提交前复核）
    pub file_path: PathBuf,
    /// 来源包的文件摘要（提交前复核）
    pub file_digest: String,
    /// 建立时间（判超时用）
    pub created_at: Instant,
}

/// 会话表（进程内，按 id 索引）
#[derive(Default)]
struct Sessions {
    /// inspect 上下文
    inspects: BTreeMap<String, InspectContext>,
    /// plan 上下文
    plans: BTreeMap<String, PlanContext>,
}

/// 会话表单例
fn sessions() -> &'static Mutex<Sessions> {
    static SESSIONS: OnceLock<Mutex<Sessions>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(Sessions::default()))
}

/// 清掉过期条目（每次写入时顺手做，避免表无限增长）
fn prune(sessions: &mut Sessions) {
    let now = Instant::now();
    sessions
        .inspects
        .retain(|_, context| now.duration_since(context.created_at) < SESSION_TTL);
    sessions
        .plans
        .retain(|_, context| now.duration_since(context.created_at) < SESSION_TTL);
}

/// 登记一次 inspect，返回 `inspectId`
pub(crate) fn put_inspect(
    file_path: PathBuf,
    file_digest: String,
    manifest: PackageManifest,
) -> Result<String, String> {
    let id = format!("insp-{}", uuid::Uuid::new_v4());
    let mut guard = sessions().lock().map_err(|e| e.to_string())?;
    prune(&mut guard);
    guard.inspects.insert(
        id.clone(),
        InspectContext {
            file_path,
            file_digest,
            manifest,
            created_at: Instant::now(),
        },
    );
    Ok(id)
}

/// 取一次 inspect（不删除：用户可以反复调整选择再规划；超时/未知即报错）
pub(crate) fn inspect(id: &str) -> Result<(PathBuf, String, PackageManifest), String> {
    let guard = sessions().lock().map_err(|e| e.to_string())?;
    let context = guard
        .inspects
        .get(id)
        .ok_or_else(|| "预览已过期，请重新选择数据包并输入密码".to_string())?;
    if Instant::now().duration_since(context.created_at) >= SESSION_TTL {
        return Err("预览已过期，请重新选择数据包并输入密码".into());
    }
    Ok((
        context.file_path.clone(),
        context.file_digest.clone(),
        context.manifest.clone(),
    ))
}

/// 登记一次 plan，返回 `planId`
pub(crate) fn put_plan(
    plan: ImportPlan,
    file_path: PathBuf,
    file_digest: String,
) -> Result<String, String> {
    let id = plan.plan_id.clone();
    let mut guard = sessions().lock().map_err(|e| e.to_string())?;
    prune(&mut guard);
    guard.plans.insert(
        id.clone(),
        PlanContext {
            plan,
            file_path,
            file_digest,
            created_at: Instant::now(),
        },
    );
    Ok(id)
}

/// 取一次 plan（不删除：提交失败后用户可以改选择重试）
pub(crate) fn plan(id: &str) -> Result<(ImportPlan, PathBuf, String), String> {
    let guard = sessions().lock().map_err(|e| e.to_string())?;
    let context = guard
        .plans
        .get(id)
        .ok_or_else(|| "导入计划已过期，请重新预览后再提交".to_string())?;
    if Instant::now().duration_since(context.created_at) >= SESSION_TTL {
        return Err("导入计划已过期，请重新预览后再提交".into());
    }
    Ok((
        context.plan.clone(),
        context.file_path.clone(),
        context.file_digest.clone(),
    ))
}

/// 丢弃一次 plan（提交成功后清理，避免同一份计划被重复提交）
pub(crate) fn drop_plan(id: &str) {
    if let Ok(mut guard) = sessions().lock() {
        guard.plans.remove(id);
    }
}

/// 取消令牌：传输过程轮询它，取消时删掉半成品（导出临时文件 / 导入暂存目录）
#[derive(Clone)]
pub(crate) struct CancelToken {
    /// 取消标志（由 `data_transfer_cancel` 置位）
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// 是否已请求取消
    pub(crate) fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// 取消检查：已取消即返回错误，调用方据此清理半成品并结束
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.is_cancelled() {
            return Err("已取消".into());
        }
        Ok(())
    }
}

/// 当前传输的取消标志（进程内单例）
fn current() -> &'static Mutex<Option<CancelToken>> {
    static CURRENT: OnceLock<Mutex<Option<CancelToken>>> = OnceLock::new();
    CURRENT.get_or_init(|| Mutex::new(None))
}

/// 开始一次传输：登记新令牌（上一次的令牌随之作废）
pub(crate) fn begin_transfer() -> CancelToken {
    let token = CancelToken {
        flag: Arc::new(AtomicBool::new(false)),
    };
    if let Ok(mut guard) = current().lock() {
        *guard = Some(token.clone());
    }
    token
}

/// 请求取消当前传输（返回是否确有在跑的传输）
pub(crate) fn cancel_current() -> bool {
    match current().lock() {
        Ok(guard) => match guard.as_ref() {
            Some(token) => {
                token.flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        },
        Err(_) => false,
    }
}

/// 结束一次传输（清掉令牌，避免取消命令作用到已结束的传输上）
pub(crate) fn end_transfer() {
    if let Ok(mut guard) = current().lock() {
        *guard = None;
    }
}
