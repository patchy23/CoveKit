//! 框架 · 存储不可用时的可见恢复状态（可靠性 T01-5/T01-6）
//!
//! 语义约定：
//! - 配置的存储盘不可用（未插盘、只读、路径失效）时**不得静默退回默认目录新建一套空环境**，
//!   否则用户会以为「数据没了」，新环境也可能在盘回来后与真实数据分叉。
//! - 此时生效根保持为配置值，业务读写自然失败并可见；恢复状态通过 IPC 上报，前端显示恢复页。
//! - 磁盘重新出现不会让同进程自动换根：用户必须显式选择动作（重试 / 选择新数据环境 /
//!   改用默认数据环境），且生效一律需要重启。
//! - 迁移失败同样登记恢复状态，并保留源、计划与错误详情。

use std::sync::Mutex;

use serde::Serialize;

/// 恢复原因（前端据此选择文案与可选动作）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryReason {
    /// 配置的数据目录不可用（未连接、只读或路径失效）
    ConfiguredRootUnavailable,
    /// 上次启动的根迁移失败
    MigrationFailed,
}

/// 恢复状态（展示用；不含任何凭证或文件内容）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRecovery {
    /// 原因分类
    pub reason: RecoveryReason,
    /// 配置里指向的数据目录
    pub configured_root: String,
    /// 本次运行实际生效的根（恢复态下等于 configured_root，不回退默认目录）
    pub active_root: String,
    /// 关联的迁移计划标识（迁移失败时存在）
    pub plan_id: Option<String>,
    /// 恢复建议（面向用户的一句话，已脱敏）
    pub detail: String,
    /// 可选动作：重试探测
    pub can_retry: bool,
    /// 可选动作：改用默认数据环境（清空配置根）
    pub can_use_default: bool,
    /// 发现时间（Unix 毫秒）
    pub created_at: i64,
}

impl StorageRecovery {
    /// 配置根不可用
    pub fn configured_unavailable(configured_root: &str, active_root: &str) -> Self {
        Self {
            reason: RecoveryReason::ConfiguredRootUnavailable,
            configured_root: configured_root.to_string(),
            active_root: active_root.to_string(),
            plan_id: None,
            detail: "配置的数据目录当前不可用，请恢复该磁盘后重试，或选择新的数据环境。".into(),
            can_retry: true,
            can_use_default: true,
            created_at: now_ms(),
        }
    }

    /// 根迁移失败（保留计划与错误）
    pub fn migration_failed(
        configured_root: &str,
        active_root: &str,
        plan_id: Option<String>,
        detail: String,
    ) -> Self {
        Self {
            reason: RecoveryReason::MigrationFailed,
            configured_root: configured_root.to_string(),
            active_root: active_root.to_string(),
            plan_id,
            detail,
            can_retry: true,
            can_use_default: false,
            created_at: now_ms(),
        }
    }
}

/// 进程内恢复状态（启动时确定；用户动作后更新）
static RECOVERY: Mutex<Option<StorageRecovery>> = Mutex::new(None);

/// 登记恢复状态（后写覆盖先写：以最近一次判定为准）
pub fn set(recovery: StorageRecovery) {
    if let Ok(mut slot) = RECOVERY.lock() {
        *slot = Some(recovery);
    }
}

/// 当前恢复状态（None = 正常）
pub fn current() -> Option<StorageRecovery> {
    RECOVERY.lock().ok().and_then(|slot| slot.clone())
}

/// 清除恢复状态（用户在恢复页选定动作并确认后调用）
pub fn clear() {
    if let Ok(mut slot) = RECOVERY.lock() {
        *slot = None;
    }
}

/// 测试串行化：恢复状态是进程级共享，凡读写它的用例必须持锁（避免并行调度下互相覆盖）
#[cfg(test)]
pub fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

/// 当前 Unix 毫秒
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_state_roundtrip_is_process_wide() {
        let _guard = test_guard();
        clear();
        assert!(current().is_none());

        set(StorageRecovery::configured_unavailable(
            "E:/patchybox",
            "E:/patchybox",
        ));
        let state = current().expect("恢复状态应可读回");
        assert_eq!(state.reason, RecoveryReason::ConfiguredRootUnavailable);
        assert_eq!(state.configured_root, "E:/patchybox");
        assert!(state.can_retry && state.can_use_default);
        assert!(state.plan_id.is_none());

        // 迁移失败必须带计划标识与错误详情，便于恢复页给出可执行动作
        set(StorageRecovery::migration_failed(
            "C:/old",
            "C:/old",
            Some("plan-1".into()),
            "校验失败：文件数不一致".into(),
        ));
        let state = current().expect("恢复状态应可读回");
        assert_eq!(state.reason, RecoveryReason::MigrationFailed);
        assert_eq!(state.plan_id.as_deref(), Some("plan-1"));
        assert!(state.detail.contains("校验失败"));
        assert!(!state.can_use_default, "迁移失败不应引导用户改用默认环境");

        clear();
        assert!(current().is_none());
    }
}
