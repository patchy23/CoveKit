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

/// 默认空间标识：L1 只放置这一个空间（禁止模块自建空间 id 或把账号 id 当空间 id）
pub const DEFAULT_SPACE_ID: &str = "default";
/// 默认空间代际：空间切换/导入激活后由后续批次递增；当前进程内不重新赋值
pub const DEFAULT_GENERATION_ID: u64 = 1;

/// 空间数据的落盘形态。
///
/// 两种形态都要能被同一个启动解析表达（数据边界冻结条款 §13.3）：
/// - `LegacyFlat`：空间即设备根，数据与凭证在 `<deviceRoot>/{data,vault}`。默认空间在迁移或
///   导入提交之前保持此形态，**不硬搬**；
/// - `Partitioned`：空间落在 `<deviceRoot>/spaces/<spaceId>/generations/<generationId>/` 之下，
///   日志与缓存按空间分层放在设备根下（不随空间迁移、不参与导出）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// 旧扁平布局（默认空间当前形态）
    LegacyFlat,
    /// 分区布局（后续批次的空间形态）
    Partitioned,
}

impl LayoutKind {
    /// 稳定展示名（设置页与诊断输出用；前端契约的取值来源）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyFlat => "legacyFlat",
            Self::Partitioned => "partitioned",
        }
    }
}

/// 存储位置：一次运行只解析一次的落盘位置描述符。
///
/// 分三层含义，改动前先分清，避免把设备级产物写进空间目录（或反过来）：
/// - `device_root`：设备级根（配置项生效值或默认目录）。日志/缓存分层基、根迁移的源与目标、
///   「存储位置」卡片展示的都是它，**跨空间共享**；
/// - `root`：空间根，数据与凭证分区之上。旧扁平布局下与设备根相同；
/// - `data` / `vault` / `logs` / `cache`：实际分区目录，所有消费方只取这四个字段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLocation {
    /// 落盘形态（旧扁平 / 分区）
    pub layout: LayoutKind,
    /// 设备级存储根（跨空间共享；日志缓存分层基与根迁移的源）
    pub device_root: PathBuf,
    /// 空间根（数据与凭证之上；旧扁平布局下与设备根相同）
    pub root: PathBuf,
    /// 空间标识（本批只有一个默认空间；后续批次由空间解析给出）
    pub space_id: String,
    /// 数据分区 `<空间根>/data`（插件 SQLite、known_hosts、本地凭据文件）
    pub data: PathBuf,
    /// 凭证分区 `<空间根>/vault`
    pub vault: PathBuf,
    /// 日志分区：旧扁平布局为 `<设备根>/logs`，分区布局为 `<设备根>/logs/<空间 id>`
    pub logs: PathBuf,
    /// 缓存分区：旧扁平布局为 `<设备根>/cache`，分区布局为 `<设备根>/cache/<空间 id>`
    pub cache: PathBuf,
}

