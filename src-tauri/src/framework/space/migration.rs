//! 框架 · 空间化迁移（一次性升级路径：旧扁平布局与字面量承载位 → `spaces/<uid>/`）
//!
//! 规则（2026-09-16 裁决：不做兼容、数据本次就搬、不留尾巴）：
//! - **一次跑完**：旧扁平布局（`<设备根>/{data,vault,preferences.json}`）在启动维护窗口内
//!   迁移为 `spaces/<uid>/{data,vault,preferences.json}`，此后正常路径没有任何旧布局分支；
//! - **fail-fast**：任一步失败即报错（启动登记恢复状态），现场保留原位，不猜、不静默继续；
//! - **密钥同步搬家**：主密钥从旧 keyring service 读出 → 写新 service → 回读校验 → 删旧；
//!   密钥库原生后端不可用时跳过（降级密钥文件随目录搬迁，不丢）；
//! - **代际目录平铺**：空间化之前的 `spaces/<id>/generations/1/` 一并平铺为 `spaces/<id>/`；
//!   出现无法识别的代际目录（非 `1` 或有多个）→ fail-fast，不猜测取舍；
//! - **幂等**：已完成空间化的安装再次执行 = 空操作；崩溃中途重启可续跑（搬移按项校验）。

use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::framework::secure_store::{
    keyring_service, MasterKeyStore, ScopedKeyringStore, CREDENTIALS_KEY_SPEC, KEYRING_SERVICE,
    VAULT_KEY_SPEC,
};

use super::index;
use super::is_valid_space_id;

/// 迁移决策（纯判定，便于单测覆盖四种情形）
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MigrationPlan {
    /// 全新安装：生成 uid、登记索引与自举、建空间目录
    FreshInstall,
    /// 旧扁平布局：生成 uid、搬数据与密钥、登记；`rewrite_active` = 自举需改写为 uid
    LegacyDefault {
        /// 自举值缺失/为旧承载位/非法时为 true（合法 uid 时保留用户当前选择）
        rewrite_active: bool,
    },
    /// 已完成空间化（代际目录平铺单独检查，与本状态无关）
    AlreadyCurrent,
}

/// 迁移报告（仅含可诊断事实，不含秘密）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceMigrationReport {
    /// 默认空间的处置结果（FreshInstall / LegacyDefault 时为新生成的 uid）
    pub outcome: &'static str,
    /// 新生成的默认空间 uid（无则空串）
    pub new_space_id: String,
    /// 被平铺的空间 id 列表（`generations/1` → 空间根）
    pub flattened: Vec<String>,
    /// 被清理的暂存目录名列表
    pub staging_removed: Vec<String>,
    /// 搬家的主密钥条数（密钥库不可用时为 0，属正常路径）
    pub keys_moved: usize,
}

impl SpaceMigrationReport {
    /// 一行摘要（启动日志用）
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("空间化迁移: {}", self.outcome)];
        if !self.new_space_id.is_empty() {
            parts.push(format!("uid={}", self.new_space_id));
        }
        if !self.flattened.is_empty() {
            parts.push(format!("平铺 {} 个空间", self.flattened.len()));
        }
        if !self.staging_removed.is_empty() {
            parts.push(format!("清理暂存目录 {} 个", self.staging_removed.len()));
        }
        if self.keys_moved > 0 {
            parts.push(format!("主密钥搬家 {} 条", self.keys_moved));
        }
        parts.join("；")
    }
}

/// 旧扁平布局的空间级内容（相对设备根）：数据分区、凭证分区、空间级偏好文件
const LEGACY_CONTENT: [&str; 3] = ["data", "vault", "preferences.json"];

/// 设备根是否存在旧扁平布局的空间级内容（任一存在即视为旧安装）
fn legacy_content_present(root: &Path) -> bool {
    LEGACY_CONTENT.iter().any(|name| root.join(name).exists())
}

/// 迁移决策：给定自举值与目录现状，判定要执行的动作（纯判定 + 只读目录探测）
///
/// 四种情形：
/// - 自举是合法 uid 且空间目录存在 → `AlreadyCurrent`；
/// - 自举是合法 uid 但目录不存在 → `Broken`（拒绝继续，不新建空环境冒充）；
/// - 设备根有旧布局内容 → `LegacyDefault`（自举为合法 uid 时保留它，否则改写为 uid）；
/// - 其余 → `FreshInstall`。
pub(crate) fn decide(root: &Path, active: Option<&str>) -> Result<MigrationPlan, String> {
    let active_id = active.map(str::trim).filter(|text| !text.is_empty());
    if let Some(id) = active_id {
        if is_valid_space_id(id) {
            return if index::space_root(root, id).is_dir() {
                if legacy_content_present(root) {
                    // 用户已切到导入空间，但默认空间的旧内容还在设备根：仍要入位，但不动自举
                    Ok(MigrationPlan::LegacyDefault {
                        rewrite_active: false,
                    })
                } else {
                    Ok(MigrationPlan::AlreadyCurrent)
                }
            } else if legacy_content_present(root) {
                // 旧内容还在但自举已指向别处（例如切换后目录被外部删了）：旧内容仍要保住
                Ok(MigrationPlan::LegacyDefault {
                    rewrite_active: false,
                })
            } else {
                Err(format!(
                    "自举配置指向的空间目录不存在：spaces/{id}（拒绝启动，现场保留；请检查存储目录或被外部删除的空间目录）"
                ))
            };
        }
    }
    if legacy_content_present(root) {
        return Ok(MigrationPlan::LegacyDefault {
            rewrite_active: true,
        });
    }
    Ok(MigrationPlan::FreshInstall)
}

