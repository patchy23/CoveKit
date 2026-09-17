//! 夹具：先复现旧检查器的误判（见 [`crate::legacy_checkers`]），再验证修复后的行为。
//!
//! 所有夹具都走 `rule_engine::scan_source`，与真实全仓扫描同一条代码路径，
//! 避免「夹具走一套、扫描走另一套」的口径漂移。

use crate::rule_engine::{self, Baseline, Candidate, Mode};

/// 夹具用的插件目录名集合。
fn owners() -> Vec<String> {
    vec!["api".to_string(), "database".to_string(), "ssh".to_string()]
}

/// 代码模式下扫描一段源码。
fn code(rel: &str, src: &str) -> Vec<Candidate> {
    rule_engine::candidates_for(Mode::Code, rel, src, &owners())
}

/// 文档模式下扫描一段源码。
fn docs(rel: &str, src: &str) -> Vec<Candidate> {
    rule_engine::candidates_for(Mode::Docs, rel, src, &owners())
}

/// 取候选的细分类别，便于断言。
fn kinds(items: &[Candidate]) -> Vec<&'static str> {
    items.iter().map(|c| c.kind).collect()
}

/// 旧检查器按首个 `#[cfg(test)]` 行截断整个文件；修复后必须继续扫描后面的生产项。
/// 误判 1 证据：`src-tauri/src/plugins/frp/runtime.rs` 首个 `#[cfg(test)]` 出现在第 40 行的
/// 枚举变体上，旧实现因此只看得到前 39 行。
#[test]
fn cfg_test_subtree_is_skipped_without_hiding_later_production_code() {
    let src = r#"
//! 模块职责
#[cfg(test)]
mod tests {
    fn t() {
        let v: Option<u8> = None;
        let _ = v.unwrap();
    }
}

/// 生产函数（在测试模块之后）
pub fn production() -> u8 {
    let v: Option<u8> = None;
    v.expect("生产代码里的候选不能被测试模块吃掉")
}
"#;
    let found = code("fixture.rs", src);
    assert_eq!(
        kinds(&found),
        vec!["expect"],
        "测试模块里的 unwrap 应跳过，测试模块之后的生产 expect 必须被抓到：{found:?}"
    );
    assert_eq!(found[0].symbol, "production");
}

/// 注释与文档注释里提到候选名不算候选（旧实现按行匹配会误报）。
#[test]
fn comments_are_not_candidates() {
    let src = r#"
//! 模块职责：这里解释为什么不要 .unwrap() 与 panic!
/// 说明：调用方自己处理，不要 expect(
pub fn safe() -> Option<u8> {
    // Avoid .unwrap() here
    None
}
"#;
    assert!(code("fixture.rs", src).is_empty(), "注释不应产生候选");
}

/// 字符串与 raw string 里的候选名不算候选（旧实现按行匹配会误报）。
#[test]
fn string_literals_are_not_candidates() {
    // 外层用 r##"…"##，因为夹具源码里本身就含 raw string（r#"…"#）。
    let src = r##"
//! 模块职责
/// 生成提示文本
pub fn hint() -> &'static str {
    let injected = r#"unsafe { ptr::read(x) }"#;
    let _ = injected;
    "别用 .unwrap() / expect( ，用 unwrap_or"
}
"##;
    let found = code("fixture.rs", src);
    assert!(found.is_empty(), "字符串内容不应产生候选：{found:?}");
}

/// unsafe 的 SAFETY 论证必须紧邻；多行论证块有效，中间夹代码无效。
#[test]
fn unsafe_requires_adjacent_safety_note() {
    let src = r#"
//! 模块职责
/// 合法：单行紧邻论证
pub fn ok_one(p: *const u8) -> u8 {
    // SAFETY: 指针由调用方保证有效且对齐。
    unsafe { *p }
}

/// 合法：多行论证块
pub fn ok_many(p: *const u8) -> u8 {
    // SAFETY: 同上；
    // 该块覆盖同一不变量，允许跨行。
    unsafe { *p }
}

/// 非法：论证与 unsafe 之间夹了代码行（旧实现只认「紧邻注释块」，这条本来会过）
pub fn bad(p: *const u8) -> u8 {
    // SAFETY: 这句离 unsafe 太远，中间夹了代码行，不算紧邻。
    let _ = p;
    unsafe { *p }
}
"#;
    let found = code("fixture.rs", src);
    assert_eq!(found.len(), 1, "只应命中「不紧邻」的那一处：{found:?}");
    assert_eq!(found[0].symbol, "bad");
    assert_eq!(found[0].rule, "unsafe_without_safety");
}