impl StorageLocation {
    /// 指定空间标识的旧扁平布局（默认空间在迁移/导入提交前保持此形态）。
    ///
    /// 纯计算，不触碰文件系统。
    pub fn legacy_for(root: impl Into<PathBuf>, space_id: &str) -> Self {
        let root = root.into();
        Self {
            layout: LayoutKind::LegacyFlat,
            device_root: root.clone(),
            data: root.join("data"),
            vault: root.join("vault"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            root,
            space_id: space_id.to_string(),
        }
    }

    /// 分区布局：空间在 `<设备根>/spaces/<空间 id>/generations/<代际>/` 之下，
    /// 日志与缓存按空间分层落在设备根下（可重建产物，不随空间迁移）。
    ///
    /// 纯计算，不触碰文件系统；目录建立与可读性校验由空间激活流程负责。
    pub fn partitioned(
        device_root: impl Into<PathBuf>,
        space_id: &str,
        generation_id: u64,
    ) -> Self {
        let device_root = device_root.into();
        let root = device_root
            .join("spaces")
            .join(space_id)
            .join("generations")
            .join(generation_id.to_string());
        Self {
            layout: LayoutKind::Partitioned,
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
    /// 空间标识（本批为默认空间；后续批次由空间解析给出，模块不得自建）
    space_id: String,
    /// 空间代际（导入激活/空间切换后递增，用于判定计划是否过期）
    generation_id: u64,
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
///
/// `generation_id` 来自空间解析结果（本批为默认代际 1）：代际与空间标识必须同时落进上下文，
/// 否则「计划是否过期」会与真实空间对不上。
pub fn init(location: StorageLocation, generation_id: u64) -> &'static DataContext {
    CONTEXT.get_or_init(|| {
        let space_id = location.space_id.clone();
        DataContext {
            space_id,
            generation_id,
            epoch: next_epoch(),
            location,
        }
    })
}

/// 启动期固定上下文：解析存储根与活动空间后固定下来，**在维护阶段之后调用**。
///
/// 顺序是契约（任务书 §13.3/§13.4）：根迁移（维护阶段，可能要换盘）→ 空间解析（读设备级
/// 自举配置）→ 固定上下文。上下文一旦固定便不再换根、不再换空间，因此三步顺序不能颠倒：
/// 先固定上下文会让「切换空间重启生效」永远读不到新值。
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
    // 空间解析是位置描述符的唯一来源：默认空间保持旧扁平布局（本批零迁移），
    // 标识不可用时可见回落，绝不静默新建空环境。
    let resolution = crate::framework::space::resolve_active(app);
    let location = crate::framework::space::location_for(&root, &resolution);
    Ok(init(location, resolution.generation_id))
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
/// 晚到则丢弃并计数）——空间切换（E 批 L1）落地后消费方不必再改。
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

/// 登记测试期的模块描述表清理入口所用（生产不调用）
#[cfg(test)]
pub(crate) fn reset_for_test() {
    // 上下文本身不可重置（OnceLock 语义即契约），仅清理计数便于断言
    STALE_DROPPED.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧扁平布局：分区由根推导，且与改动前的算法逐字符相同（默认空间零迁移的硬门禁）
    #[test]
    fn location_derives_partitions() {
        let loc = StorageLocation::legacy_for(PathBuf::from("D:/pb-root"), DEFAULT_SPACE_ID);
        // 期望值写死为改动前的算法结果，不用 loc 自身推导，避免「自己验自己」
        assert_eq!(loc.data, PathBuf::from("D:/pb-root/data"));
        assert_eq!(loc.vault, PathBuf::from("D:/pb-root/vault"));
        assert_eq!(loc.logs, PathBuf::from("D:/pb-root/logs"));
        assert_eq!(loc.cache, PathBuf::from("D:/pb-root/cache"));
        assert_eq!(loc.root, PathBuf::from("D:/pb-root"));
        assert_eq!(loc.device_root, PathBuf::from("D:/pb-root"));
        assert_eq!(loc.layout, LayoutKind::LegacyFlat);
        assert_eq!(loc.space_id, DEFAULT_SPACE_ID);
    }

    /// 分区布局：空间在 spaces/<id>/generations/<n> 之下，日志与缓存按空间分层留在设备根下
    #[test]
    fn partitioned_location_keeps_logs_and_cache_on_device_root() {
        let loc = StorageLocation::partitioned(PathBuf::from("D:/pb-root"), "9f1c4e2a", 3);
        assert_eq!(loc.layout, LayoutKind::Partitioned);
        assert_eq!(loc.device_root, PathBuf::from("D:/pb-root"));
        assert_eq!(
            loc.root,
            PathBuf::from("D:/pb-root/spaces/9f1c4e2a/generations/3")
        );
        assert_eq!(loc.data, loc.root.join("data"));
        assert_eq!(loc.vault, loc.root.join("vault"));
        // 日志与缓存是可重建产物：不随空间迁移，因此落在设备根下按空间分层
        assert_eq!(loc.logs, PathBuf::from("D:/pb-root/logs/9f1c4e2a"));
        assert_eq!(loc.cache, PathBuf::from("D:/pb-root/cache/9f1c4e2a"));
        assert_eq!(loc.space_id, "9f1c4e2a");
    }

    /// 两种形态下空间根都不得把数据写进另一个空间的目录（隔离的最小判据）
    #[test]
    fn layouts_do_not_share_data_dirs() {
        let device_root = PathBuf::from("D:/pb-root");
        let a = StorageLocation::partitioned(&device_root, "aaaa1111", 1);
        let b = StorageLocation::partitioned(&device_root, "bbbb2222", 1);
        let default = StorageLocation::legacy_for(&device_root, DEFAULT_SPACE_ID);
        for (left, right) in [(&a, &b), (&a, &default), (&b, &default)] {
            assert_ne!(left.data, right.data, "数据分区不得重叠");
            assert_ne!(left.vault, right.vault, "凭证分区不得重叠");
            assert_ne!(left.logs, right.logs, "日志分区不得重叠");
            assert_ne!(left.cache, right.cache, "缓存分区不得重叠");
        }
    }

    /// 上下文首次初始化即固定：再次 init 返回同一实例，运行期不会换根
    #[test]
    fn context_is_fixed_once() {
        let first = init(
            StorageLocation::legacy_for(PathBuf::from("D:/pb-first"), DEFAULT_SPACE_ID),
            DEFAULT_GENERATION_ID,
        );
        let second = init(
            StorageLocation::legacy_for(PathBuf::from("D:/pb-second"), DEFAULT_SPACE_ID),
            2,
        );
        assert!(std::ptr::eq(first, second), "上下文必须唯一");
        assert_eq!(first.location.root, PathBuf::from("D:/pb-first"));
        // 代际也随首次初始化固定：切换空间必须重启，运行期不得改代际
        assert_eq!(first.generation_id(), DEFAULT_GENERATION_ID);
        assert_eq!(
            current().map(|ctx| ctx.location.root.clone()),
            Some(PathBuf::from("D:/pb-first"))
        );
    }

    /// epoch：当前 epoch 有效、其他 epoch 一律判为晚到；丢弃计数可诊断
    #[test]
    fn epoch_gates_late_events() {
        let ctx = init(
            StorageLocation::legacy_for(PathBuf::from("D:/pb-first"), DEFAULT_SPACE_ID),
            DEFAULT_GENERATION_ID,
        );
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
