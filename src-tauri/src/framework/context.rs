//! 数据上下文（AR06 §9.1）：进程内唯一的存储位置 + 空间标识 + 本次启动 epoch。
//!
//! 为什么需要：此前每次调用都从 `settings.json` 现读存储根、各模块自行拼 `app_data_dir()`，
//! 运行期改配置会中途换根，也没有任何东西能标识「这一次启动」，晚到任务无法判定属于哪次上下文。
//!
//! 不变式（改动前先读这里）：
//! - **不可变**：`DataContext` 字段私有、无 setter，进程内只初始化一次（`OnceLock`），
//!   之后 `paths` 的分区路径与 `PluginDb` 的数据文件都取自同一实例，运行期改配置只影响下次启动；
//! - **空间只由框架给出**：空间标识是全局唯一的 uid（UUIDv4），模块不得自建空间 id、
//!   不得把账号 id 当空间 id（默认空间身份与索引由导入导出批次定义，本模块只承载其值）；
//! - **epoch 只标识本次启动**：用于丢弃晚到事件/任务（`is_current`），不充当云同步 revision；
//! - **维护互斥只有一处**：根迁移、导入提交、空间激活、更新安装共用 `maintenance_guard()`，
//!   前端禁用按钮不算锁，最终互斥在 Rust 侧；
//! - **没有代际**：2026-09-16 裁决后空间布局为 `spaces/<uid>/`（无 `generations/` 层），
//!   合并导入用原地事务提交（SQLite 事务 + 文件原子替换），不做整目录克隆与指针切换。
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;

use super::paths;

/// 存储位置：一次运行只解析一次的落盘位置描述符。
///
/// 分两层含义，改动前先分清，避免把设备级产物写进空间目录（或反过来）：
/// - `device_root`：设备级根（配置项生效值或默认目录）。日志/缓存分层基、根迁移的源与目标、
///   「存储位置」卡片展示的都是它，**跨空间共享**；
/// - `root`：空间根（`<设备根>/spaces/<uid>`），数据与凭证分区之上；
/// - `data` / `vault` / `logs` / `cache`：实际分区目录，所有消费方只取这四个字段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLocation {
    /// 设备级存储根（跨空间共享；日志缓存分层基与根迁移的源）
    pub device_root: PathBuf,
    /// 空间根 `<设备根>/spaces/<uid>`（数据与凭证之上）
    pub root: PathBuf,
    /// 空间标识（全局唯一 uid，由空间解析给出）
    pub space_id: String,
    /// 数据分区 `<空间根>/data`（插件 SQLite、known_hosts、本地凭据文件）
    pub data: PathBuf,
    /// 凭证分区 `<空间根>/vault`
    pub vault: PathBuf,
    /// 日志分区 `<设备根>/logs/<空间 id>`（可重建产物，不随空间迁移）
    pub logs: PathBuf,
    /// 缓存分区 `<设备根>/cache/<空间 id>`（可重建产物，不随空间迁移）
    pub cache: PathBuf,
}

impl StorageLocation {
    /// 构造存储位置描述符：空间在 `<设备根>/spaces/<空间 id>/` 之下，
    /// 日志与缓存按空间分层落在设备根下（可重建产物，不随空间迁移）。
    ///
    /// 纯计算，不触碰文件系统；目录建立与可读性校验由空间激活流程负责。
    pub fn for_space(device_root: impl Into<PathBuf>, space_id: &str) -> Self {
        let device_root = device_root.into();
        let root = device_root.join("spaces").join(space_id);
        Self {
            data: root.join("data"),
            vault: root.join("vault"),
            logs: device_root.join("logs").join(space_id),
            cache: device_root.join("cache").join(space_id),
            device_root,
            root,
            space_id: space_id.to_string(),
        }
    }
}

/// 数据上下文：不可变，进程内唯一
pub struct DataContext {
    /// 空间标识（全局唯一 uid；模块不得自建）
    space_id: String,
    /// 本次启动标识（与 `EPOCH_SEQ` 对应，仅用于丢弃晚到事件，不是云同步 revision）
    epoch: u64,
    /// 存储位置描述符（启动时解析一次，运行期不再更换）
    location: StorageLocation,
}

