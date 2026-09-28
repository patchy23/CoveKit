//! 数据根维护：关闭资源、排空实际执行者后切换同一空间，应用进程保持运行。

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use super::plan::ConfigStore;
use super::{access, migrate, plan, recovery, RecoveryActionResult};
use crate::framework::{context, lifecycle, paths, space};

/// 同一空间在应用内完成根切换后的结果。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrateResult {
    /// 已校验并生效的数据根绝对路径。
    root: String,
}

enum Action {
    Migrate(Option<String>),
    Recover {
        action: String,
        target: Option<String>,
    },
}

/// 前端已完成未保存内容协商；后端仍独立裁决并等待所有旧根执行者退出。
#[tauri::command]
pub async fn storage_migrate_now(
    app: AppHandle,
    target: Option<String>,
) -> Result<StorageMigrateResult, String> {
    run(app, Action::Migrate(target)).await
}

/// 将恢复页动作交给同一维护协调入口，成功后无需重启进程。
pub(super) async fn recover(
    app: AppHandle,
    action: String,
    target: Option<String>,
) -> Result<RecoveryActionResult, String> {
    let result = run(app, Action::Recover { action, target }).await?;
    Ok(RecoveryActionResult {
        ok: true,
        restart_required: false,
        message: format!("数据目录已切换到 {}，无需重启应用。", result.root),
    })
}

async fn run(app: AppHandle, action: Action) -> Result<StorageMigrateResult, String> {
    // 维护任务自身持有冻结权；等待者被丢弃也不能让复制期间重新开放业务写入。
    tauri::async_runtime::spawn(async move {
        let freeze = access::freeze()?;
        let cleanup_app = app.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let preparation =
                lifecycle::prepare_close(&cleanup_app, lifecycle::CloseReason::Restart);
            if !preparation.proceed {
                return Err(preparation.blockers.join("；"));
            }
            let outcome = lifecycle::dispose_for_storage(&cleanup_app);
            if outcome.timed_out || !outcome.failures.is_empty() {
                return Err(format!(
                    "工具资源尚未释放，目录未切换：{}",
                    outcome.failures.join("；")
                ));
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("关闭工具资源失败：{error}"))?;
        outcome?;
        // 不能提前持维护锁：旧导入可能仍持操作租约等待同一把锁。
        freeze.wait_idle(Duration::from_secs(30)).await?;
        let maintenance = context::maintenance_guard().await;
        tokio::task::spawn_blocking(move || {
            let _freeze = freeze;
            let _maintenance = maintenance;
            lifecycle::reset_storage(&app)?;
            let root = execute(&app, action)?;
            log::info!("数据目录维护完成，应用内切换已生效");
            Ok(StorageMigrateResult {
                root: root.display().to_string(),
            })
        })
        .await
        .map_err(|error| format!("数据目录维护失败：{error}"))?
    })
    .await
    .map_err(|error| format!("数据目录维护任务失败：{error}"))?
}

fn execute(app: &AppHandle, action: Action) -> Result<PathBuf, String> {
    let cfg = plan::AppConfig(app);
    let space_id = space::resolve_active(app)?.space_id;
    let previous = paths::configured_root(app).unwrap_or_default();
    let previous_recovery = recovery::current();
    let mut pending = plan::load_pending(&cfg);
    let result = (|| {
        let target = match action {
            Action::Migrate(target) => {
                if let Some(target) = target {
                    super::schedule_migration(app, &target)?;
                    pending = plan::load_pending(&cfg);
                }
                execute_pending(app, &cfg)?
            }
            Action::Recover { action, target } => match action.as_str() {
                "retry" if plan::load_pending(&cfg).is_some() => execute_pending(app, &cfg)?,
                "retry" => paths::resolve_root(&previous, &paths::default_root(app)?),
                "choose" | "use-default" => {
                    if plan::load_pending(&cfg).is_some() {
                        return Err("已有待执行的迁移计划，请先取消计划或重试迁移".into());
                    }
                    if action == "use-default" {
                        paths::default_root(app)?
                    } else {
                        let target = target
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .ok_or("请选择包含当前数据空间的原有数据根目录")?;
                        PathBuf::from(target)
                    }
                }
                _ => return Err("未知的存储恢复动作".into()),
            },
        };
        recovery::validate_existing_root(&target, &space_id)?;
        if !paths::is_writable_dir(&target) {
            return Err("所选数据目录不可写，请检查磁盘和权限".into());
        }
        paths::set_storage_root(app, &target.display().to_string())?;
        context::replace_location(context::StorageLocation::for_space(&target, &space_id))?;
        recovery::clear();
        paths::grant_asset_scope(app);
        Ok(target)
    })();
    match result {
        Ok(target) => Ok(target),
        Err(mut error) => {
            // 复制提交成功后仍可能在激活前失败；磁盘配置与进程上下文必须一起回到旧根。
            if let Err(rollback) = restore_configuration(&cfg, &previous, pending.as_ref()) {
                error = format!("{error}；{rollback}");
                recovery::set(recovery::StorageRecovery::migration_failed(
                    &previous,
                    &previous,
                    pending.as_ref().map(|plan| plan.id.clone()),
                    error.clone(),
                ));
                return Err(error);
            }
            if let Some(state) = previous_recovery {
                recovery::set(state);
            }
            Err(error)
        }
    }
}

