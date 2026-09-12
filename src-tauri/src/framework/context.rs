//! 数据上下文（AR06 §9.1）：进程内唯一的存储位置 + 空间/代际标识 + 本次启动 epoch。
//!
//! 为什么需要：此前每次调用都从 `settings.json` 现读存储根、各模块自行拼 `app_data_dir()`，
//! 运行期改配置会中途换根，也没有任何东西能标识「这一次启动」，晚到任务无法判定属于哪次上下文。
//!
//! 不变式（改动前先读这里）：
//! - **不可变**：`DataContext` 字段私有、无 setter，进程内只初始化一次（`OnceLock`），
//!   之后 `paths` 的分区路径与 `PluginDb` 的数据文件都取自同一实例，运行期改配置只影响下次启动；
//! - **空间与代际只由框架给出**：模块不得自建空间 id、不得把账号 id 当空间 id（默认空间身份与
//!   索引由导入导出 L0/L1 定义，本模块只承载其值，不做第二套配置）；
//! - **epoch 只标识本次启动**：用于丢弃晚到事件/任务（`is_current`），不充当云同步 revision；
//! - **维护互斥只有一处**：根迁移、导入提交、空间激活、更新安装共用 `maintenance_guard()`，
//!   前端禁用按钮不算锁，最终互斥在 Rust 侧。
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;

use super::paths;

/// 默认空间标识：L0/L1 交付前唯一空间（禁止模块自建空间 id 或把账号 id 当空间 id）
pub const DEFAULT_SPACE_ID: &str = "default";
/// 默认空间代际：空间切换/导入激活后由 L1 递增；当前进程内不重新赋值
pub const DEFAULT_GENERATION_ID: u64 = 1;

/// 存储位置四分区（与 `docs/02-architecture.md` §3.1 的四分区布局一致）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLocation {
    /// 存储根目录（配置项的生效值，或降级后的默认目录）
    pub root: PathBuf,
    /// 数据分区 `<root>/data`（插件 SQLite、known_hosts、本地凭据文件）
    pub data: PathBuf,
    /// 凭证分区 `<root>/vault`
    pub vault: PathBuf,
    /// 日志分区 `<root>/logs`
    pub logs: PathBuf,
}

impl StorageLocation {
    /// 由存储根推导四分区（纯计算，不触碰文件系统）
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            data: root.join("data"),
            vault: root.join("vault"),
            logs: root.join("logs"),
            root,
        }
    }
}

/// 数据上下文：不可变，进程内唯一
pub struct DataContext {
    /// 空间标识（默认空间；导入导出 L0/L1 交付后由它给出真实身份，模块不得自建）
    space_id: &'static str,
    /// 空间代际（导入激活/空间切换后递增，用于判定计划是否过期）
    generation_id: u64,
    /// 本次启动标识（与 `EPOCH_SEQ` 对应，仅用于丢弃晚到事件，不是云同步 revision）
    epoch: u64,
    /// 存储位置四分区（启动时解析一次，运行期不再更换）
    location: StorageLocation,
}

impl DataContext {
    /// 空间标识（默认空间；L0/L1 交付后由导入导出层提供真实身份）
    pub fn space_id(&self) -> &'static str {
        self.space_id
    }

    /// 空间代际（导入激活/空间切换后递增，用于判定计划是否过期）
    pub fn generation_id(&self) -> u64 {
        self.generation_id
    }

    /// 本次启动标识（进程内单调递增，仅用于丢弃晚到事件）
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

/// 进程内唯一上下文
static CONTEXT: OnceLock<DataContext> = OnceLock::new();
/// epoch 序列（每次启动初始化一次，测试可多次调用验证单调性）
static EPOCH_SEQ: AtomicU64 = AtomicU64::new(1);
/// 被丢弃的晚到事件计数（可诊断统计，不做静默忽略）
static STALE_DROPPED: AtomicU64 = AtomicU64::new(0);

/// 取下一个 epoch（只用于 `init`）
fn next_epoch() -> u64 {
    EPOCH_SEQ.fetch_add(1, Ordering::SeqCst)
}

/// 固定上下文（首次调用生效；后续调用返回同一实例，参数被忽略）
pub fn init(location: StorageLocation) -> &'static DataContext {
    CONTEXT.get_or_init(|| DataContext {
        space_id: DEFAULT_SPACE_ID,
        generation_id: DEFAULT_GENERATION_ID,
        epoch: next_epoch(),
        location,
    })
}