/// 多行属性不应打断文档判定（`#[allow(...)]` 跨行、`#[serde(...)]` 跨行）。
#[test]
fn multi_line_attributes_do_not_break_doc_detection() {
    let documented = r#"
//! 模块职责
/// 有文档的函数
#[allow(
    clippy::needless_return
)]
pub fn documented() -> u8 {
    0
}
"#;
    assert!(
        docs("fixture.rs", documented).is_empty(),
        "多行属性之后的 /// 仍应被认可"
    );

    let undocumented = r#"
//! 模块职责
#[allow(
    clippy::needless_return
)]
pub fn undocumented() -> u8 {
    0
}
"#;
    let found = docs("fixture.rs", undocumented);
    assert_eq!(kinds(&found), vec!["api_doc"], "多行属性不能掩盖缺文档");

    let fields = r#"
//! 模块职责
/// 配置
#[derive(serde::Serialize)]
pub struct Config {
    #[serde(
        rename = "db_type"
    )]
    pub db_type: String,
}
"#;
    let found = docs("fixture.rs", fields);
    assert_eq!(
        kinds(&found),
        vec!["field_doc"],
        "多行 serde 属性之后的字段仍要求 ///：{found:?}"
    );
}

/// 固有 impl 里缩进的 pub 方法同样要求文档（旧正则只看行首，漏掉这类）。
/// 误判 3 证据：旧 `check_docs.py` 对 `impl X { pub fn undocumented(&self) {} }` 返回空问题列表。
#[test]
fn inherent_impl_methods_require_docs() {
    let src = r#"
//! 模块职责
/// 存储
pub struct Store {
    /// 连接串
    pub url: String,
}

impl Store {
    pub fn save(&self) {}
}
"#;
    let found = docs("fixture.rs", src);
    assert_eq!(kinds(&found), vec!["api_doc"], "{found:?}");
    assert_eq!(found[0].symbol, "Store::save");
}

/// trait 实现里的方法不能是 pub，不要求文档；私有项也不要求。
#[test]
fn trait_impl_methods_and_private_items_are_not_required() {
    let src = r#"
//! 模块职责
/// 句柄
pub struct Handle;

impl Drop for Handle {
    fn drop(&mut self) {}
}

fn private_helper() {}
"#;
    assert!(
        docs("fixture.rs", src).is_empty(),
        "trait 实现与私有项不应要求文档"
    );
}

/// 可见性矩阵：pub(crate)/pub(super) 要求文档，私有不要求。
#[test]
fn visibility_matrix_for_docs() {
    let src = r#"
//! 模块职责
pub(crate) fn crate_visible() {}

pub(super) fn super_visible() {}

fn private() {}
"#;
    let found = docs("fixture.rs", src);
    assert_eq!(
        found.len(),
        2,
        "pub(crate) 与 pub(super) 都应要求文档：{found:?}"
    );
}

/// 缺 `//!` 文件头必须报 module_doc；有则放行。
#[test]
fn module_doc_is_required() {
    let without = "/// 函数\npub fn f() {}\n";
    assert_eq!(kinds(&docs("fixture.rs", without)), vec!["module_doc"]);

    let with = "//! 模块职责\n/// 函数\npub fn f() {}\n";
    assert!(docs("fixture.rs", with).is_empty());
}

/// 字段说明只提示；模块与公开 API 缺失仍会使检查失败。
#[test]
fn field_documentation_is_advisory_without_weakening_api_checks() {
    let baseline = Baseline::parse("{}").expect("夹具基线应可解析");
    let source = "//! 模块职责\n/// 配置\npub struct Config { pub name: String }\n";
    let report = rule_engine::report_for(Mode::Docs, "fixture.rs", source, &owners(), &baseline);
    assert_eq!(report.count("field_doc"), 1);
    assert!(report.failures(&baseline).is_empty());
    let missing_api = format!("{source}\npub fn save() {{}}\n");
    let report =
        rule_engine::report_for(Mode::Docs, "fixture.rs", &missing_api, &owners(), &baseline);
    assert!(!report.failures(&baseline).is_empty());
}