/// 密钥搬家存储抽象（生产 = 系统密钥库条目；测试 = 内存表）
pub(crate) trait KeyMover {
    /// 读取条目（`Ok(None)` = 没有记录）
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String>;
    /// 写入条目
    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String>;
    /// 删除条目（条目不存在不算失败）
    fn delete(&self, account: &str) -> Result<(), String>;
}

impl KeyMover for ScopedKeyringStore {
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        MasterKeyStore::read(self, account)
    }

    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String> {
        MasterKeyStore::write(self, account, key)
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        ScopedKeyringStore::delete(self, account)
    }
}

/// 主密钥搬家：读旧 → 写新 → 回读校验 → 删旧。
///
/// 任一步失败即报错（此时文件系统尚未动，现场完整）；删旧失败只告警——旧条目从此无人读取，
/// 属惰性残留，可手工清理，不应因此挡住启动。
pub(crate) fn move_master_keys(
    old_store: &dyn KeyMover,
    new_store: &dyn KeyMover,
) -> Result<usize, String> {
    let mut moved = 0;
    for account in [VAULT_KEY_SPEC.account, CREDENTIALS_KEY_SPEC.account] {
        let Some(key) = old_store.read(account)? else {
            continue;
        };
        new_store.write(account, &key)?;
        match new_store.read(account)? {
            Some(readback) if readback == key => {}
            _ => {
                return Err(format!(
                    "主密钥搬家回读校验失败（account={account}）：新条目与旧值不一致，迁移中止"
                ));
            }
        }
        if let Err(error) = old_store.delete(account) {
            eprintln!("[space] 旧密钥库条目删除失败（不影响使用，可手工清理）: {error}");
        }
        moved += 1;
    }
    Ok(moved)
}

/// 把 `spaces/<id>/generations/1/` 的内容平铺到 `spaces/<id>/`（一次性，崩溃可续跑）。
///
/// 只认「generations 目录下有且仅有 `1`」这一种历史形态；其余形态（多个代际目录、
/// 非数字名）一律 fail-fast，不猜测取舍。返回被平铺的空间 id 列表。
pub(crate) fn flatten_generation_dirs(root: &Path) -> Result<Vec<String>, String> {
    let spaces = index::spaces_dir(root);
    let entries = match std::fs::read_dir(&spaces) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("读取空间目录集失败: {error}")),
    };
    let mut flattened = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取空间目录项失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let space_dir = entry.path();
        if !space_dir.is_dir() || name.starts_with(index::STAGING_PREFIX) {
            continue;
        }
        let generations = space_dir.join("generations");
        if !generations.exists() {
            continue;
        }
        let subs: Vec<String> = std::fs::read_dir(&generations)
            .map_err(|e| format!("读取代际目录失败: {e}"))?
            .filter_map(|item| item.ok())
            .map(|item| item.file_name().to_string_lossy().to_string())
            .collect();
        if subs != ["1"] {
            return Err(format!(
                "空间 {name} 存在无法识别的代际目录（{subs:?}），拒绝平铺，现场保留"
            ));
        }
        let generation_one = generations.join("1");
        let mut children: Vec<PathBuf> = std::fs::read_dir(&generation_one)
            .map_err(|e| format!("读取代际内容目录失败: {e}"))?
            .filter_map(|item| item.ok())
            .map(|item| item.path())
            .collect();
        children.sort();
        for child in children {
            let child_name = child.file_name().unwrap_or_default();
            let target = space_dir.join(child_name);
            if target.exists() {
                return Err(format!(
                    "平铺空间 {name} 时目标已存在（{}），拒绝覆盖，现场保留",
                    target.display()
                ));
            }
            std::fs::rename(&child, &target)
                .map_err(|e| format!("平铺空间 {name} 的 {} 失败: {e}", child.display()))?;
        }
        std::fs::remove_dir(&generation_one)
            .and_then(|()| std::fs::remove_dir(&generations))
            .map_err(|e| format!("清理空间 {name} 的代际目录失败: {e}"))?;
        flattened.push(name);
    }
    flattened.sort();
    Ok(flattened)
}