/// 启动期固定上下文：解析存储根（配置优先、不可写时降级默认目录）后固定下来。
///
/// 可写性降级与告警的语义与迁移前一致，只是从「每次调用都判定」变为「启动时判定一次」。
pub fn init_from_app(app: &AppHandle) -> Result<&'static DataContext, String> {
    let default = paths::default_root(app)?;
    let configured = paths::read_setting(app, paths::KEY_STORAGE_ROOT)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let root = paths::resolve_root(&configured, &default);
    let root = if root != default && !paths::is_writable_dir(&root) {
        eprintln!(
            "[storage] 配置的存储目录不可用，已降级到默认目录: {}",
            root.display()
        );
        default
    } else {
        root
    };
    Ok(init(StorageLocation::new(root)))
}

/// 当前上下文（未初始化时 None：测试或极早期调用）
pub fn current() -> Option<&'static DataContext> {
    CONTEXT.get()
}

/// 当前存储位置；未初始化返回 None
pub fn location() -> Option<&'static StorageLocation> {
    CONTEXT.get().map(|ctx| &ctx.location)
}

/// 当前存储根；未初始化返回 None
pub fn root() -> Option<&'static Path> {
    location().map(|loc| loc.root.as_path())
}

/// 事件/任务是否属于当前启动上下文（false = 晚到，调用方应丢弃并 `note_stale_dropped`）
///
/// 契约入口：消费方（可靠性 T04/T05/T09/T10/T11 的晚到结果判定）尚未接入，
/// 当前仅单元测试覆盖；接入时删除下面这行 `allow`。
#[allow(dead_code)]
pub fn is_current(epoch: u64) -> bool {
    match current() {
        Some(ctx) => ctx.epoch() == epoch,
        None => false,
    }
}

/// 记录一次被丢弃的晚到事件（诊断统计）
///
/// 契约入口：与 `is_current` 同批接入，当前仅单元测试覆盖；接入时删除下面这行 `allow`。
#[allow(dead_code)]
pub fn note_stale_dropped() {
    STALE_DROPPED.fetch_add(1, Ordering::Relaxed);
}

/// 已丢弃的晚到事件累计数
pub fn stale_dropped() -> u64 {
    STALE_DROPPED.load(Ordering::Relaxed)
}

/// 取维护互斥（异步版：维护流程可能跨 await，禁止用同步锁跨 await 持锁）。
///
/// 覆盖范围：根迁移、导入提交、空间激活、更新安装——同时只允许一个在跑。
pub async fn maintenance_guard() -> tokio::sync::MutexGuard<'static, ()> {
    static GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    GUARD.lock().await
}

/// 登记测试期的模块描述表清理入口所用（生产不调用）
#[cfg(test)]
pub(crate) fn reset_for_test() {
    // 上下文本身不可重置（OnceLock 语义即契约），仅清理计数便于断言
    STALE_DROPPED.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 四分区由根推导：纯计算，root 不重复出现
    #[test]
    fn location_derives_partitions() {
        let loc = StorageLocation::new(PathBuf::from("D:/pb-root"));
        assert_eq!(loc.data, PathBuf::from("D:/pb-root/data"));
        assert_eq!(loc.vault, PathBuf::from("D:/pb-root/vault"));
        assert_eq!(loc.logs, PathBuf::from("D:/pb-root/logs"));
        assert_eq!(loc.root, PathBuf::from("D:/pb-root"));
    }

    /// 上下文首次初始化即固定：再次 init 返回同一实例，运行期不会换根
    #[test]
    fn context_is_fixed_once() {
        let first = init(StorageLocation::new(PathBuf::from("D:/pb-first")));
        let second = init(StorageLocation::new(PathBuf::from("D:/pb-second")));
        assert!(std::ptr::eq(first, second), "上下文必须唯一");
        assert_eq!(first.location.root, PathBuf::from("D:/pb-first"));
        assert_eq!(
            current().map(|ctx| ctx.location.root.clone()),
            Some(PathBuf::from("D:/pb-first"))
        );
    }

    /// epoch：当前 epoch 有效、其他 epoch 一律判为晚到；丢弃计数可诊断
    #[test]
    fn epoch_gates_late_events() {
        let ctx = init(StorageLocation::new(PathBuf::from("D:/pb-first")));
        reset_for_test();
        assert!(is_current(ctx.epoch()));
        assert!(!is_current(ctx.epoch().wrapping_add(1)));
        note_stale_dropped();
        note_stale_dropped();
        assert_eq!(stale_dropped(), 2);
    }

    /// 维护互斥：同时只允许一个持有（前端禁用按钮不算锁）
    #[test]
    fn maintenance_guard_is_exclusive() {
        tauri::async_runtime::block_on(async {
            let guard = maintenance_guard().await;
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(50), maintenance_guard())
                    .await
                    .is_err(),
                "第二个维护任务不应拿到锁"
            );
            drop(guard);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(500), maintenance_guard())
                    .await
                    .is_ok(),
                "释放后应能再次取锁"
            );
        });
    }
}
