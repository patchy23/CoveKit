//! 框架 · 空间自举（首装建空间 + 启动完整性自检）
//!
//! 规则（2026-09-16 裁决：不做兼容层与数据迁移层，旧数据直接放弃）：
//! - **只有三种情形**：已是空间化布局（空操作）/ 自举缺失或非法（全新安装：生成 uid、
//!   登记索引、建目录）/ 自举指向的空间目录不存在（拒绝启动，现场保留，不新建空环境冒充）；
//! - 设备根下的旧扁平内容（`data/`、`vault/`、`preferences.json`）与任何非 uid 形态
//!   **不再读取、不再搬迁**：留在原位由用户自行处理，应用一律当作不存在；
//! - 暂存目录残留（导入中断的半成品）每次启动清理，这不是兼容逻辑，是崩溃现场清扫；
//! - 失败即 `Err`（调用方登记恢复状态），不猜、不静默继续。

use std::path::Path;

use tauri::AppHandle;

use super::index;
use super::is_valid_space_id;

/// 自举报告（仅含可诊断事实，不含秘密）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceBootstrapReport {
    /// 处置结果（已是空间化布局 / 全新安装）
    pub outcome: &'static str,
    /// 全新安装时生成的默认空间 uid（否则空串）
    pub new_space_id: String,
    /// 被清理的暂存目录名列表
    pub staging_removed: Vec<String>,
}

impl SpaceBootstrapReport {
    /// 一行摘要（启动日志用）
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("空间自举: {}", self.outcome)];
        if !self.new_space_id.is_empty() {
            parts.push(format!("uid={}", self.new_space_id));
        }
        if !self.staging_removed.is_empty() {
            parts.push(format!("清理暂存目录 {} 个", self.staging_removed.len()));
        }
        parts.join("；")
    }
}

/// 自举动作（纯判定结果，不写盘）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapAction {
    /// 已是空间化布局：空操作
    Current,
    /// 全新安装：生成 uid、登记索引与自举、建空间目录
    FreshInstall,
}

/// 纯判定：自举值 + 目录现状 → 动作（不写盘，便于单测覆盖三种情形）
///
/// 旧扁平内容与否不影响判定（放弃旧数据：不读、不搬、不删）。
pub(crate) fn plan(root: &Path, active: Option<&str>) -> Result<BootstrapAction, String> {
    let active_id = active.map(str::trim).filter(|text| !text.is_empty());
    if let Some(id) = active_id {
        if is_valid_space_id(id) {
            // 已是空间化布局：自举是合法 uid 且目录存在
            if index::space_root(root, id).is_dir() {
                return Ok(BootstrapAction::Current);
            }
            // 自举指向的目录被外部删了：拒绝启动，不新建空环境冒充「数据还在」
            return Err(format!(
                "自举配置指向的空间目录不存在：spaces/{id}（拒绝启动，现场保留；请检查存储目录或被外部删除的空间目录）"
            ));
        }
    }
    Ok(BootstrapAction::FreshInstall)
}

/// 空间自举入口（启动维护窗口调用；幂等）。
///
/// 顺序：清暂存残留 → 纯判定 → 处置。失败即 `Err`（调用方登记恢复状态）。
pub fn ensure_space(app: &AppHandle, root: &Path) -> Result<SpaceBootstrapReport, String> {
    let staging_removed = index::cleanup_staging(root)?;
    match plan(root, index::read_active_id(app).as_deref())? {
        BootstrapAction::Current => Ok(SpaceBootstrapReport {
            outcome: "已是空间化布局",
            new_space_id: String::new(),
            staging_removed,
        }),
        BootstrapAction::FreshInstall => {
            // 旧扁平内容与旧承载位一律不读不搬（放弃旧数据，由用户自行清理）
            let uid = uuid::Uuid::new_v4().to_string();
            register_space(app, root, &uid)?;
            super::record_active(app, &uid)?;
            Ok(SpaceBootstrapReport {
                outcome: "全新安装：已生成默认空间 uid",
                new_space_id: uid,
                staging_removed,
            })
        }
    }
}

/// 登记默认空间：写入 uid 条目（is_default 唯一），并建目录
fn register_space(app: &AppHandle, root: &Path, uid: &str) -> Result<(), String> {
    let mut space_index = index::read_index(app)?;
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
mod tests {
    use super::*;

    /// 建立本用例专用临时目录（避免并行用例互相干扰）
    fn temp_root(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchybox-space-bootstrap-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 空根 + 无自举 → 全新安装
    #[test]
    fn plan_fresh_install_on_empty_root() {
        let root = temp_root("fresh");
        assert_eq!(
            plan(&root, None).expect("判定成功"),
            BootstrapAction::FreshInstall
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 合法 uid 自举 + 目录存在 → 空操作
    #[test]
    fn plan_current_when_space_dir_exists() {
        let root = temp_root("current");
        let uid = uuid::Uuid::new_v4().to_string();
        std::fs::create_dir_all(index::space_root(&root, &uid)).expect("建空间目录");
        assert_eq!(
            plan(&root, Some(&uid)).expect("判定成功"),
            BootstrapAction::Current
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 合法 uid 自举但目录不存在 → 拒绝启动（不新建空环境冒充）
    #[test]
    fn plan_broken_active_refuses() {
        let root = temp_root("broken");
        let ghost = uuid::Uuid::new_v4().to_string();
        let error = plan(&root, Some(&ghost)).expect_err("目录缺失必须拒绝");
        assert!(error.contains("拒绝启动"), "错误文案: {error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 旧扁平内容在场不影响判定（放弃旧数据：不读、不搬、不删）；
    /// 非法自举值（旧字面量承载位）按全新安装处理
    #[test]
    fn plan_ignores_legacy_content_and_legacy_active() {
        let root = temp_root("legacy");
        std::fs::create_dir_all(root.join("data")).expect("造旧布局");
        std::fs::write(root.join("preferences.json"), b"{}").expect("造旧偏好");

        assert_eq!(
            plan(&root, Some("default")).expect("旧承载位不卡住启动"),
            BootstrapAction::FreshInstall
        );
        assert_eq!(
            plan(&root, None).expect("判定成功"),
            BootstrapAction::FreshInstall
        );
        // 旧内容原样保留（放弃 ≠ 删除；用户自行处理）
        assert!(root.join("data").is_dir());
        assert!(root.join("preferences.json").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