/// 把设备根的旧扁平内容搬进 `spaces/<uid>/`（rename；目标已存在即报错，不覆盖）。
///
/// 逐项搬移，失败时把已搬项退回设备根（回滚），保证「要么全到位要么全原位」。
pub(crate) fn move_legacy_content(root: &Path, uid: &str) -> Result<(), String> {
    let space_dir = index::space_root(root, uid);
    if space_dir.exists() {
        return Err(format!(
            "空间目录 {} 已存在（uid 冲突或残留），拒绝迁移，现场保留",
            space_dir.display()
        ));
    }
    std::fs::create_dir_all(&space_dir)
        .map_err(|e| format!("创建空间目录 {} 失败: {e}", space_dir.display()))?;
    let mut moved: Vec<(&str, PathBuf)> = Vec::new();
    for name in LEGACY_CONTENT {
        let from = root.join(name);
        if !from.exists() {
            continue;
        }
        let target = space_dir.join(name);
        match std::fs::rename(&from, &target) {
            Ok(()) => moved.push((name, target)),
            Err(error) => {
                // 回滚已搬项，保持现场为旧布局
                for (moved_name, moved_target) in &moved {
                    if let Err(rollback) = std::fs::rename(moved_target, root.join(moved_name)) {
                        eprintln!(
                            "[space] 迁移回滚 {moved_name} 失败（新位置数据仍在）: {rollback}"
                        );
                    }
                }
                let _ = std::fs::remove_dir(&space_dir);
                return Err(format!(
                    "迁移 {name} 到空间目录失败: {error}（已回滚，数据在原位）"
                ));
            }
        }
    }
    Ok(())
}

/// 空间化迁移入口（启动维护窗口调用；幂等）。
///
/// 顺序：清暂存残留 → 平铺代际目录 → 判定并处置默认空间。失败即 `Err`（调用方登记恢复状态）。
pub fn migrate_to_spaces(app: &AppHandle, root: &Path) -> Result<SpaceMigrationReport, String> {
    let staging_removed = index::cleanup_staging(root)?;
    let flattened = flatten_generation_dirs(root)?;
    let active = index::read_active_id(app);
    let plan = decide(root, active.as_deref())?;
    match plan {
        MigrationPlan::AlreadyCurrent => Ok(SpaceMigrationReport {
            outcome: "已是空间化布局",
            new_space_id: String::new(),
            flattened,
            staging_removed,
            keys_moved: 0,
        }),

        MigrationPlan::FreshInstall => {
            let uid = uuid::Uuid::new_v4().to_string();
            register_space(app, root, &uid)?;
            super::record_active(app, &uid)?;
            Ok(SpaceMigrationReport {
                outcome: "全新安装：已生成默认空间 uid",
                new_space_id: uid,
                flattened,
                staging_removed,
                keys_moved: 0,
            })
        }
        MigrationPlan::LegacyDefault { rewrite_active } => {
            let uid = uuid::Uuid::new_v4().to_string();
            // 密钥先搬：失败时文件系统尚未动，现场完整
            let keys_moved = if crate::framework::secure_store::native_backend_available() {
                let old_store = ScopedKeyringStore::new(KEYRING_SERVICE);
                let new_store = ScopedKeyringStore::new(keyring_service(&uid));
                move_master_keys(&old_store, &new_store)?
            } else {
                eprintln!("[space] 密钥库原生后端不可用，跳过主密钥搬家（降级密钥文件随目录搬迁）");
                0
            };
            move_legacy_content(root, &uid)?;
            register_space(app, root, &uid)?;
            if rewrite_active {
                super::record_active(app, &uid)?;
            }
            Ok(SpaceMigrationReport {
                outcome: "旧扁平布局已迁入默认空间",
                new_space_id: uid,
                flattened,
                staging_removed,
                keys_moved,
            })
        }
    }
}

/// 登记默认空间：索引里清除旧承载位条目、写入 uid 条目（is_default 唯一），并建目录
fn register_space(app: &AppHandle, root: &Path, uid: &str) -> Result<(), String> {
    let mut space_index = index::read_index(app)?;
    space_index.remove("default");
    for record in space_index.values_mut() {
        record.is_default = false;
    }
    space_index.insert(
        uid.to_string(),
        index::SpaceRecord {
            name: String::new(),
            created_at: index::now_iso(),
            imported_from: None,
            is_default: true,
        },
    );
    index::write_all(app, &space_index)?;
    let space_dir = index::space_root(root, uid);
    std::fs::create_dir_all(space_dir.join("data"))
        .and_then(|()| std::fs::create_dir_all(space_dir.join("vault")))
        .map_err(|e| format!("创建默认空间目录 {} 失败: {e}", space_dir.display()))
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod migration_tests;