/// 例外按「路径 + 符号 + 类别」绑定：同一句 expect 文案不能让整个文件放行。
#[test]
fn exceptions_bind_to_symbol_not_to_text() {
    let src = r#"
//! 模块职责
/// 注册
pub fn a() {
    let r: Result<u8, &str> = Ok(1);
    let _ = r.expect("IPC 命令重复注册");
}

/// 另一个注册
pub fn b() {
    let r: Result<u8, &str> = Ok(1);
    let _ = r.expect("IPC 命令重复注册");
}
"#;
    let baseline = Baseline::parse(
        r#"{"exceptions":[{"path":"fixture.rs","symbol":"a","kind":"expect","reason":"启动期 fail-fast"}]}"#,
    )
    .expect("夹具基线应可解析");
    let report = rule_engine::report_for(Mode::Code, "fixture.rs", src, &owners(), &baseline);
    assert_eq!(
        report.candidates.len(),
        1,
        "只有未登记的符号应保留：{:?}",
        report.candidates
    );
    assert_eq!(report.candidates[0].symbol, "b");
    assert_eq!(report.exceptions_used.len(), 1, "登记的例外应被记为命中");
    assert!(report.stray_exceptions.is_empty());
}

/// 登记的例外一旦不再命中任何候选，必须报失效（符号移动即失效）。
#[test]
fn stale_exception_is_reported() {
    let src = r#"
//! 模块职责
/// 注册
pub fn renamed() {}
"#;
    let baseline = Baseline::parse(
        r#"{"exceptions":[{"path":"fixture.rs","symbol":"old_name","kind":"expect","reason":"已移动的符号"}]}"#,
    )
    .expect("夹具基线应可解析");
    let report = rule_engine::report_for(Mode::Code, "fixture.rs", src, &owners(), &baseline);
    assert_eq!(report.stray_exceptions.len(), 1);
    let problems = report.failures(&baseline);
    assert!(
        problems.iter().any(|p| p.contains("失效例外")),
        "失效例外必须进入失败原因：{problems:?}"
    );
}

/// cfg 组合（`all(test, …)` / `not(test)`）不作真假推断：一律保守扫描并列入未覆盖清单。
#[test]
fn cfg_combinations_are_scanned_conservatively() {
    let src = r#"
//! 模块职责
#[cfg(not(test))]
fn prod_only() {
    let v: Option<u8> = None;
    let _ = v.unwrap();
}

#[cfg(all(test, feature = "fixture"))]
fn maybe_test() {
    let v: Option<u8> = None;
    let _ = v.unwrap();
}
"#;
    let found = code("fixture.rs", src);
    assert_eq!(found.len(), 2, "cfg 组合内的候选不能静默跳过：{found:?}");
    let notes = rule_engine::scan_source(Mode::Code, "fixture.rs", src, &owners()).notes;
    assert!(
        notes.iter().any(|n| n.contains("cfg 组合不作真假推断")),
        "必须列出未推断的 cfg 组合：{notes:?}"
    );
}

/// `assert!` 系列不在旧 PANIC_PATTERN 里，仍要纳入候选清单；`cfg_attr` 里的同名文本不是调用。
#[test]
fn assert_macros_are_listed_but_attribute_text_is_not() {
    let src = r#"
//! 模块职责
/// 校验路由表
pub fn validate_routing() {
    assert!(true, "登记了命令却没有路由分支");
    debug_assert!(true);
}
"#;
    assert_eq!(
        kinds(&code("fixture.rs", src)),
        vec!["assert", "assert_debug"]
    );

    // 真实场景：`windows_subsystem = "windows"` 所在的 cfg_attr 含 `debug_assertions`，
    // 旧正则按行匹配会把 `debug_assert` 当成 assert 调用。
    let attr_only = r#"
//! 模块职责
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
"#;
    assert!(
        code("fixture.rs", attr_only).is_empty(),
        "属性里的同名文本不是宏调用，不能误报"
    );
}

