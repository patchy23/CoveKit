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
//! 每项传输按请求标识取消，不限制合法并发；旧调用缺省标识时兼容最近任务入口。

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

struct TransferRegistration {
    token: CancelToken,
    claimed: bool,
}

#[derive(Default)]
struct Transfers {
    jobs: BTreeMap<String, TransferRegistration>,
    latest: Option<String>,
}

fn transfers() -> &'static Mutex<Transfers> {
    static TRANSFERS: OnceLock<Mutex<Transfers>> = OnceLock::new();
    TRANSFERS.get_or_init(|| Mutex::new(Transfers::default()))
}

/// 先登记再传正文，取消早于执行命令抵达时不会漏掉。
pub(crate) fn prepare_transfer() -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    transfers().lock().map_err(|e| e.to_string())?.jobs.insert(
        id.clone(),
        TransferRegistration {
            token: CancelToken {
                flag: Arc::new(AtomicBool::new(false)),
            },
            claimed: false,
        },
    );
    Ok(id)
}

/// 传输调用的所有权守卫：正常返回、异常和 future 被丢弃时统一清理自己的登记。
pub(crate) struct TransferGuard {
    id: String,
    token: CancelToken,
}

impl TransferGuard {
    /// 工作线程只持有协作取消令牌，登记的释放责任留在调用方。
    pub(crate) fn token(&self) -> CancelToken {
        self.token.clone()
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        // spawn_blocking 无法被 drop 强行中止；通知仍在执行的工作在检查点退出。
        self.token.flag.store(true, Ordering::SeqCst);
        match transfers().lock() {
            Ok(mut guard) => {
                guard.jobs.remove(&self.id);
                if guard.latest.as_deref() == Some(self.id.as_str()) {
                    guard.latest = None;
                }
            }
            Err(error) => log::error!("释放数据传输登记失败: {error}"),
        }
    }
}

/// 领取一次登记；同一标识不能重入，独立请求仍可并发。
pub(crate) fn begin_transfer(request_id: Option<&str>) -> Result<TransferGuard, String> {
    let id = match request_id {
        Some(id) => id.to_string(),
        None => prepare_transfer()?,
    };
    let mut guard = transfers().lock().map_err(|e| e.to_string())?;
    let request = guard.jobs.get_mut(&id).ok_or("已取消或传输登记不存在")?;
    if request.claimed {
        return Err("传输请求已经开始".into());
    }
    request.claimed = true;
    let token = request.token.clone();
    guard.latest = Some(id.clone());
    Ok(TransferGuard { id, token })
}

/// 按标识取消；只有旧调用没有提供标识时才使用最近领取的任务。
pub(crate) fn cancel_transfer(request_id: Option<&str>) -> Result<bool, String> {
    let mut guard = transfers().lock().map_err(|e| e.to_string())?;
    let id = request_id
        .map(str::to_string)
        .or_else(|| guard.latest.clone());
    let Some(id) = id else {
        return Ok(false);
    };
    if let Some(request) = guard.jobs.remove(&id) {
        request.token.flag.store(true, Ordering::SeqCst);
        if guard.latest.as_deref() == Some(id.as_str()) {
            guard.latest = None;
        }
        return Ok(true);
    }
    Ok(false)
}

/// 退出或空间维护时通知全部在途任务在原有安全点停止。
pub(crate) fn cancel_all_transfers() -> Result<(), String> {
    let mut guard = transfers().lock().map_err(|e| e.to_string())?;
    for (_, request) in std::mem::take(&mut guard.jobs) {
        request.token.flag.store(true, Ordering::SeqCst);
    }
    guard.latest = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finishing_old_transfer_preserves_new_cancellation_and_drop_notifies_worker() {
        let early = prepare_transfer().unwrap();
        assert!(cancel_transfer(Some(&early)).unwrap());
        assert!(begin_transfer(Some(&early)).is_err());
        let old = begin_transfer(None).unwrap();
        let old_worker = old.token();
        let new = begin_transfer(None).unwrap();
        let new_worker = new.token();
        assert!(cancel_transfer(Some(&old.id)).unwrap());
        assert!(old_worker.check().is_err());
        assert!(new_worker.check().is_ok());
        drop(old);
        assert!(cancel_transfer(Some(&new.id)).unwrap());
        assert!(new_worker.check().is_err());
        drop(new);
        assert!(!cancel_transfer(None).unwrap());
    }
}
