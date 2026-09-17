//! `framework::space` 的独立用例（整体跑在 cfg(test) 下，不参与架构守卫判定）
//!
//! 覆盖三类判据：空间 id 校验（只接受小写 UUIDv4）、解析三情形（合法/非法回落/自举损坏）、
//! 空间布局落位与「两空间不共享任何路径」（隔离的路径层）。

#[cfg(test)]
mod cases {
    // 用例整体放在 cfg(test) 子树里：规范扫描器跳过 cfg(test) 子树，
    // 独立 `*_tests.rs` 文件不享受该豁免（assert 会被计成 panic 候选）。
    use std::path::{Path, PathBuf};

    use crate::framework::context::StorageLocation;
    use crate::framework::space::{
        is_valid_space_id, location_for, resolve_value, SpaceResolution,
    };

    /// 用例里使用的空间标识（小写 UUIDv4 形态）
    const SPACE_A: &str = "3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23";
    /// 另一个空间标识（用于「两空间互不共享路径」的对照）
    const SPACE_B: &str = "7c9d1a20-6b31-4e58-8f42-2a5c9e3d7b10";

    /// 校验只接受小写 UUIDv4，其余一律拒绝（禁路径穿越与名称注入；旧承载位 default 已废弃）
    #[test]
    fn space_id_validation_rejects_out_of_contract_values() {
        assert!(is_valid_space_id(SPACE_A));
        assert!(is_valid_space_id(SPACE_B));

        // 旧承载位与大小写形态：一律拒绝（2026-09-16 裁决：不做兼容）
        assert!(!is_valid_space_id("default"));
        assert!(!is_valid_space_id("3F2B6C1E-5A44-4D7E-9B01-8C2D6F0A1B23"));
        assert!(!is_valid_space_id("3f2b6c1e5a444d7e9b018c2d6f0a1b23"));
        assert!(!is_valid_space_id("3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b2"));
        // 版本位必须是 4：v1 与 v7 都不接受，避免把别的 UUID 世代混进来
        assert!(!is_valid_space_id("3f2b6c1e-5a44-1d7e-9b01-8c2d6f0a1b23"));
        assert!(!is_valid_space_id("3f2b6c1e-5a44-7d7e-9b01-8c2d6f0a1b23"));
        // 路径穿越、空值、近似形态
        assert!(!is_valid_space_id("../default"));
        assert!(!is_valid_space_id(".."));
        assert!(!is_valid_space_id(""));
        assert!(!is_valid_space_id("spaces/default"));
    }

    /// 解析三情形：合法值按值解析；非法值回落默认空间并留下可见登记；
    /// 连默认条目都没有 = 自举损坏，报错（不静默新建空环境）
    #[test]
    fn resolve_value_handles_valid_invalid_and_broken() {
        // 合法：按值解析，不回落
        let padded = format!("  {SPACE_A}  ");
        let valid = resolve_value(Some(&padded), Some(SPACE_B)).expect("合法值解析成功");
        assert_eq!(valid.space_id, SPACE_A);
        assert!(valid.fallback.is_none());

        // 非法：回落默认空间，但必须带可见原因（不静默、不新建空环境）
        let invalid =
            resolve_value(Some("..\\..\\Windows"), Some(SPACE_B)).expect("有默认条目时可回落");
        assert_eq!(invalid.space_id, SPACE_B);
        let fallback = invalid.fallback.expect("非法值必须留下回落登记");
        assert!(fallback.reason.contains("非法"));
        assert!(fallback.raw.contains("Windows"));

        // 自举缺失：回落默认空间并登记
        let missing = resolve_value(None, Some(SPACE_B)).expect("有默认条目时可回落");
        assert_eq!(missing.space_id, SPACE_B);
        assert!(missing.fallback.is_some());

        // 超长异常值按字符截断，避免把大段内容写进日志与界面
        let long = "x".repeat(200);
        let truncated = resolve_value(Some(&long), Some(SPACE_B))
            .expect("有默认条目时可回落")
            .fallback
            .expect("非法值必须留下回落登记");
        assert_eq!(truncated.raw.chars().count(), 65);

        // 自举损坏：没有默认条目可回落 → 报错
        assert!(resolve_value(Some("not-a-uuid"), None).is_err());
        assert!(resolve_value(None, None).is_err());
    }

    /// 所有空间统一为 spaces/<uid>/ 布局（无代际层），日志与缓存留在设备根按空间分层
    #[test]
    fn space_location_is_flat_under_spaces_dir() {
        let device_root = PathBuf::from("D:/pb-root");
        let resolution = SpaceResolution {
            space_id: SPACE_A.to_string(),
            fallback: None,
        };
        let location = location_for(&device_root, &resolution);

        assert_eq!(location.device_root, device_root);
        assert_eq!(
            location.root,
            PathBuf::from("D:/pb-root/spaces/3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23")
        );
        assert_eq!(location.data, location.root.join("data"));
        assert_eq!(location.vault, location.root.join("vault"));
        assert_eq!(
            location.logs,
            PathBuf::from("D:/pb-root/logs/3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23")
        );
        assert_eq!(
            location.cache,
            PathBuf::from("D:/pb-root/cache/3f2b6c1e-5a44-4d7e-9b01-8c2d6f0a1b23")
        );
    }

    /// 隔离的路径层：两个空间的五个目录互不为对方的前缀（同名记录不可能落到同一个文件）
    #[test]
    fn two_spaces_share_no_partition_path() {
        let device_root = PathBuf::from("D:/pb-root");
        let spaces = [SPACE_A, SPACE_B];
        let locations: Vec<_> = spaces
            .iter()
            .map(|id| {
                location_for(
                    &device_root,
                    &SpaceResolution {
                        space_id: (*id).to_string(),
                        fallback: None,
                    },
                )
            })
            .collect();

        let partitions = |location: &StorageLocation| {
            vec![
                location.root.clone(),
                location.data.clone(),
                location.vault.clone(),
                location.logs.clone(),
                location.cache.clone(),
            ]
        };

        for (index, first) in locations.iter().enumerate() {
            for second in locations.iter().skip(index + 1) {
                for a in partitions(first) {
                    for b in partitions(second) {
                        assert_ne!(a, b, "两空间不得共享同一个分区目录");
                        assert!(
                            !is_prefix(&a, &b) && !is_prefix(&b, &a),
                            "两空间的分区不得互相嵌套: {} / {}",
                            a.display(),
                            b.display()
                        );
                    }
                }
            }
        }
    }

    /// 是否为路径前缀（按组件比较，避免 `data` 误判 `database`）
    fn is_prefix(candidate: &Path, full: &Path) -> bool {
        full.starts_with(candidate) && full != candidate
    }
}
