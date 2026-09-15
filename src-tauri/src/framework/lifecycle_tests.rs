//! `lifecycle` 的用例（单独成文件，让协调实现本体保持在 400 行内）
//!
//! 钩子表是进程级静态，libtest 默认并行跑会互相污染（计数与登记互相干扰），
//! 因此每个用例入口都先取串行锁再清表。
#[cfg(test)]
mod cases {
    // 用例放在 cfg(test) 子树里：规范扫描器会跳过 cfg(test) 子树，
    // 独立 `*_tests.rs` 文件不享受该豁免（assert 会被计成 panic 候选）
    use super::super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 用例串行锁：钩子表是进程级静态
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 每个用例入口：先串行、再清表，保证断言只看到自己登记的钩子
    fn isolated() -> MutexGuard<'static, ()> {
        let guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_for_test();
        guard
    }

    /// 测试用 owner
    const OWNER_OK: &str = "__test_ok__";
    const OWNER_VETO: &str = "__test_veto__";
    const OWNER_SLOW: &str = "__test_slow__";
    /// 全局资源（只随退出清理）：被计数钩子判据用来证明页签关闭没有误清它
    const OWNER_GLOBAL: &str = "__test_global__";
    /// 页签级资源：归属工具 `__test_tool__`
    const TOOL: &str = "__test_tool__";
    const OWNER_TAB: &str = "__test_tab__";

    /// 全局清理调用计数（断言「页签关闭跑不到全局清理」）
    static GLOBAL_DISPOSED: AtomicUsize = AtomicUsize::new(0);
    /// 页签级清理调用计数
    static TAB_DISPOSED: AtomicUsize = AtomicUsize::new(0);

    fn ok_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        Vec::new()
    }

    fn veto_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Result<(), String> {
        Err("有未保存内容".into())
    }

    fn slow_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        std::thread::sleep(Duration::from_millis(300));
        Vec::new()
    }

    fn failing_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        vec!["断开连接失败".into()]
    }

    fn global_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        GLOBAL_DISPOSED.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }

    fn tab_hook(_app: Option<&AppHandle>, _reason: CloseReason) -> Vec<String> {
        TAB_DISPOSED.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }

    /// 强退标记：只由显式动作置位，读到 true 后由用户流程负责复位（用例内复位以免污染他人）
    #[test]
    fn force_flag_round_trip() {
        let _guard = isolated();
        clear_force_for_test();
        assert!(!force_requested(), "默认不处于强退流程");
        set_force_exit();
        assert!(force_requested());
        clear_force_for_test();
        assert!(!force_requested());
    }

    /// 关闭原因：稳定字符串与解析互为逆运算；未知值不猜默认
    #[test]
    fn close_reason_codes_round_trip() {
        for reason in [
            CloseReason::Tab,
            CloseReason::Exit,
            CloseReason::Restart,
            CloseReason::Update,
            CloseReason::SpaceSwitch,
        ] {
            assert_eq!(CloseReason::from_code(reason.code()), Some(reason));
        }
        assert_eq!(CloseReason::from_code("unknown"), None);
    }

    /// prepare：任一模块拒绝即不允许关闭，原因带上模块 owner；清理阶段不执行
    #[test]
    fn prepare_can_be_rejected() {
        let _guard = isolated();
        register(ModuleLifecycle::exit_only(OWNER_OK));
        register(ModuleLifecycle::exit_only(OWNER_VETO).with_prepare(veto_hook));
        let outcome = prepare_close_with(None, CloseReason::Exit, None);
        assert!(!outcome.proceed, "有拒绝原因时不应继续关闭");
        assert_eq!(outcome.blockers.len(), 1);
        assert!(outcome.blockers[0].starts_with(OWNER_VETO));
    }

    /// 裁决合成：页面 owner 的拒绝与后端拒绝平权，两侧原因都要能看到
    #[test]
    fn decision_merges_both_sides() {
        let frontend = vec!["ssh.sessions: 有未保存内容".to_string()];
        let allowed = compose_decision(
            false,
            frontend.clone(),
            PrepareOutcome {
                proceed: true,
                blockers: Vec::new(),
            },
        );
        assert!(!allowed.proceed, "页面 owner 拒绝时后端不能放行");
        assert_eq!(allowed.blockers, frontend);

        let both = compose_decision(
            false,
            vec!["ui.editor: 有未保存内容".into()],
            PrepareOutcome {
                proceed: false,
                blockers: vec!["frp: 1 个档案正在运行".into()],
            },
        );
        assert!(!both.proceed);
        assert_eq!(
            both.blockers.len(),
            2,
            "两侧原因都要带出，用户才知道该处理谁"
        );
    }

    /// 强制关闭：跳过拦截但保留两侧原因（诊断需要知道谁本来不同意）
    #[test]
    fn decision_forced_keeps_reasons() {
        let decision = compose_decision(
            true,
            vec!["ui.editor: 有未保存内容".into()],
            PrepareOutcome {
                proceed: false,
                blockers: vec!["database: 1 个连接仍在执行查询".into()],
            },
        );
        assert!(decision.proceed, "强制关闭必须放行");
        assert!(decision.forced);
        assert_eq!(decision.blockers.len(), 2);
    }

    /// 页签关闭的作用域：只碰声明了 tab 作用域且归属该工具的模块，
    /// 全局资源（只随退出清理）绝不参与——这是「单页签关闭跑不到全局清理」的判据。
    #[test]
    fn tab_close_never_touches_exit_only_modules() {
        let _guard = isolated();
        GLOBAL_DISPOSED.store(0, Ordering::SeqCst);
        TAB_DISPOSED.store(0, Ordering::SeqCst);
        register(ModuleLifecycle::exit_only(OWNER_GLOBAL).with_dispose(global_hook));
        register(
            ModuleLifecycle::for_tool(OWNER_TAB, TOOL)
                .with_tab_scope()
                .with_prepare(veto_hook)
                .with_dispose(tab_hook),
        );

        let prepared = prepare_close_with(None, CloseReason::Tab, Some(TOOL));
        assert!(!prepared.proceed, "页签级模块的拒绝要能拦住页签关闭");
        let other = prepare_close_with(None, CloseReason::Tab, Some("other-tool"));
        assert!(other.proceed, "别的工具关闭时不能问到这个模块");

        let disposed =
            dispose_with_timeout(None, CloseReason::Tab, Some(TOOL), Duration::from_secs(2));
        assert_eq!(disposed.owners, vec![OWNER_TAB]);
        assert_eq!(TAB_DISPOSED.load(Ordering::SeqCst), 1);
        assert_eq!(
            GLOBAL_DISPOSED.load(Ordering::SeqCst),
            0,
            "页签关闭不得清理只随退出清理的全局资源"
        );

        // 同一张表在退出原因下：两个模块都参与（页签级资源也必须随进程退出释放）
        let exit_disposed =
            dispose_with_timeout(None, CloseReason::Exit, None, Duration::from_secs(2));
        assert_eq!(exit_disposed.owners, vec![OWNER_GLOBAL, OWNER_TAB]);
        assert_eq!(GLOBAL_DISPOSED.load(Ordering::SeqCst), 1);
        assert_eq!(TAB_DISPOSED.load(Ordering::SeqCst), 2);
    }

    /// 页签关闭缺少工具标识时一个模块都不问：不猜「全都问一遍」
    #[test]
    fn tab_close_without_tool_asks_nobody() {
        let _guard = isolated();
        register(
            ModuleLifecycle::for_tool(OWNER_TAB, TOOL)
                .with_tab_scope()
                .with_prepare(veto_hook),
        );
        let outcome = prepare_close_with(None, CloseReason::Tab, None);
        assert!(outcome.proceed);
        assert!(outcome.blockers.is_empty());
    }

    /// dispose：按失败描述收集，成功钩子计入 owners；未登记 dispose 的模块不参与
    #[test]
    fn dispose_collects_failures_with_owner() {
        let _guard = isolated();
        register(ModuleLifecycle::exit_only(OWNER_OK).with_dispose(ok_hook));
        register(ModuleLifecycle::exit_only("__test_fail__").with_dispose(failing_hook));
        let outcome = dispose_with_timeout(None, CloseReason::Exit, None, Duration::from_secs(2));
        assert!(!outcome.timed_out);
        assert_eq!(outcome.owners.len(), 2, "两个 dispose 钩子都应跑完");
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("断开连接失败"));
        assert!(outcome.failures[0].contains("__test_fail__"));
    }

    /// dispose 总超时：慢钩子不阻塞退出，超时计入失败并可诊断
    #[test]
    fn dispose_times_out_without_blocking_exit() {
        let _guard = isolated();
        register(ModuleLifecycle::exit_only(OWNER_SLOW).with_dispose(slow_hook));
        let outcome =
            dispose_with_timeout(None, CloseReason::Exit, None, Duration::from_millis(50));
        assert!(outcome.timed_out, "超过总超时应标记超时");
        assert!(outcome.owners.is_empty());
        assert!(outcome.failures[0].contains("总超时"));
    }

    /// 同 owner 重复登记按覆盖：不 panic、不重复执行
    #[test]
    fn register_replaces_same_owner() {
        let _guard = isolated();
        register(ModuleLifecycle::exit_only("__test_dup__").with_dispose(ok_hook));
        register(ModuleLifecycle::exit_only("__test_dup__").with_dispose(failing_hook));
        let owners = registered_owners();
        assert_eq!(
            owners.iter().filter(|o| **o == "__test_dup__").count(),
            1,
            "同 owner 只保留一条登记"
        );
    }
}
