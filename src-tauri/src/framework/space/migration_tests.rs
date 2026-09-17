//! `space::migration` 的独立用例（整体跑在 cfg(test) 下，不参与架构守卫判定）
//!
//! 覆盖：迁移决策四情形、代际目录平铺（含 fail-fast 与暂存目录豁免）、
//! 旧扁平内容搬移（含目标冲突拒绝与回滚）、主密钥搬家（内存表 fake）。

#[cfg(test)]
mod cases {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use super::super::{
        decide, flatten_generation_dirs, move_legacy_content, move_master_keys, KeyMover,
        MigrationPlan,
    };
    use crate::framework::space::index;
    use crate::framework::space::is_valid_space_id;

    const SPACE_A: &str = "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23";
    const SPACE_B: &str = "7c9d1a20-6b31-4e58-8f42-2a5c9e3d7b10";

    /// 建立本用例专用临时目录（避免并行用例互相干扰）
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchybox-space-migration-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 写入一个测试文件（自动建父目录）
    fn write(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("建父目录");
        }
        std::fs::write(path, content).expect("写文件");
    }

    // ── decide：四种情形 ──

    #[test]
    fn decide_fresh_install_on_empty_root() {
        let root = temp_root("decide-fresh");
        let plan = decide(&root, None).expect("空根判定成功");
        assert_eq!(plan, MigrationPlan::FreshInstall);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn decide_legacy_default_rewrites_missing_or_legacy_active() {
        let root = temp_root("decide-legacy");
        write(&root.join("data").join("ssh.db"), b"db");

        // 自举缺失 → 迁且改写
        assert_eq!(
            decide(&root, None).expect("判定成功"),
            MigrationPlan::LegacyDefault {
                rewrite_active: true
            }
        );
        // 自举是旧承载位 → 迁且改写
        assert_eq!(
            decide(&root, Some("default")).expect("判定成功"),
            MigrationPlan::LegacyDefault {
                rewrite_active: true
            }
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn decide_legacy_content_with_valid_active_keeps_user_choice() {
        let root = temp_root("decide-legacy-keep-active");
        write(&root.join("vault").join("vault.dat"), b"vault");
        // 用户已切到导入空间（目录存在）：默认空间内容仍要入位，但不动自举
        std::fs::create_dir_all(index::space_root(&root, SPACE_A)).expect("建空间目录");
        assert_eq!(
            decide(&root, Some(SPACE_A)).expect("判定成功"),
            MigrationPlan::LegacyDefault {
                rewrite_active: false
            }
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn decide_broken_when_active_points_to_missing_space() {
        let root = temp_root("decide-broken");
        let error = decide(&root, Some(SPACE_A)).expect_err("自举指向不存在目录必须报错");
        assert!(error.contains("空间目录不存在"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn decide_already_current_when_space_dir_exists() {
        let root = temp_root("decide-current");
        std::fs::create_dir_all(index::space_root(&root, SPACE_A)).expect("建空间目录");
        assert_eq!(
            decide(&root, Some(SPACE_A)).expect("判定成功"),
            MigrationPlan::AlreadyCurrent
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── 代际目录平铺 ──

    #[test]
    fn flatten_moves_generation_content_up() {
        let root = temp_root("flatten");
        write(
            &index::space_root(&root, SPACE_A)
                .join("generations/1/data")
                .join("ssh.db"),
            b"db",
        );
        write(
            &index::space_root(&root, SPACE_A)
                .join("generations/1")
                .join("preferences.json"),
            b"{}",
        );
        // 暂存目录不参与平铺
        std::fs::create_dir_all(
            index::spaces_dir(&root).join(".patchybox-staging-imp-x/generations/1"),
        )
        .expect("建暂存目录");

        let flattened = flatten_generation_dirs(&root).expect("平铺成功");
        assert_eq!(flattened, vec![SPACE_A.to_string()]);
        let space = index::space_root(&root, SPACE_A);
        assert!(space.join("data/ssh.db").exists());
        assert!(space.join("preferences.json").exists());
        assert!(!space.join("generations").exists(), "代际目录必须移除");
        // 暂存目录原样保留（由 cleanup_staging 负责，不归平铺管）
        assert!(index::spaces_dir(&root)
            .join(".patchybox-staging-imp-x/generations/1")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn flatten_refuses_unrecognized_generations() {
        let root = temp_root("flatten-refuse");
        std::fs::create_dir_all(index::space_root(&root, SPACE_A).join("generations/2"))
            .expect("建异常代际目录");
        let error = flatten_generation_dirs(&root).expect_err("无法识别的代际目录必须 fail-fast");
        assert!(error.contains("无法识别的代际目录"), "{error}");
        assert!(
            index::space_root(&root, SPACE_A)
                .join("generations/2")
                .exists(),
            "现场必须保留"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn flatten_refuses_to_overwrite_existing_target() {
        let root = temp_root("flatten-conflict");
        let space = index::space_root(&root, SPACE_A);
        write(&space.join("generations/1/data/x.txt"), b"new");
        write(&space.join("data"), b"existing"); // 目标已存在
        let error = flatten_generation_dirs(&root).expect_err("目标已存在必须拒绝");
        assert!(error.contains("拒绝覆盖"), "{error}");
        assert!(
            space.join("generations/1/data/x.txt").exists(),
            "现场必须保留"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── 旧扁平内容搬移 ──

    #[test]
    fn move_legacy_content_relocates_all_three_items() {
        let root = temp_root("move-legacy");
        write(&root.join("data/ssh.db"), b"db");
        write(&root.join("vault/vault.dat"), b"vault");
        write(&root.join("preferences.json"), b"{}");

        move_legacy_content(&root, SPACE_B).expect("搬移成功");
        let space = index::space_root(&root, SPACE_B);
        assert!(space.join("data/ssh.db").exists());
        assert!(space.join("vault/vault.dat").exists());
        assert!(space.join("preferences.json").exists());
        assert!(!root.join("data").exists(), "旧位置必须清空");
        assert!(!root.join("vault").exists(), "旧位置必须清空");
        assert!(!root.join("preferences.json").exists(), "旧位置必须清空");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn move_legacy_content_refuses_existing_space_dir() {
        let root = temp_root("move-conflict");
        write(&root.join("data/ssh.db"), b"db");
        std::fs::create_dir_all(index::space_root(&root, SPACE_B)).expect("建冲突目录");

        let error = move_legacy_content(&root, SPACE_B).expect_err("目标空间目录已存在必须拒绝");
        assert!(error.contains("拒绝迁移"), "{error}");
        assert!(root.join("data/ssh.db").exists(), "现场必须保留在原位");
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── 主密钥搬家（内存表 fake）──

    /// 内存表密钥库（可注入删除失败，验证「删旧失败只告警不中止」）
    struct MemKeys {
        entries: RefCell<HashMap<String, [u8; 32]>>,
        fail_delete: bool,
    }

    impl MemKeys {
        fn with(entries: &[(&str, [u8; 32])]) -> Self {
            Self {
                entries: RefCell::new(
                    entries
                        .iter()
                        .map(|(account, key)| ((*account).to_string(), *key))
                        .collect(),
                ),
                fail_delete: false,
            }
        }

        fn has(&self, account: &str) -> bool {
            self.entries.borrow().contains_key(account)
        }
    }

    impl KeyMover for MemKeys {
        fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
            Ok(self.entries.borrow().get(account).copied())
        }

        fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String> {
            self.entries.borrow_mut().insert(account.to_string(), *key);
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), String> {
            if self.fail_delete {
                return Err("注入的删除失败".to_string());
            }
            self.entries.borrow_mut().remove(account);
            Ok(())
        }
    }

    #[test]
    fn move_master_keys_copies_verifies_and_deletes() {
        let old = MemKeys::with(&[
            ("vault-master-key", [7u8; 32]),
            ("credentials-master-key", [9u8; 32]),
        ]);
        let new = MemKeys::with(&[]);

        let moved = move_master_keys(&old, &new).expect("搬家成功");
        assert_eq!(moved, 2);
        assert!(new.has("vault-master-key"));
        assert!(new.has("credentials-master-key"));
        assert!(!old.has("vault-master-key"), "旧条目必须删除");
        assert!(!old.has("credentials-master-key"), "旧条目必须删除");
    }

    #[test]
    fn move_master_keys_skips_missing_accounts() {
        let old = MemKeys::with(&[("vault-master-key", [7u8; 32])]);
        let new = MemKeys::with(&[]);
        let moved = move_master_keys(&old, &new).expect("搬家成功");
        assert_eq!(moved, 1, "只有存在的条目参与搬家");
        assert!(!new.has("credentials-master-key"));
    }

    #[test]
    fn move_master_keys_tolerates_delete_failure() {
        let mut old = MemKeys::with(&[("vault-master-key", [7u8; 32])]);
        old.fail_delete = true;
        let new = MemKeys::with(&[]);
        let moved = move_master_keys(&old, &new).expect("删旧失败不中止搬家");
        assert_eq!(moved, 1);
        assert!(new.has("vault-master-key"), "新条目必须已写入并校验");
        assert!(old.has("vault-master-key"), "旧条目残留属惰性，可手工清理");
    }

    #[test]
    fn is_valid_space_id_accepts_generated_uid_shape() {
        // 决策函数产出的路径依赖 uid 形态合法；生成器本身是 uuid crate，验形态即可
        let uid = uuid::Uuid::new_v4().to_string();
        assert!(is_valid_space_id(&uid), "生成的 uid 必须通过空间 id 校验");
    }
}