impl DataContext {
    /// 空间标识（取值来自存储位置描述符，不再有第二处来源）
    pub fn space_id(&self) -> &str {
        &self.space_id
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
    CONTEXT.get_or_init(|| {
        let space_id = location.space_id.clone();
        DataContext {
            space_id,
            epoch: next_epoch(),
            location,
        }
    })
}

/// 启动期固定上下文：解析存储根与活动空间后固定下来，**在维护阶段之后调用**。
///
/// 顺序是契约：根迁移（维护阶段，可能要换盘）→ 布局迁移（旧文件进分区）→ 空间化迁移
/// （uid 生成与数据入位，见 `space::migration`）→ 空间解析（读设备级自举配置）→ 固定上下文。
/// 上下文一旦固定便不再换根、不再换空间，因此顺序不能颠倒。
///
/// 配置根不可用时不降级默认目录：生效根保持配置值并登记可见恢复状态（`storage::recovery`），
/// 业务读写自然失败且用户能看到恢复页，不会静默在默认目录新建一套空环境。
/// 恢复动作（重试/选择新环境/改用默认）一律需要重启，因此本函数不做运行期换根。
pub fn init_from_app(app: &AppHandle) -> Result<&'static DataContext, String> {
    let default = paths::default_root(app)?;
    let configured = paths::configured_root(app).unwrap_or_default();
    let root = paths::resolve_root(&configured, &default);
    if root != default && !paths::is_writable_dir(&root) {
        // 磁盘未连接或路径失效：登记恢复状态，生效根保持配置值
        crate::framework::storage::recovery::set(
            crate::framework::storage::recovery::StorageRecovery::configured_unavailable(
                &root.display().to_string(),
                &root.display().to_string(),
            ),
        );
        eprintln!(
            "[storage] 配置的存储目录不可用，进入恢复状态（不回退默认目录）: {}",
            root.display()
        );
    }
    // 空间解析是位置描述符的唯一来源：标识不可用时可见回落，绝不静默新建空环境
    let resolution = crate::framework::space::resolve_active(app)?;
    let location = crate::framework::space::location_for(&root, &resolution);
    Ok(init(location))
}

/// 当前上下文（未初始化时 None：测试或极早期调用）
pub fn current() -> Option<&'static DataContext> {
    CONTEXT.get()
}

/// 当前存储位置描述符；未初始化返回 None
pub fn location() -> Option<&'static StorageLocation> {
    CONTEXT.get().map(|ctx| &ctx.location)
}

/// 设备级存储根（配置项生效值；日志缓存分层基与根迁移的源，跨空间共享）；未初始化返回 None
pub fn root() -> Option<&'static Path> {
    location().map(|loc| loc.device_root.as_path())
}

/// 事件/任务是否属于当前启动上下文（false = 晚到，调用方应丢弃并 `note_stale_dropped`）
///
/// 消费方：`tasks` 的长任务出口（登记时记下 epoch，进度与结束前判一次，
/// 晚到则丢弃并计数）。
pub fn is_current(epoch: u64) -> bool {
    match current() {
        Some(ctx) => ctx.epoch() == epoch,
        None => false,
    }
}

/// 记录一次被丢弃的晚到事件（诊断统计）
///
/// 与 `is_current` 同批接入：调用点见 `tasks` 的长任务出口与退出日志的统计打印。
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

/// 导入提交窗口的写冻结标记（进程内；只在合并/覆盖导入提交期间为 true）
static WRITE_FROZEN: AtomicBool = AtomicBool::new(false);

/// 写冻结守卫（RAII）：提交开始挂上、离开作用域自动解除
pub struct WriteFreezeGuard(());

