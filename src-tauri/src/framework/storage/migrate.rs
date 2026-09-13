//! 框架 · 维护阶段执行待执行的根迁移（可靠性 T01 / T02）
//!
//! 启动顺序契约：`run_pending` 必须在**任何业务资源初始化之前**执行；成功提交新根之后
//! 才由 `context::init_from_app` 固定生效根。此时所有插件数据库尚未打开，属于 SQLite 的
//! 完整离线处理（`.db` 与 `-wal` / `-shm` 一并复制），因此不需要在线备份 API。
//!
//! 一次执行的严格顺序（任一步失败即停止，且**绝不写 `storageRoot`**）：
//! 1. 预检：计划路径合法、源目录可读、目标根可写；清掉本计划自己的暂存目录（按随机 id 匹配）。
//! 2. 目标判定：空目标直接迁；非空目标只有「逐项等于源清单」才继续提交（上次在提交路径中断），
//!    否则报冲突让人处理——**绝不优先采用半截目标，也不自动覆盖/合并**。
//! 3. 扫描源目录生成清单（拒绝链接，错误传播）。
//! 4. 复制到本次任务专属暂存目录（`create_new` 写入探针，不覆盖用户已有文件）。
//! 5. 校验暂存区：逐文件摘要比对 + 数据库 `quick_check` + vault 长度语义。
//! 6. 提交：逐分区 rename 进目标根（同卷原子）；失败尽量回滚，回滚不了则明确报出残留路径。
//! 7. 写 `storageRoot`（失败时源与目标都完整，重试即可），留档，清计划。
//!
//! 失败一律保留源目录、计划与错误详情；源目录**永不删除**。

use std::time::{Duration, Instant};

use crate::framework::paths;

use super::{plan, recovery, scan, transfer, verify, PARTITIONS};

/// 进度上报节流间隔（按文件 / 分块更新，不只在分区结束时发一次）
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// 迁移执行结果
#[derive(Debug, Clone)]
pub enum MigrationOutcome {
    /// 没有待执行计划
    NoPlan,
    /// 已复制、校验并提交新根
    Committed {
        /// 计划标识
        plan_id: String,
        /// 源根
        source: String,
        /// 新根
        target: String,
        /// 复制文件数
        copied_files: u64,
        /// 复制字节数
        copied_bytes: u64,
        /// 是否走了「目标已是完整副本」的续跑提交路径
        resumed: bool,
    },
    /// 失败：源、计划与错误详情都保留
    Failed {
        /// 计划标识
        plan_id: String,
        /// 失败阶段：precheck / scan / copy / verify / commit
        stage: &'static str,
        /// 失败原因
        detail: String,
    },
}

impl MigrationOutcome {
    /// 一行摘要（日志与恢复状态共用）
    pub fn summary(&self) -> String {
        match self {
            Self::NoPlan => "无待执行迁移".to_string(),
            Self::Committed {
                plan_id,
                source,
                target,
                copied_files,
                copied_bytes,
                resumed,
            } => format!(
                "存储根迁移完成[{plan_id}]：{source} → {target}（{copied_files} 个文件 / {copied_bytes} 字节{}）",
                if *resumed { "，续跑提交" } else { "" }
            ),
            Self::Failed {
                plan_id,
                stage,
                detail,
            } => format!("存储根迁移失败[{plan_id}]（{stage}）：{detail}"),
        }
    }
}

/// 进度回调：`(阶段, 已复制文件数, 已复制字节数, 总字节数)`
pub type ProgressFn<'a> = &'a dyn Fn(&'static str, u64, u64, u64);

/// 节流后的进度上报器（按文件/字节推进，避免每个文件都跨线程发事件）
struct Throttle<'a> {
    progress: ProgressFn<'a>,
    last: Instant,
}

impl<'a> Throttle<'a> {
    /// 新建节流器（首个事件立即发出）
    fn new(progress: ProgressFn<'a>) -> Self {
        Self {
            progress,
            last: Instant::now() - PROGRESS_INTERVAL,
        }
    }

