//! 框架 · 旧布局集合迁移（可靠性 T03）
//!
//! 旧版本把数据直接放在存储根下，现布局分四分区（`data` / `vault` / `logs` / `cache`）。
//! 本模块负责把根下的旧文件搬进分区，规则（任务书 T03）：
//!
//! 1. **按集合迁移**：密文与其主密钥、数据库与其 `-wal` / `-shm` / `-journal` 属于同一集合，
//!    要么全部到位，要么全部保留原位（缺一半会造成「能解密但读不到数据」或反之）。
//! 2. **暂存后提交**：先复制到临时名并校验（字节数 + SHA-256），校验通过才改名进分区；
//!    复制中断不会在分区里留下半截文件，重试也不会因为「目标存在」而跳过。
//! 3. **目标存在 ≠ 目标完整**：新位置已有同名文件时做内容比对 —— 一致则清理旧位置；
//!    不一致（半截/损坏）把新位置改名留档为 `.incomplete-<时间戳>`，再把旧位置搬入，**绝不静默覆盖**。
//! 4. **版本只在全部必需项完成后推进**：任一项失败就保持原版本，下次启动继续修复（修复遍）。
//!    因此 `layoutVersion` 达到当前值时也仍然检查旧位置残留，不会把「已推进版本」当「无需修复」。

use std::path::Path;

use crate::framework::paths;

use engine::migrate_member;
use groups::{db_group_members, dynamic_db_groups, leftovers, Group, ItemKind, GROUPS};

pub(crate) use report::{ItemOutcome, ItemState, LayoutReport};