impl WriteFreezeGuard {
    /// 开启写冻结（同一时间只应有一个提交窗口；由 maintenance_guard 保证互斥）
    pub fn begin() -> Self {
        WRITE_FROZEN.store(true, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for WriteFreezeGuard {
    fn drop(&mut self) {
        WRITE_FROZEN.store(false, Ordering::SeqCst);
    }
}

/// 当前是否处于导入提交窗口（写冻结）
pub fn writes_frozen() -> bool {
    WRITE_FROZEN.load(Ordering::SeqCst)
}

/// 写入口统一校验：提交窗口内拒绝并给出明确文案（不静默丢弃、不写进旧数据）。
///
/// 导入提交自身不走这条（它直接调用 `*_at` 原语）；覆盖的是用户操作进入的
/// 框架写入口（凭证保存、偏好写入等）。SQLite 侧的并发由 busy_timeout 序列化兜底。
pub fn assert_writable() -> Result<(), String> {
    if writes_frozen() {
        return Err("数据正在提交导入，请稍候重试".into());
    }
    Ok(())
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

    const SPACE_A: &str = "9f1c4e2a-1111-4111-8111-111111111111";
    const SPACE_B: &str = "bbbb2222-2222-4222-8222-222222222222";

    /// 空间布局：空间在 spaces/<uid> 之下（无代际层），日志与缓存按空间分层留在设备根下
    #[test]
    fn space_location_layout() {
        let loc = StorageLocation::for_space(PathBuf::from("D:/pb-root"), SPACE_A);
        assert_eq!(loc.device_root, PathBuf::from("D:/pb-root"));
        assert_eq!(loc.root, PathBuf::from("D:/pb-root/spaces").join(SPACE_A));
        assert_eq!(loc.data, loc.root.join("data"));
        assert_eq!(loc.vault, loc.root.join("vault"));
        // 日志与缓存是可重建产物：不随空间迁移，因此落在设备根下按空间分层
        assert_eq!(loc.logs, PathBuf::from("D:/pb-root/logs").join(SPACE_A));
        assert_eq!(loc.cache, PathBuf::from("D:/pb-root/cache").join(SPACE_A));
        assert_eq!(loc.space_id, SPACE_A);
    }

    /// 不同空间的分区目录不得重叠（隔离的最小判据）
    #[test]
    fn spaces_do_not_share_data_dirs() {
        let device_root = PathBuf::from("D:/pb-root");
        let a = StorageLocation::for_space(&device_root, SPACE_A);
        let b = StorageLocation::for_space(&device_root, SPACE_B);
        assert_ne!(a.data, b.data, "数据分区不得重叠");
        assert_ne!(a.vault, b.vault, "凭证分区不得重叠");
        assert_ne!(a.logs, b.logs, "日志分区不得重叠");
        assert_ne!(a.cache, b.cache, "缓存分区不得重叠");
    }

    /// 上下文首次初始化即固定：再次 init 返回同一实例，运行期不会换根
    #[test]
    fn context_is_fixed_once() {
        let first = init(StorageLocation::for_space(
            PathBuf::from("D:/pb-first"),
            SPACE_A,
        ));
        let second = init(StorageLocation::for_space(
            PathBuf::from("D:/pb-second"),
            SPACE_B,
        ));
        assert!(std::ptr::eq(first, second), "上下文必须唯一");
        assert_eq!(
            first.location.root,
            PathBuf::from("D:/pb-first/spaces").join(SPACE_A)
        );
        assert_eq!(
            current().map(|ctx| ctx.location.root.clone()),
            Some(PathBuf::from("D:/pb-first/spaces").join(SPACE_A))
        );
    }

    /// 写冻结：窗口内写入口被拒、守卫离开作用域即恢复（RAII 解除，不依赖手工复位）
    #[test]
    fn write_freeze_blocks_and_recovers() {
        assert!(assert_writable().is_ok());
        {
            let _freeze = WriteFreezeGuard::begin();
            assert!(writes_frozen());
            let error = assert_writable().expect_err("冻结窗口内写入必须被拒");
            assert!(error.contains("导入"), "{error}");
        }
        assert!(!writes_frozen());
        assert!(assert_writable().is_ok(), "守卫离开后必须恢复可写");
    }

    /// epoch：当前 epoch 有效、其他 epoch 一律判为晚到；丢弃计数可诊断
    #[test]
    fn epoch_gates_late_events() {
        let ctx = init(StorageLocation::for_space(
            PathBuf::from("D:/pb-first"),
            SPACE_A,
        ));
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