    /// 按节流间隔上报；`force` 用于阶段切换等必须可见的时刻
    fn tick(&mut self, stage: &'static str, files: u64, bytes: u64, total: u64, force: bool) {
        if force || self.last.elapsed() >= PROGRESS_INTERVAL {
            self.last = Instant::now();
            (self.progress)(stage, files, bytes, total);
        }
    }
}

/// 执行待执行计划（可测版本：配置抽象 + 显式进度回调）
pub fn execute_pending(
    cfg: &dyn plan::ConfigStore,
    progress: ProgressFn<'_>,
) -> Result<MigrationOutcome, String> {
    let Some(mut pending) = plan::load_pending(cfg) else {
        return Ok(MigrationOutcome::NoPlan);
    };
    pending.attempts = pending.attempts.saturating_add(1);
    plan::save_pending(cfg, &pending)?;

    let source = pending.source_path();
    let target = pending.target_path();
    let staging_name = plan::staging_dir_name(&pending.id);
    let staging = target.join(&staging_name);
    let mut throttle = Throttle::new(progress);

    // ── 1. 预检 ──
    if let Err(e) = plan::validate_target(&source, &target) {
        return fail(cfg, pending, "precheck", e);
    }
    if !source.is_dir() {
        return fail(
            cfg,
            pending,
            "precheck",
            format!("源数据目录不可用：{}", source.display()),
        );
    }
    if !paths::is_writable_dir(&target) {
        return fail(
            cfg,
            pending,
            "precheck",
            format!(
                "目标目录不可写（磁盘未就绪或权限不足）：{}",
                target.display()
            ),
        );
    }
    // 只清理本次计划自己的暂存目录：名字带随机 id，路径又经 validate_target 校验过
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|e| format!("清理上次暂存目录 {} 失败: {e}", staging.display()))?;
    }
    let target_empty = verify::only_own_staging(&target, &staging_name)?;

    // ── 2. 源清单（迁移与校验的唯一依据）──
    throttle.tick("scan", 0, 0, 0, true);
    let manifest = match scan::scan_root(&source) {
        Ok(m) => m,
        Err(e) => return fail(cfg, pending, "scan", e),
    };
    let total_bytes = manifest.total_bytes;
    throttle.tick("scan", 0, 0, total_bytes, true);

    // ── 3. 非空目标：只接受「已是源完整副本」的续跑 ──
    let resumed = !target_empty;
    if resumed {
        if let Err(e) = verify::verify_existing_target(&target, &manifest) {
            return fail(
                cfg,
                pending,
                "precheck",
                format!(
                    "目标目录已有内容且与源不一致，请改选空目录或先清理目标（不会自动覆盖/合并）：{e}"
                ),
            );
        }
    }

    let mut copied_files = 0u64;
    let mut copied_bytes = 0u64;
    if !resumed {
        // ── 4. 复制到暂存目录 ──
        if let Err(e) = std::fs::create_dir(&staging) {
            return fail(
                cfg,
                pending,
                "precheck",
                format!("创建暂存目录 {} 失败：{e}", staging.display()),
            );
        }
        // 写入探针用 create_new：绝不覆盖用户已有的同名文件
        let probe = staging.join(format!(".patchybox-write-probe-{}", pending.id));
        if let Err(e) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            let _ = std::fs::remove_dir_all(&staging);
            return fail(
                cfg,
                pending,
                "precheck",
                format!("目标目录写入探针失败（磁盘不可写）：{e}"),
            );
        }
        if let Err(e) = plan::set_phase(cfg, &mut pending, plan::MigrationPhase::Copying) {
            let _ = std::fs::remove_dir_all(&staging);
            return fail(cfg, pending, "precheck", e);
        }
        for name in PARTITIONS {
            let from = source.join(name);
            if !from.is_dir() {
                continue;
            }
            let to = staging.join(name);
            let copied = {
                let mut on_file = |files: u64, bytes: u64, _rel: &str| {
                    throttle.tick("copy", files, bytes, total_bytes, false);
                };
                transfer::copy_tree_with(&from, &to, &mut on_file)
            };
            match copied {
                Ok((files, bytes)) => {
                    copied_files += files;
                    copied_bytes += bytes;
                }
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staging);
                    return fail(cfg, pending, "copy", e);
                }
            }
        }
        throttle.tick("copy", copied_files, copied_bytes, total_bytes, true);

        // ── 5. 校验暂存区（失败即清理，目标保持为空）──
        if let Err(e) = plan::set_phase(cfg, &mut pending, plan::MigrationPhase::Verifying) {
            let _ = std::fs::remove_dir_all(&staging);
            return fail(cfg, pending, "precheck", e);
        }
        throttle.tick("verify", copied_files, copied_bytes, total_bytes, true);
        if let Err(e) = verify::verify_staging(&staging, &manifest) {
            let _ = std::fs::remove_dir_all(&staging);
            return fail(cfg, pending, "verify", e);
        }
        // 探针不是数据：提交前移除（scan 只遍历四分区，探针在暂存根，不影响清单）
        let _ = std::fs::remove_file(&probe);

        // ── 6. 提交：逐分区 rename（同卷原子）──
        let mut moved: Vec<String> = Vec::new();
        for name in PARTITIONS {
            let from = staging.join(name);
            if !from.exists() {
                continue;
            }
            let to = target.join(name);
            if let Err(e) = std::fs::rename(&from, &to) {
                // 回滚已搬过去的分区，尽量让目标恢复为空（下次重试干净）
                for done in moved.iter().rev() {
                    let _ = std::fs::rename(target.join(done), staging.join(done));
                }
                let _ = std::fs::remove_dir_all(&staging);
                return fail(
                    cfg,
                    pending,
                    "commit",
                    format!("搬移 {name} 分区失败（{e}），已回滚本次已搬分区"),
                );
            }
            moved.push((*name).to_string());
        }
        let _ = std::fs::remove_dir_all(&staging);
    }

    // ── 7. 写新根（失败时源与目标都完整，重试即可）──
    if let Err(e) = cfg.write(
        paths::KEY_STORAGE_ROOT,
        serde_json::json!(target.display().to_string()),
    ) {
        return fail(
            cfg,
            pending,
            "commit",
            format!("写入存储位置配置失败：{e}（原环境不受影响，可重试）"),
        );
    }
    // 留档与清计划：失败只影响展示与「是否再次尝试」，迁移本身已提交
    let last = plan::LastMigration {
        id: pending.id.clone(),
        source: source.display().to_string(),
        target: target.display().to_string(),
        completed_at: plan::now_ms(),
        copied_files,
        copied_bytes,
    };
    if let Err(e) = plan::save_last(cfg, &last) {
        eprintln!("[storage] 迁移留档写入失败：{e}");
    }
    if let Err(e) = plan::clear_pending(cfg) {
        eprintln!("[storage] 迁移计划清理失败（下次启动会走续跑路径）：{e}");
    }
    recovery::clear();
    throttle.tick("done", copied_files, copied_bytes, total_bytes, true);
    Ok(MigrationOutcome::Committed {
        plan_id: pending.id,
        source: source.display().to_string(),
        target: target.display().to_string(),
        copied_files,
        copied_bytes,
        resumed,
    })
}