/// 生命周期逃逸调用零容忍。
#[test]
fn lifetime_escapes_are_flagged() {
    let src = r#"
//! 模块职责
/// 泄漏
pub fn leak(v: Vec<u8>) -> &'static [u8] {
    let forgotten = std::mem::forget(v);
    let _ = forgotten;
    Box::leak(vec![1u8].into_boxed_slice())
}
"#;
    let found = code("fixture.rs", src);
    assert_eq!(kinds(&found), vec!["lifetime_escape", "lifetime_escape"]);
}

/// 层级越界：framework 不得引用插件；插件之间不得互相 import；
/// 但 plugins/mod.rs 路由表与 plugins 下的共享模块（如 ipc_registry）不算越界。
#[test]
fn layer_violations_are_detected_without_false_positives() {
    let bad_framework = r#"
//! 模块职责
use crate::plugins::ssh::models::ServerProfile;
"#;
    assert_eq!(
        kinds(&code("framework/store.rs", bad_framework)),
        vec!["layer_violation"]
    );

    let cross_plugin = r#"
//! 模块职责
use crate::plugins::database::models::DbType;
"#;
    assert_eq!(
        kinds(&code("plugins/api/mod.rs", cross_plugin)),
        vec!["layer_violation"]
    );

    let self_use = r#"
//! 模块职责
use crate::plugins::api::models::ApiRecord;
"#;
    assert!(
        code("plugins/api/mod.rs", self_use).is_empty(),
        "插件引用自身不算越界"
    );

    let router = r#"
//! 模块职责
pub(crate) fn dispatch() {
    let _ = crate::plugins::ssh::invoke_handler();
}
"#;
    assert!(
        code("plugins/mod.rs", router).is_empty(),
        "组装/路由文件引用各插件不算越界"
    );

    let shared = r#"
//! 模块职责
use crate::plugins::ipc_registry;
"#;
    assert!(
        code("plugins/frp/mod.rs", shared).is_empty(),
        "plugins 下的共享模块（非插件 owner 目录）不算跨插件越界"
    );

    let framework_ok = r#"
//! 模块职责
use crate::framework::paths;
"#;
    assert!(code("framework/store.rs", framework_ok).is_empty());
}

/// 宏体内含候选时，syn 不展开：必须显式列为未覆盖项，不能假装已覆盖。
#[test]
fn unexpanded_macros_are_reported_as_notes() {
    let src = r#"
//! 模块职责
macro_rules! must_ok {
    ($e:expr) => {
        $e.expect("宏展开后的候选")
    };
}

/// 使用宏
pub fn f() {
    let v: Option<u8> = None;
    let _ = must_ok!(v);
}
"#;
    let outcome = rule_engine::scan_source(Mode::Code, "fixture.rs", src, &owners());
    assert!(
        outcome.candidates.is_empty(),
        "宏体是 token 流，不产生可归因候选：{:?}",
        outcome.candidates
    );
    assert!(
        outcome.notes.iter().any(|n| n.contains("`must_ok`")),
        "必须列出未展开的宏：{:?}",
        outcome.notes
    );
}

/// 未知宏调用点汇总提示（不逐条刷屏），并给出数量。
#[test]
fn unknown_macro_calls_are_summarized() {
    let src = r#"
//! 模块职责
/// 打印
pub fn f() {
    println!("hello {}", 1);
    vec![1u8];
}
"#;
    let outcome = rule_engine::scan_source(Mode::Code, "fixture.rs", src, &owners());
    // 第三方宏调用点只汇总数量（报告阶段合并成一条），不在文件级逐条刷屏。
    assert_eq!(outcome.other_macro_calls, 2, "{outcome:?}");
    assert!(outcome.notes.is_empty(), "{:?}", outcome.notes);
}

/// `.clone()` 只计数、不产生候选（保持「只报告不拦截」）。
#[test]
fn clone_is_counted_not_gated() {
    let src = r#"
//! 模块职责
/// 复制
pub fn f(v: Vec<u8>) -> Vec<u8> {
    let a = v.clone();
    let _ = v.clone();
    a
}
"#;
    let outcome = rule_engine::scan_source(Mode::Code, "fixture.rs", src, &owners());
    assert!(outcome.candidates.is_empty());
    assert_eq!(outcome.clone_count, 2);
}

