//! 规范与依赖检查入口（AR02）：测试工具自身即检查器。
//!
//! target 名取自目录名（`tests/source_rules/main.rs` → target `source_rules`），
//! 实现与夹具因此能按目录解析，全部收在 `tests/source_rules/` 下。
//!
//! 两个约定入口（Python 薄 wrapper 按 `--exact` 过滤，名字改动会同时失效）：
//! - `scan_rust_rules`：代码级规则（panic 候选 / 生命周期逃逸 / unsafe 论证 / 层级越界）
//! - `scan_docs`：文档级规则（`//!` 模块职责、pub API、DTO 字段语义）
//!
//! 两者与全部夹具同属 source_rules target，因此 CI 跑整个 target 即可覆盖两类扫描与夹具。
//! 报告会原样打印覆盖边界（见 `rule_engine::COVERAGE_LIMITS`），不输出「语义全覆盖」结论。

mod fixtures;
mod legacy_checkers;
mod rule_engine;

use rule_engine::{scan_with_baseline, Baseline, Mode};

/// 默认扫描根下的最小文件数：防止路径解析坏掉后空扫描被当成通过。
const MIN_FILES_DEFAULT_ROOT: usize = 50;

/// 读取基线文件；缺失或损坏都视为检查器自身故障，必须显式失败。
fn load_baseline() -> Baseline {
    match Baseline::load(&Baseline::default_path()) {
        Ok(b) => b,
        Err(e) => panic!("{e}"),
    }
}

/// 断言扫描范围合理：默认根必须扫到足够多文件，自定义根至少 1 个。
fn assert_scope(report: &rule_engine::ScanReport) {
    let default_root = rule_engine::repo_root().join("src-tauri").join("src");
    let min = if report.root == default_root {
        MIN_FILES_DEFAULT_ROOT
    } else {
        1
    };
    assert!(
        report.files >= min,
        "扫描范围异常：{} 只扫到 {} 个文件（期望 ≥{min}），不能把空扫描当通过",
        report.root.display(),
        report.files
    );
}

/// 入口 1：代码级规则全仓扫描 + 棘轮判定。
#[test]
fn scan_rust_rules() {
    let baseline = load_baseline();
    let report = scan_with_baseline(Mode::Code, &baseline);
    report.print();
    assert_scope(&report);
    let problems = report.failures(&baseline);
    assert!(
        problems.is_empty(),
        "Rust 代码规范检查未通过（{} 项）：\n{}",
        problems.len(),
        problems.join("\n")
    );
}

/// 入口 2：文档级规则全仓扫描 + 棘轮判定（目录可由 COVEKIT_DOCS_ROOT 覆盖）。
#[test]
fn scan_docs() {
    let baseline = load_baseline();
    let report = scan_with_baseline(Mode::Docs, &baseline);
    report.print();
    assert_scope(&report);
    let problems = report.failures(&baseline);
    assert!(
        problems.is_empty(),
        "Rust 文档覆盖检查未通过（{} 项）：\n{}",
        problems.len(),
        problems.join("\n")
    );
}