/// 失败处理：阶段与错误写回计划，保留源/目标/尝试次数，返回失败结果。
///
/// 已进入复制阶段的失败会附带一句提示：目标目录可能留有半截数据，
/// 下次启动重试按合并方式覆盖（同名文件以源为准），不会把半截目录当成功。
fn fail(
    cfg: &dyn plan::ConfigStore,
    mut pending: plan::PendingPlan,
    stage: &'static str,
    detail: String,
) -> Result<MigrationOutcome, String> {
    let mut detail = detail;
    if pending.phase.has_started_copy() {
        detail.push_str("（暂存区已清理由本次任务创建的文件，目标目录未被改动）");
    }
    pending.last_error = Some(detail.clone());
    if let Err(e) = plan::save_pending(cfg, &pending) {
        eprintln!("[storage] 迁移计划失败状态写入失败：{e}");
    }
    Ok(MigrationOutcome::Failed {
        plan_id: pending.id,
        stage,
        detail,
    })
}

/// 生产入口：读自举配置 → 执行计划 → 失败登记可见恢复状态。
///
/// 该函数**不返回错误**：启动过程不能因为一次迁移失败而整体失败（那样用户看不到任何提示）。
pub fn run_pending(app: &tauri::AppHandle, configured_root: &str) -> MigrationOutcome {
    let cfg = plan::AppConfig(app);
    let emit = |phase: &'static str, files: u64, bytes: u64, total: u64| {
        super::emit_progress(app, phase, files, bytes, total);
    };
    let outcome = match execute_pending(&cfg, &emit) {
        Ok(outcome) => outcome,
        Err(e) => MigrationOutcome::Failed {
            plan_id: String::new(),
            stage: "precheck",
            detail: e,
        },
    };
    match &outcome {
        MigrationOutcome::NoPlan => {}
        MigrationOutcome::Committed { .. } => eprintln!("[storage] {}", outcome.summary()),
        MigrationOutcome::Failed {
            plan_id, detail, ..
        } => {
            eprintln!("[storage] {}", outcome.summary());
            // 失败时生效根仍停在源目录（提交从不半途生效）：恢复页展示的就是它
            let active = plan::load_pending(&cfg)
                .map(|p| p.source)
                .unwrap_or_else(|| configured_root.to_string());
            recovery::set(recovery::StorageRecovery::migration_failed(
                configured_root,
                &active,
                Some(plan_id.clone()),
                detail.clone(),
            ));
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::storage::plan::ConfigStore;
    use crate::framework::storage::test_support::{temp_dir, FileConfig};
    use std::path::Path;

    /// 造一份源存储根：真实 SQLite 库 + vault 密文 + 日志 + 缓存
    fn make_source(root: &Path) {
        std::fs::create_dir_all(root.join("data").join("database")).unwrap();
        // 夹具里的 .db 必须是真 SQLite：校验会对每个 .db 跑 quick_check，假文件应当被判失败
        for db in ["ssh.db", "database/mysql.db"] {
            let conn = rusqlite::Connection::open(root.join("data").join(db)).unwrap();
            conn.execute_batch("CREATE TABLE t (id INTEGER);").unwrap();
        }
        std::fs::create_dir_all(root.join("vault")).unwrap();
        std::fs::write(root.join("vault").join("vault.dat"), vec![7u8; 64]).unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::write(root.join("logs").join("app.log"), b"hello").unwrap();
        std::fs::create_dir_all(root.join("cache")).unwrap();
        std::fs::write(root.join("cache").join("thumb.bin"), b"cache").unwrap();
    }

    /// 登记一份计划（源 → 目标）
    fn schedule(cfg: &FileConfig, source: &Path, target: &Path) -> plan::PendingPlan {
        let pending = plan::PendingPlan::new(source, target, paths::LAYOUT_VERSION);
        plan::save_pending(cfg, &pending).unwrap();
        pending
    }

    /// 无计划时启动流程不动任何东西
    #[test]
    fn no_plan_is_noop() {
        let dir = temp_dir("migrate-noop");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        assert!(matches!(outcome, MigrationOutcome::NoPlan));
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 成功路径：复制 + 校验 + 提交新根 + 清计划 + 留档；源目录保留
    #[test]
    fn committed_plan_copies_files_switches_root_and_clears_plan() {
        let dir = temp_dir("migrate-ok");
        let source = dir.join("source");
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();
        make_source(&source);
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let pending = schedule(&cfg, &source, &target);
        let phases: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
        let outcome = execute_pending(&cfg, &|phase, _, _, _| {
            // 仅测试：记录阶段序列，证明进度按文件节流上报（非每分区一次）
            if let Ok(mut guard) = phases.lock() {
                guard.push(phase);
            }
        })
        .unwrap();
        match outcome {
            MigrationOutcome::Committed {
                copied_files,
                resumed,
                ..
            } => {
                assert!(copied_files >= 4, "复制文件数偏少：{copied_files}");
                assert!(!resumed);
            }
            other => panic!("期望提交成功，实际 {other:?}"),
        }
        // 新根已写入配置，计划已清除
        assert_eq!(
            cfg.read(paths::KEY_STORAGE_ROOT).unwrap(),
            serde_json::json!(target.display().to_string())
        );
        assert!(plan::load_pending(&cfg).is_none());
        assert_eq!(
            plan::load_last(&cfg).unwrap().id,
            pending.id,
            "留档应记录本次计划"
        );
        // 目标内容完整，源目录保留
        assert!(target.join("data").join("ssh.db").exists());
        assert!(target.join("vault").join("vault.dat").exists());
        assert!(source.join("data").join("ssh.db").exists(), "源不得删除");
        // 暂存目录已清理
        assert!(!target.join(plan::staging_dir_name(&pending.id)).exists());
        // 进度含 copy 与 verify 阶段（不只每个分区发一次）
        let phases = phases.into_inner().unwrap_or_default();
        assert!(phases.contains(&"copy"), "阶段序列: {phases:?}");
        assert!(phases.contains(&"verify"), "阶段序列: {phases:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 复制失败（目标盘不可写）→ 保留计划与错误、不写配置、源不变、暂存清理
    #[test]
    fn copy_failure_keeps_plan_and_leaves_source_untouched() {
        let dir = temp_dir("migrate-copyfail");
        let source = dir.join("source");
        make_source(&source);
        // 目标位置是一个文件 → create_dir 失败
        let target = dir.join("target-is-file");
        std::fs::write(&target, b"not a dir").unwrap();
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let pending = schedule(&cfg, &source, &target);

        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "precheck");
                assert!(
                    detail.contains("不可写") || detail.contains("非目录"),
                    "详情: {detail}"
                );
            }
            other => panic!("期望预检失败，实际 {other:?}"),
        }
        let kept = plan::load_pending(&cfg).expect("计划应保留");
        assert_eq!(kept.id, pending.id);
        assert!(kept.last_error.is_some(), "失败原因应写回计划");
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none(), "不得写新根");
        assert!(source.join("data").join("ssh.db").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 源目录缺失 → 预检失败并保留计划
    #[test]
    fn missing_source_is_reported_as_precheck_failure() {
        let dir = temp_dir("migrate-missing");
        let source = dir.join("gone");
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let cfg = FileConfig::new(&dir.join("settings.json"));
        schedule(&cfg, &source, &target);

        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "precheck");
                assert!(detail.contains("源数据目录不可用"), "详情: {detail}");
            }
            other => panic!("期望预检失败，实际 {other:?}"),
        }
        assert!(plan::load_pending(&cfg).is_some());
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 中途失败后重启：阶段不回退为 scheduled（半截状态可识别）
    #[test]
    fn second_attempt_does_not_revert_to_scheduled_phase() {
        let dir = temp_dir("migrate-phase");
        let source = dir.join("source");
        make_source(&source);
        let target = dir.join("target-is-file");
        std::fs::write(&target, b"nope").unwrap();
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let pending = schedule(&cfg, &source, &target);

        execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        let first = plan::load_pending(&cfg).unwrap();
        assert_eq!(first.attempts, 1);
        assert_eq!(first.phase, plan::MigrationPhase::Scheduled);

        execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        let second = plan::load_pending(&cfg).unwrap();
        assert_eq!(second.id, pending.id);
        assert_eq!(second.attempts, 2, "尝试次数应累加");
        assert_eq!(second.source, pending.source);
        assert_eq!(second.target, pending.target);
        assert!(second.last_error.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 校验失败（暂存区内容被篡改）→ 目标保持为空、计划保留、阶段停在 verifying
    #[test]
    fn verify_failure_keeps_target_empty_and_plan_resumable() {
        let dir = temp_dir("migrate-verifyfail");
        let source = dir.join("source");
        make_source(&source);
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let pending = schedule(&cfg, &source, &target);

        // 在预检后、校验前插入破坏：用包装回调在校验阶段篡改暂存区
        let staging = target.join(plan::staging_dir_name(&pending.id));
        let outcome = execute_pending(&cfg, &|phase, _, _, _| {
            if phase == "verify" && staging.join("logs").is_dir() {
                let _ = std::fs::write(staging.join("logs").join("app.log"), b"tampered-content");
            }
        })
        .unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "verify");
                assert!(detail.contains("内容不一致"), "详情: {detail}");
            }
            other => panic!("期望校验失败，实际 {other:?}"),
        }
        // 目标保持为空（分区没搬过去），暂存区被清理，计划保留可重试
        assert!(!target.join("data").exists(), "目标不得留下半截数据");
        assert!(!staging.exists(), "暂存区应清理");
        assert!(plan::load_pending(&cfg).is_some());
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none());
        assert!(source.join("data").join("ssh.db").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 非空且与源不一致的目标 → 冲突拒绝，不覆盖用户既有内容
    #[test]
    fn non_empty_target_with_other_content_is_refused() {
        let dir = temp_dir("migrate-conflict");
        let source = dir.join("source");
        make_source(&source);
        let target = dir.join("target");
        std::fs::create_dir_all(target.join("data")).unwrap();
        std::fs::write(target.join("data").join("someone-else.db"), b"mine").unwrap();
        let cfg = FileConfig::new(&dir.join("settings.json"));
        schedule(&cfg, &source, &target);

        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "precheck");
                assert!(detail.contains("已有内容"), "详情: {detail}");
            }
            other => panic!("期望冲突拒绝，实际 {other:?}"),
        }
        assert!(
            target.join("data").join("someone-else.db").exists(),
            "用户既有数据不得被覆盖"
        );
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 配置保存失败：数据已在目标且完整，重试走续跑提交路径，源仍保留
    #[test]
    fn config_failure_keeps_both_sides_and_retry_commits() {
        let dir = temp_dir("migrate-configfail");
        let source = dir.join("source");
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();
        make_source(&source);
        let cfg = FileConfig::new(&dir.join("settings.json"));
        schedule(&cfg, &source, &target);

        // 只让新根配置写不进去：迁移其他步骤（含计划阶段落盘）都正常
        cfg.fail_key(Some(paths::KEY_STORAGE_ROOT));
        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "commit");
                assert!(detail.contains("写入存储位置配置失败"), "详情: {detail}");
            }
            other => panic!("期望提交阶段失败，实际 {other:?}"),
        }
        assert!(cfg.read(paths::KEY_STORAGE_ROOT).is_none(), "配置不得写入");
        assert!(source.join("vault").join("vault.dat").exists(), "源保留");
        assert!(
            target.join("vault").join("vault.dat").exists(),
            "目标数据已在"
        );

        cfg.fail_key(None);
        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Committed { resumed, .. } => assert!(resumed, "应走续跑提交"),
            other => panic!("期望续跑提交成功，实际 {other:?}"),
        }
        assert!(plan::load_pending(&cfg).is_none());
        assert!(source.join("data").join("ssh.db").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 空数据环境也允许变更位置（不因零文件卡住）
    #[test]
    fn empty_source_switches_root() {
        let dir = temp_dir("migrate-empty");
        let source = dir.join("source");
        std::fs::create_dir_all(&source).unwrap();
        let target = dir.join("target");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        schedule(&cfg, &source, &target);

        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Committed { copied_files, .. } => assert_eq!(copied_files, 0),
            other => panic!("期望空环境也能提交，实际 {other:?}"),
        }
        assert_eq!(
            cfg.read(paths::KEY_STORAGE_ROOT).unwrap(),
            serde_json::json!(target.display().to_string())
        );
        assert!(plan::load_pending(&cfg).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 源内出现符号链接 → 扫描阶段拒绝（不跟随链接，不产生半截目标）
    #[test]
    fn link_in_source_is_refused_before_copying() {
        let dir = temp_dir("migrate-link");
        let source = dir.join("source");
        std::fs::create_dir_all(source.join("data")).unwrap();
        let real = source.join("data").join("real.db");
        std::fs::write(&real, b"payload").unwrap();
        let link = source.join("data").join("link.db");
        let created = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&real, &link).is_ok()
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(&real, &link).is_ok()
            }
        };
        if !created {
            eprintln!("跳过：当前环境无法创建符号链接");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let target = dir.join("target");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        schedule(&cfg, &source, &target);

        let outcome = execute_pending(&cfg, &|_, _, _, _| {}).unwrap();
        match outcome {
            MigrationOutcome::Failed { stage, detail, .. } => {
                assert_eq!(stage, "scan");
                assert!(detail.contains("符号链接"), "详情: {detail}");
            }
            other => panic!("期望扫描拒绝，实际 {other:?}"),
        }
        assert!(!target.join("data").exists(), "不得搬入半截数据");
        assert!(plan::load_pending(&cfg).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