/// 旧布局迁移的纯实现（不需要 AppHandle，便于用临时目录单测）。
///
/// 语义：按集合处理，集合内任一成员失败即整组保留原位（已提交的成员回滚）。
pub fn migrate_layout_at(root: &Path) -> Result<LayoutReport, String> {
    let token = paths::now_stamp();
    let mut report = LayoutReport::default();

    // 1) 固定集合
    for group in GROUPS {
        let names = group_member_names(root, &group);
        let existing: Vec<String> = names
            .iter()
            .filter(|name| root.join(name).exists())
            .cloned()
            .collect();
        if existing.is_empty() {
            for name in &names {
                report.outcomes.push(ItemOutcome {
                    group: group.id,
                    name: name.clone(),
                    target: format!("{}/{name}", group.partition_of(name)),
                    state: ItemState::Missing,
                    detail: String::new(),
                });
            }
            continue;
        }
        let mut group_outcomes = Vec::new();
        for name in &names {
            let partition = group.partition_of(name);
            let kind = group.kind_of(name);
            group_outcomes.push(migrate_member(
                root, group.id, name, partition, &token, kind,
            ));
        }
        let failed = group_outcomes.iter().any(|o| o.state == ItemState::Failed);
        if failed {
            // 集合回滚：把已完成搬移的成员退回旧位置，保证「要么全到位要么全原位」
            for outcome in &group_outcomes {
                if outcome.state == ItemState::Migrated {
                    let to = root.join(&outcome.target);
                    let from = root.join(&outcome.name);
                    if let Err(error) = std::fs::rename(&to, &from) {
                        eprintln!(
                            "[storage] 集合 {} 回滚 {} 失败（新位置数据仍在）: {error}",
                            group.id, outcome.target
                        );
                    }
                }
            }
        }
        report.outcomes.extend(group_outcomes);
    }

    // 2) 动态数据库集合
    for db_name in dynamic_db_groups(root)? {
        let names = db_group_members(root, &db_name);
        let mut group_outcomes = Vec::new();
        for name in &names {
            group_outcomes.push(migrate_member(
                root,
                "plugin-db",
                name,
                "data",
                &token,
                ItemKind::File,
            ));
        }
        if group_outcomes.iter().any(|o| o.state == ItemState::Failed) {
            for outcome in &group_outcomes {
                if outcome.state == ItemState::Migrated {
                    let to = root.join(&outcome.target);
                    let from = root.join(&outcome.name);
                    if let Err(error) = std::fs::rename(&to, &from) {
                        eprintln!(
                            "[storage] 数据库集合 {db_name} 回滚 {} 失败: {error}",
                            outcome.target
                        );
                    }
                }
            }
        }
        report.outcomes.extend(group_outcomes);
    }

    Ok(report)
}
/// 集合成员名（数据库会把 `-wal` / `-shm` / `-journal` 一并纳入）
fn group_member_names(root: &Path, group: &Group) -> Vec<String> {
    let mut names = Vec::new();
    for member in group.members {
        if member.kind == ItemKind::Dir {
            names.push(member.name.to_string());
            continue;
        }
        if member.name.to_ascii_lowercase().ends_with(".db") {
            names.extend(db_group_members(root, member.name));
        } else {
            names.push(member.name.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// 旧布局迁移（含版本门槛与修复遍）。
///
/// - 版本已达当前值且旧位置无残留：直接返回空报告（不触盘）。
/// - 版本已达当前值但仍有残留（历史失败或版本被误推进）：仍然执行修复遍。
/// - **只有全部集合无失败时才推进版本**，失败保持原版本，下次启动继续。
pub fn migrate_layout(app: &tauri::AppHandle) -> Result<LayoutReport, String> {
    let root = paths::storage_root(app)?;
    let current = paths::layout_version(app);
    let pending = leftovers(&root)?;
    if current >= paths::LAYOUT_VERSION && pending.is_empty() {
        return Ok(LayoutReport {
            outcomes: Vec::new(),
            version_advanced: false,
        });
    }

    let mut report = migrate_layout_at(&root)?;
    if report.has_failures() {
        eprintln!("[storage] {}", report.summary());
        return Ok(report);
    }

    if current < paths::LAYOUT_VERSION {
        paths::write_setting(
            app,
            paths::KEY_LAYOUT_VERSION,
            serde_json::json!(paths::LAYOUT_VERSION),
        )?;
        report.version_advanced = true;
    }
    eprintln!("[storage] {}", report.summary());
    Ok(report)
}

mod engine;
mod groups;
mod report;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// 建立本测试专用临时目录
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchybox-layout-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 写入一个测试文件
    fn write(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// 生成一份真实 SQLite 数据库，便于验证「集合整体搬移」在真实文件上的行为
    fn make_db(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO t (v) VALUES ('x')", []).unwrap();
    }

    /// 旧位置项搬入分区并清理旧位置
    #[test]
    fn moves_legacy_items_into_partitions_and_clears_source() {
        let root = temp_root("moves");
        write(&root.join("credentials-master.key"), &[7u8; 32]);
        write(&root.join("credentials").join("database.enc"), b"cipher");
        write(&root.join("ssh-known-hosts"), b"host keys");
        write(&root.join("vault.dat"), b"vault");
        write(&root.join("vault-master.key"), &[9u8; 32]);
        make_db(&root.join("ssh.db"));
        write(&root.join("ssh.db-wal"), b"wal-page");

        let report = migrate_layout_at(&root).unwrap();
        assert!(
            !report.has_failures(),
            "不应有失败项: {:?}",
            report.failures()
        );
        assert!(root.join("data/credentials-master.key").exists());
        assert!(root.join("data/credentials/database.enc").exists());
        assert!(root.join("data/ssh.db").exists());
        assert!(
            root.join("data/ssh.db-wal").exists(),
            "数据库伴随文件必须同集合搬移"
        );
        assert!(root.join("data/ssh-known-hosts").exists());
        assert!(root.join("vault/vault.dat").exists());
        assert!(root.join("vault/vault-master.key").exists());
        // 旧位置已清空
        assert!(!root.join("credentials-master.key").exists());
        assert!(!root.join("ssh.db").exists());
        assert!(!root.join("vault.dat").exists());
        assert!(leftovers(&root).unwrap().is_empty());
    }

    /// 新位置已有相同内容：清理旧位置，不重复搬，也不算失败
    #[test]
    fn identical_target_clears_legacy_without_conflict() {
        let root = temp_root("identical");
        write(&root.join("vault.dat"), b"same-bytes");
        write(&root.join("vault/vault.dat"), b"same-bytes");

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(!root.join("vault.dat").exists(), "内容一致时旧位置应被清理");
        assert!(root.join("vault/vault.dat").exists());
        let state = report
            .outcomes
            .iter()
            .find(|o| o.name == "vault.dat")
            .map(|o| o.state)
            .unwrap();
        assert_eq!(state, ItemState::AlreadyCurrent);
    }

    /// 新位置内容不一致（半截）：留档并改用旧位置数据，绝不静默覆盖
    #[test]
    fn divergent_target_is_archived_and_legacy_wins() {
        let root = temp_root("divergent");
        write(&root.join("vault.dat"), b"complete-legacy-data");
        write(&root.join("vault/vault.dat"), b"half");

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures(), "{:?}", report.failures());
        assert_eq!(
            std::fs::read(root.join("vault/vault.dat")).unwrap(),
            b"complete-legacy-data"
        );
        let archived: Vec<_> = std::fs::read_dir(root.join("vault"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".incomplete-"))
            .collect();
        assert_eq!(archived.len(), 1, "半截目标必须留档: {archived:?}");
        assert!(!root.join("vault.dat").exists());
    }

    /// 集合内任一成员失败：整组保持原位（已搬移的成员回滚），版本不推进
    #[test]
    fn group_failure_rolls_back_whole_set() {
        let root = temp_root("group-rollback");
        make_db(&root.join("app.db"));
        // app.db-wal 故意做成目录：按文件复制必然失败，用来验证集合整体回滚
        std::fs::create_dir_all(root.join("app.db-wal")).unwrap();

        let report = migrate_layout_at(&root).unwrap();
        assert!(report.has_failures(), "成员失败必须报失败: {report:?}");
        assert!(
            root.join("app.db").exists(),
            "失败集合必须整体回滚，数据库留在旧位置"
        );
        assert!(root.join("app.db-wal").is_dir());
        assert!(!root.join("data/app.db").exists(), "回滚后新位置不得残留");
        let strays: Vec<String> = std::fs::read_dir(root.join("data"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        assert!(strays.is_empty(), "暂存残留必须清理: {strays:?}");
    }

    /// 密文与主密钥同集合：密钥缺失不影响密文搬迁，但两者都在时一起走
    #[test]
    fn ciphertext_and_key_move_together() {
        let root = temp_root("pair");
        write(&root.join("db-secrets.enc"), b"cipher");
        write(&root.join("db-master.key"), &[3u8; 32]);

        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(root.join("data/db-secrets.enc").exists());
        assert!(root.join("data/db-master.key").exists());
        assert!(!root.join("db-secrets.enc").exists());
        assert!(!root.join("db-master.key").exists());
    }

    /// 修复遍：版本已达标但旧位置仍有残留（历史失败）→ 仍然修
    #[test]
    fn repair_pass_detects_leftovers_even_when_version_current() {
        let root = temp_root("repair");
        std::fs::create_dir_all(root.join("data")).unwrap();
        write(&root.join("db-master.key"), &[5u8; 32]);

        assert_eq!(leftovers(&root).unwrap(), vec!["db-master.key".to_string()]);
        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert!(root.join("data/db-master.key").exists());
        assert!(leftovers(&root).unwrap().is_empty(), "修复后旧位置应无残留");
    }

    /// 符号链接必须被拒绝并指出路径（不跟随）
    #[cfg(windows)]
    #[test]
    fn symlink_member_is_refused_with_path() {
        let root = temp_root("symlink");
        let target = root.join("real-tree");
        write(&target.join("inner.txt"), b"data");
        let link = root.join("agents");
        if std::os::windows::fs::symlink_dir(&target, &link).is_err() {
            return; // 无创建符号链接权限（未开发者模式）时跳过
        }

        let report = migrate_layout_at(&root).unwrap();
        let failed = report.failures();
        assert_eq!(failed.len(), 1);
        assert!(
            failed[0].detail.contains("符号链接"),
            "{}",
            failed[0].detail
        );
        assert!(link.exists(), "失败的项保留原位");
    }

    /// 空根目录：无项可搬，也不算失败（允许空数据环境迁移位置）
    #[test]
    fn empty_root_is_noop_without_failures() {
        let root = temp_root("empty");
        let report = migrate_layout_at(&root).unwrap();
        assert!(!report.has_failures());
        assert_eq!(report.moved(), 0);
        assert!(leftovers(&root).unwrap().is_empty());
    }
}