/// 激活失败只回退配置，保留已经校验的副本；失败计划的最新阶段与错误不能被旧快照覆盖。
fn restore_configuration(
    cfg: &dyn ConfigStore,
    previous: &str,
    pending: Option<&plan::PendingPlan>,
) -> Result<(), String> {
    let current = cfg
        .read(paths::KEY_STORAGE_ROOT)
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    if current != previous {
        cfg.write(paths::KEY_STORAGE_ROOT, serde_json::json!(previous))
            .map_err(|error| format!("恢复原目录配置失败：{error}"))?;
    }
    if let Some(pending) = pending.filter(|_| plan::load_pending(cfg).is_none()) {
        plan::save_pending(cfg, pending).map_err(|error| format!("恢复迁移计划失败：{error}"))?;
    }
    Ok(())
}

fn execute_pending(app: &AppHandle, cfg: &dyn plan::ConfigStore) -> Result<PathBuf, String> {
    if let (Some(pending), Some(root)) = (plan::load_pending(cfg), context::root()) {
        if pending.source_path() != root && pending.target_path() != root {
            return Err("迁移计划不属于当前数据目录，请取消旧计划后重新迁移".into());
        }
    }
    match migrate::execute_pending(cfg, &|phase, files, bytes, total| {
        super::emit_progress(app, phase, files, bytes, total);
    })? {
        migrate::MigrationOutcome::Committed { target, .. } => Ok(PathBuf::from(target)),
        migrate::MigrationOutcome::Failed { detail, .. } => Err(detail),
        migrate::MigrationOutcome::NoPlan => Err("没有待执行的迁移计划".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{temp_dir, FileConfig};
    use super::*;

    #[test]
    fn activation_failure_restores_old_root_and_resumable_plan() {
        let dir = temp_dir("activation-rollback");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        let pending = plan::PendingPlan::new(&dir.join("source"), &dir.join("target"));
        cfg.write(paths::KEY_STORAGE_ROOT, serde_json::json!(pending.target))
            .unwrap();
        restore_configuration(&cfg, &pending.source, Some(&pending)).unwrap();
        assert_eq!(
            cfg.read(paths::KEY_STORAGE_ROOT).unwrap(),
            serde_json::json!(pending.source)
        );
        assert_eq!(plan::load_pending(&cfg).unwrap(), pending);
        let mut latest = pending.clone();
        latest.last_error = Some("复制失败".into());
        latest.attempts = 3;
        plan::save_pending(&cfg, &latest).unwrap();
        restore_configuration(&cfg, &pending.source, Some(&pending)).unwrap();
        assert_eq!(plan::load_pending(&cfg).unwrap(), latest);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rollback_write_failure_is_reported() {
        let dir = temp_dir("activation-rollback-fail");
        let cfg = FileConfig::new(&dir.join("settings.json"));
        cfg.write(paths::KEY_STORAGE_ROOT, serde_json::json!("target"))
            .unwrap();
        cfg.fail_key(Some(paths::KEY_STORAGE_ROOT));
        assert!(restore_configuration(&cfg, "source", None)
            .unwrap_err()
            .contains("恢复原目录配置失败"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