/// 枚举变体与 trait 关联项不强制文档（覆盖边界写进报告，不静默声称全覆盖）。
#[test]
fn enum_variants_are_out_of_scope() {
    let src = r#"
//! 模块职责
/// 事件
pub enum Event {
    Started,
    Stopped,
}
"#;
    assert!(docs("fixture.rs", src).is_empty());
}

/// 解析失败必须显式暴露，不能返回「无候选」假装通过。
#[test]
fn parse_failure_is_exposed() {
    let outcome = rule_engine::scan_source(Mode::Code, "broken.rs", "fn broken( { ", &owners());
    assert!(outcome.parse_failed);
}

/// 归属符号含 impl 类型前缀，便于例外登记与人工定位。
#[test]
fn symbols_include_impl_prefix_and_module_chain() {
    let src = r#"
//! 模块职责
/// 存储
pub struct Store {
    /// 连接串
    pub url: String,
}

impl Store {
    /// 保存
    pub fn save(&self) {
        let v: Option<u8> = None;
        let _ = v.unwrap();
    }
}

pub mod inner {
    /// 内部函数
    pub fn run() {
        let v: Option<u8> = None;
        let _ = v.expect("inner");
    }
}
"#;
    let found = code("fixture.rs", src);
    let symbols: Vec<&str> = found.iter().map(|c| c.symbol.as_str()).collect();
    assert!(symbols.contains(&"Store::save"), "{symbols:?}");
    assert!(symbols.contains(&"inner::run"), "{symbols:?}");
}

/// 架构守卫：绕过 `framework::paths` 直取落盘根必须被抓出，自举文件放行。
#[test]
fn paths_bypass_is_detected_and_bootstrap_file_is_allowed() {
    let src = r#"
//! 模块职责
pub fn secrets_file(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default()
}
"#;
    assert_eq!(
        kinds(&code("src-tauri/src/plugins/api/store.rs", src)),
        vec!["paths_bypass"]
    );
    assert!(
        code("src-tauri/src/framework/paths.rs", src).is_empty(),
        "framework/paths.rs 是路径解析的自举实现，允许直接调 Tauri 路径 API"
    );
}

/// 架构守卫：框架层引用业务表名必须被抓出，插件自身引用不算违规。
#[test]
fn framework_referencing_plugin_table_is_detected() {
    let mut tables = std::collections::BTreeSet::new();
    tables.insert("ssh_profiles".to_string());
    let src = r#"
//! 模块职责
pub fn vacuum_sql() -> &'static str {
    // ssh_profiles 只是注释里的名字，不算引用
    "SELECT count(*) FROM ssh_profiles"
}
"#;
    let found =
        rule_engine::foreign_table_candidates("src-tauri/src/framework/vacuum.rs", src, &tables);
    assert_eq!(kinds(&found), vec!["foreign_table"]);
    assert!(
        rule_engine::foreign_table_candidates("src-tauri/src/plugins/ssh/store.rs", src, &tables)
            .is_empty(),
        "插件引用自己的表不算框架层越界"
    );
    let unrelated = r#"
//! 模块职责
pub const NOTE: &str = "没有命中任何业务表名的普通字面量";
"#;
    assert!(rule_engine::foreign_table_candidates(
        "src-tauri/src/framework/note.rs",
        unrelated,
        &tables
    )
    .is_empty());
}

/// 架构守卫：表名清单由插件 DDL 推导，`SHOW CREATE TABLE` 与短名不入清单。
#[test]
fn plugin_tables_are_derived_from_ddl_only() {
    let dir = std::env::temp_dir().join(format!("pb-tables-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ddl = concat!(
        "const DDL: &str = \"CREATE TABLE IF NOT EXISTS ssh_profiles (id TEXT); ",
        "CREATE TABLE t1 (id INTEGER);\";\n",
        "const SHOW: &str = \"SHOW CREATE TABLE `shop`.`orders`\";\n"
    );
    std::fs::write(dir.join("mod.rs"), ddl).unwrap();
    let names = rule_engine::plugin_tables(&dir);
    assert!(names.contains("ssh_profiles"), "建表名应入清单：{names:?}");
    assert!(
        !names
            .iter()
            .any(|n| n == "t1" || n == "shop" || n == "orders"),
        "临时表与 SHOW CREATE TABLE 不应入清单：{names:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
