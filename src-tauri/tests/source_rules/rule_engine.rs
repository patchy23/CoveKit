//! 规范检查引擎（AR02）：用 syn 2 AST 扫描自有 Rust 源码，产出可审计的候选清单。
//!
//! 设计边界（对应 docs/standards/05-Rust代码规范.md §10 与任务书 §5.2）：
//! - 只按属性 AST 的字面结构排除直接 `#[cfg(test)]` 子树，并继续遍历后面的生产项；
//!   这不是 cfg 求值器：`cfg(all(test, …))`、`cfg(not(test))`、`cfg_attr` 一律保守扫描
//!   （宁可多扫也不静默漏过生产候选），同时把这类位置列入报告的「未覆盖/歧义」清单。
//! - 宏展开、cfg 组合与别名解析无法仅靠 syn 证明，报告单列未覆盖范围，不声称语义全覆盖。
//! - 例外按「相对路径 + 符号 + 调用类别 + 原因」逐条登记；符号移动即视为失效并报错，
//!   不允许用同一句 expect 文案全局放行。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Visibility};

/// 规则清单：(规则名, 判定口径)。报告里逐条打印，避免「查了但没说查什么」。
pub const RULES: &[(&str, &str)] = &[
    ("panic", "运行期直接 panic 候选：unwrap/expect/unwrap_err/expect_err/unwrap_unchecked 与 panic!/unreachable!/todo!/assert! 系列"),
    ("lifetime_escape", "禁止的生命周期逃逸调用：Box::leak / mem::forget / transmute / transmute_copy（零容忍）"),
    ("unsafe_without_safety", "unsafe（块/函数/impl/trait）缺少紧邻 `// SAFETY:` 论证（零容忍）"),
    ("layer_violation", "层级依赖越界：framework 引用 plugins、插件之间互相 import（零容忍）"),
    ("paths_bypass", "绕过 framework::paths 直接取落盘根：Tauri 的 app_data_dir / app_config_dir / app_local_data_dir / app_cache_dir / app_log_dir（零容忍，framework/paths.rs 为自举例外）"),
    ("foreign_table", "框架层直接引用业务表名：framework 下字符串字面量命中插件自报的建表名（零容忍）"),
    ("module_doc", "文件缺少 //! 模块职责注释"),
    ("api_doc", "pub / pub(crate) / pub(super) 项（含固有 impl 方法）缺少 /// 职责注释"),
    ("field_doc", "pub 结构体字段缺少 ///，仅提示核对非显然语义"),
];

/// 覆盖边界声明：报告里原样打印，任何一条失效都必须更新这里而不是含糊带过。
pub const COVERAGE_LIMITS: &[&str] = &[
    "只能证明语法层事实：不做 cfg 真假求值、不做宏展开、不做别名/`super::` 相对路径解析",
    "层级规则只解析 `crate::plugins::<owner>` 绝对路径；`super::`/`self::` 拼出的跨插件引用不在覆盖范围",
    "枚举变体与 trait 实现的关联项不强制文档（语义充分性由人工审查）",
    "集合索引 panic、Clippy 已覆盖的编译后诊断不在本工具范围",
    "paths_bypass 只认 Tauri 路径访问器的方法名：自己再包一层同名函数、或用别名调用不在覆盖范围（这正是要禁的间接绕过）",
    "foreign_table 为文本级扫描（去行注释后取字符串字面量，按词边界匹配建表名），不做宏展开与词法分析；注释里的表名不算引用",
    "foreign_table 的表名清单由插件源码里的 CREATE TABLE/INDEX 动态推导，长度 <3 的名字（临时表 t/t1）不纳入，避免误伤通用代码",
    "两条架构守卫（paths_bypass / foreign_table）只判定生产前缀（首个 `#[cfg(test)]` 标记之前）并跳过 `*_tests.rs` 测试专用文件；文件内测试夹具不算违规",
];

/// 扫描模式：code = 代码级规则（panic/逃逸/unsafe/层级）；docs = 文档级规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 代码级规则。
    Code,
    /// 文档级规则。
    Docs,
}

impl Mode {
    /// 该模式是否检查代码级规则。
    pub fn wants_code(self) -> bool {
        matches!(self, Mode::Code)
    }

    /// 该模式是否检查文档级规则。
    pub fn wants_docs(self) -> bool {
        matches!(self, Mode::Docs)
    }

    /// 报告与失败判定用的模式名。
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Code => "代码规则",
            Mode::Docs => "文档规则",
        }
    }
}

/// 一处候选：规则 + 细分类别（kind）+ 位置 + 归属符号，例外按这些字段精确匹配。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate {
    /// 规则名（见 [`RULES`]）。
    pub rule: &'static str,
    /// 细分类别：panic 类为 unwrap/expect/...，其余与规则同名。
    pub kind: &'static str,
    /// 仓库相对路径（posix 分隔符）。
    pub path: String,
    /// 归属符号（如 `register`、`Store::replace`），用于例外绑定与人工定位。
    pub symbol: String,
    /// 1 起始行号。
    pub line: usize,
    /// 证据片段（截断源码行或说明）。
    pub detail: String,
}

/// 基线文件（scripts/rust_rules_baseline.json）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Baseline {
    /// panic 各分类的存量上限（棘轮：只降不升）。
    pub panic_by_kind: BTreeMap<String, usize>,
    /// 旧的 panic 总数键，保留兼容：作为所有分类之和的上限。
    pub panic_violations: Option<usize>,
    /// 文档缺失的存量上限（棘轮）。
    pub doc_issues: usize,
    /// 逐条登记的合法例外。
    pub exceptions: Vec<Exception>,
}

/// 一条登记例外：位置 + 符号 + 类别 + 原因，四者齐全才算有效。
#[derive(Debug, Clone, Deserialize)]
pub struct Exception {
    /// 仓库相对路径。
    pub path: String,
    /// 归属符号。
    pub symbol: String,
    /// 细分类别（与候选的 kind 对应）。
    pub kind: String,
    /// 放行原因（必须写清为什么是可证不变量或 fail-fast）。
    pub reason: String,
}

impl Baseline {
    /// 从 JSON 文本解析基线；解析失败由调用方转成「检查器自身故障」。
    pub fn parse(text: &str) -> Result<Baseline, String> {
        serde_json::from_str(text).map_err(|e| format!("基线文件解析失败：{e}"))
    }

    /// 读取基线文件。
    pub fn load(path: &Path) -> Result<Baseline, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("基线文件读取失败（{}）：{e}", path.display()))?;
        Baseline::parse(&text)
    }

    /// 基线文件的默认位置。
    pub fn default_path() -> PathBuf {
        repo_root().join("scripts").join("rust_rules_baseline.json")
    }
}

/// 一次扫描的结果：候选、计数、例外命中与未覆盖说明。
#[derive(Debug, Clone)]
pub struct ScanReport {
    /// 扫描模式。
    pub mode: Mode,
    /// 扫描根目录（绝对路径）。
    pub root: PathBuf,
    /// 参与扫描的文件数。
    pub files: usize,
    /// 参与扫描的字节数。
    pub bytes: usize,
    /// 全部候选（已过滤登记例外），按路径与行号排序。
    pub candidates: Vec<Candidate>,
    /// 命中登记例外的说明（path::symbol::kind（原因））。
    pub exceptions_used: BTreeSet<String>,
    /// 登记了但未命中任何候选的例外键（失效例外，判定失败）。
    pub stray_exceptions: BTreeSet<String>,
    /// 未覆盖/歧义项说明 -> 出现次数。
    pub notes: BTreeMap<String, usize>,
    /// `.clone()` 出现次数（仅报告，不拦截）。
    pub clone_count: usize,
}

impl ScanReport {
    /// 空报告骨架。
    fn new(mode: Mode, root: PathBuf) -> Self {
        Self {
            mode,
            root,
            files: 0,
            bytes: 0,
            candidates: Vec::new(),
            exceptions_used: BTreeSet::new(),
            stray_exceptions: BTreeSet::new(),
            notes: BTreeMap::new(),
            clone_count: 0,
        }
    }

    /// 某个规则下的候选数。
    pub fn count(&self, rule: &str) -> usize {
        self.candidates.iter().filter(|c| c.rule == rule).count()
    }

    /// panic 类候选按 kind 计数。
    pub fn panic_by_kind(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for c in self.candidates.iter().filter(|c| c.rule == "panic") {
            *m.entry(c.kind.to_string()).or_insert(0) += 1;
        }
        m
    }

    /// 按登记例外切分候选：命中的移出候选表，未命中的登记项记为失效例外。
    ///
    /// 例外只在**本模式扫描的规则族**内校验（代码规则与文档规则是两个入口），
    /// 否则跑 `scan_docs` 时会把代码类例外全判成失效。
    pub fn apply_exceptions(&mut self, baseline: &Baseline) {
        let mut used = BTreeSet::new();
        let mut kept = Vec::new();
        for c in std::mem::take(&mut self.candidates) {
            let hit = baseline
                .exceptions
                .iter()
                .filter(|e| kind_belongs_to(self.mode, &e.kind))
                .find(|e| e.path == c.path && e.symbol == c.symbol && e.kind == c.kind);
            match hit {
                Some(e) => {
                    used.insert(format!(
                        "{}::{}::{}（{}）",
                        c.path, c.symbol, c.kind, e.reason
                    ));
                }
                None => kept.push(c),
            }
        }
        self.candidates = kept;
        self.stray_exceptions = baseline
            .exceptions
            .iter()
            .filter(|e| kind_belongs_to(self.mode, &e.kind))
            .map(|e| format!("{}::{}::{}", e.path, e.symbol, e.kind))
            .filter(|k| !used.iter().any(|u| u.starts_with(&format!("{k}（"))))
            .collect();
        self.exceptions_used = used;
    }

    /// 按模式产出失败原因（空 = 通过）。不自行 panic，便于报告先完整打印。
    pub fn failures(&self, baseline: &Baseline) -> Vec<String> {
        let mut problems = Vec::new();

        if self.files == 0 {
            problems.push("扫描到 0 个源文件：路径解析异常，不能把空扫描当通过".to_string());
        }
        for note in self.stray_exceptions.iter() {
            problems.push(format!(
                "失效例外 `{note}`：登记的位置/符号/类别已不再命中任何候选，请同步更新登记"
            ));
        }
        for note in self.notes.keys() {
            if note.starts_with("解析失败") {
                problems.push(note.clone());
            }
        }

        if self.mode.wants_code() {
            let counted = self.panic_by_kind();
            let over = |kind: &str| {
                counted.get(kind).copied().unwrap_or(0)
                    > baseline.panic_by_kind.get(kind).copied().unwrap_or(0)
            };
            for (kind, n) in counted.iter() {
                let limit = baseline.panic_by_kind.get(kind).copied().unwrap_or(0);
                if *n > limit {
                    problems.push(format!(
                        "panic 候选 `{kind}` 有 {n} 处，超过基线 {limit}（新增 {} 处，需分类修复而不是抬高基线）",
                        n - limit
                    ));
                }
            }
            if let Some(total_limit) = baseline.panic_violations {
                let total: usize = counted.values().sum();
                if total > total_limit {
                    problems.push(format!(
                        "panic 候选合计 {total} 处，超过总数基线 {total_limit}"
                    ));
                }
            }
            for rule in [
                "lifetime_escape",
                "unsafe_without_safety",
                "layer_violation",
                "paths_bypass",
                "foreign_table",
            ] {
                let n = self.count(rule);
                if n > 0 {
                    problems.push(format!("零容忍规则 `{rule}` 命中 {n} 处（无基线）"));
                }
            }
            let mut evidence: Vec<String> = self
                .candidates
                .iter()
                .filter(|c| c.rule != "panic" || over(c.kind))
                .map(describe)
                .collect();
            evidence.sort();
            evidence.truncate(40);
            problems.extend(evidence);
        }

        if self.mode.wants_docs() {
            // AST 无法判断字段是否语义显然，字段文档只提示，不要求逐字段复述名字。
            let n = self.count("module_doc") + self.count("api_doc");
            if n > baseline.doc_issues {
                problems.push(format!(
                    "文档缺失 {n} 处，超过基线 {}（新增 {} 处）：",
                    baseline.doc_issues,
                    n - baseline.doc_issues
                ));
                let mut evidence: Vec<String> = self
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.rule != "field_doc")
                    .map(describe)
                    .collect();
                evidence.sort();
                evidence.truncate(60);
                problems.extend(evidence);
            }
        }

        problems
    }

    /// 打印报告：规则数、源范围、计数、例外、未覆盖边界。禁止「语义全覆盖」式结论。
    pub fn print(&self) {
        println!(
            "== patchyBox 规范检查（{}）：{} 条规则，源范围 {} -> {} 个文件 / {} 字节 ==",
            self.mode.as_str(),
            RULES.len(),
            self.root.display(),
            self.files,
            self.bytes
        );
        for (name, desc) in RULES.iter() {
            let is_doc_rule = matches!(*name, "module_doc" | "api_doc" | "field_doc");
            if is_doc_rule == self.mode.wants_docs() {
                println!("  规则 {name}：{desc}");
            }
        }
        let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
        for c in self.candidates.iter() {
            *by_rule.entry(c.rule).or_insert(0) += 1;
        }
        let summary = if by_rule.is_empty() {
            "无候选".to_string()
        } else {
            by_rule
                .iter()
                .map(|(r, n)| format!("{r}={n}"))
                .collect::<Vec<_>>()
                .join("、")
        };
        println!("  候选：{summary}");
        if self.mode.wants_code() {
            println!("  .clone() 出现次数（仅报告）：{}", self.clone_count);
        }
        for c in self.candidates.iter().take(80) {
            println!("    {}", describe(c));
        }
        if self.candidates.len() > 80 {
            println!("    …（另有 {} 处）", self.candidates.len() - 80);
        }
        println!("  命中登记例外 {} 条：", self.exceptions_used.len());
        for u in self.exceptions_used.iter() {
            println!("    {u}");
        }
        if !self.notes.is_empty() {
            println!("  未覆盖 / 歧义项（不做 cfg 真假求值，一律保守扫描）：");
            for (note, n) in self.notes.iter() {
                println!("    {note}（×{n}）");
            }
        }
        println!("  覆盖边界：");
        for limit in COVERAGE_LIMITS.iter() {
            println!("    - {limit}");
        }
    }
}

/// 把候选格式化成一行证据。
pub fn describe(c: &Candidate) -> String {
    format!(
        "{}:{} [{}:{}] {} -> {}",
        c.path, c.line, c.rule, c.kind, c.symbol, c.detail
    )
}

/// 仓库根目录（src-tauri 的上一级），由编译期 MANIFEST 路径解析，不依赖调用 cwd。
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 必有上一级仓库根")
        .to_path_buf()
}

/// 扫描根：默认 `src-tauri/src`；文档模式可用 `PATCHYBOX_DOCS_ROOT` 覆盖（wrapper 传规范化绝对路径）。
pub fn scan_root(mode: Mode) -> PathBuf {
    if mode.wants_docs() {
        if let Ok(custom) = std::env::var("PATCHYBOX_DOCS_ROOT") {
            return PathBuf::from(custom);
        }
    }
    repo_root().join("src-tauri").join("src")
}

/// 规则族归属：文档类规则只在 `scan_docs` 检查，其余只在 `scan_rust_rules` 检查。
fn kind_belongs_to(mode: Mode, kind: &str) -> bool {
    let is_doc = matches!(kind, "module_doc" | "api_doc" | "field_doc");
    is_doc == mode.wants_docs()
}

/// 枚举插件 owner 目录名（用于跨插件 import 判定，避免硬编码插件清单）。
pub fn plugin_owners(scan_dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(scan_dir.join("plugins")) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    names.push(n.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

/// 绕过 `framework::paths` 直接取落盘根的 Tauri 路径访问器（零容忍）。
///
/// 唯一例外是路径解析的自举文件 `framework/paths.rs`：其它任何位置都要经 `paths::*` 拿目录，
/// 否则存储根切换、旧布局回落与只读探测会被绕过。
pub const PATHS_BYPASS_METHODS: &[&str] = &[
    "app_data_dir",
    "app_config_dir",
    "app_local_data_dir",
    "app_cache_dir",
    "app_log_dir",
];

/// 是否为测试专用文件（`*_tests.rs`）：其内容整体跑在 cfg(test) 下，不参与架构守卫判定。
fn is_test_only_file(rel: &str) -> bool {
    rel.ends_with("_tests.rs")
}

/// 是否为路径解析的自举文件（唯一允许直接调 Tauri 路径 API 的位置）。
fn is_paths_bootstrap(rel: &str) -> bool {
    rel.ends_with("framework/paths.rs")
}

/// 从插件源码收集业务表名（`CREATE TABLE/INDEX [IF NOT EXISTS] <name>`）。
///
/// 表名清单由插件自己的 DDL 推导，不在守卫里硬编码：插件新增表自动纳入判定。
/// 长度 <3 的名字（测试里的临时表 `t`/`t1`）忽略；`SHOW CREATE TABLE` 不是建表语句，跳过。
pub fn plugin_tables(plugins_dir: &Path) -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_rs_files(plugins_dir, &mut files);
    let mut names = BTreeSet::new();
    for path in files {
        if let Ok(source) = std::fs::read_to_string(&path) {
            collect_ddl_names(&source, &mut names);
        }
    }
    names
}

/// 从一段文本里提取建表名。
fn collect_ddl_names(source: &str, out: &mut BTreeSet<String>) {
    const KEYWORDS: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS ",
        "CREATE INDEX IF NOT EXISTS ",
        "CREATE TABLE ",
        "CREATE INDEX ",
    ];
    for keyword in KEYWORDS {
        let mut rest = source;
        while let Some(idx) = rest.find(keyword) {
            let before = rest[..idx].trim_end();
            let show = before.to_ascii_uppercase().ends_with("SHOW");
            let after = &rest[idx + keyword.len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !show && name.len() >= 3 {
                out.insert(name);
            }
            rest = &rest[idx + keyword.len()..];
        }
    }
}

/// 截断到首个 `#[cfg(test)]` 标记之前：测试夹具里出现的业务表名不算框架层引用。
///
/// 这是文本级近似，与引擎按 AST 排除 `#[cfg(test)]` 子树的口径略有差异，边界见 [`COVERAGE_LIMITS`]。
fn production_prefix(source: &str) -> &str {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if line.trim_start().starts_with("#[cfg(test)]") {
            return &source[..offset];
        }
        offset += line.len();
    }
    source
}

/// 框架层直接引用业务表名的候选：`framework` 下的字符串字面量命中插件建表名（零容忍）。
///
/// 覆盖边界见 [`COVERAGE_LIMITS`]：文本级扫描，去行注释后按词边界匹配。
pub fn foreign_table_candidates(
    rel: &str,
    source: &str,
    tables: &BTreeSet<String>,
) -> Vec<Candidate> {
    if tables.is_empty() || !rel.contains("/framework/") || is_test_only_file(rel) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (idx, raw) in production_prefix(source).lines().enumerate() {
        let code = strip_line_comment(raw);
        for literal in string_literals(&code) {
            for table in tables {
                if contains_word(&literal, table) {
                    out.push(Candidate {
                        rule: "foreign_table",
                        kind: "foreign_table",
                        path: rel.to_string(),
                        symbol: "<literal>".to_string(),
                        line: idx + 1,
                        detail: format!("框架层引用业务表 `{table}`：{literal}"),
                    });
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 去掉行注释（引号内的 `//` 不算注释起点）。
fn strip_line_comment(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut in_string = false;
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && in_string {
            out.push(c);
            if i + 1 < chars.len() {
                out.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
        }
        if !in_string && c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            break;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// 提取一行里的字符串字面量内容（不处理原始字符串 `r#"..."#`）。
fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => {
                if in_string {
                    out.push(current.clone());
                    current.clear();
                }
                in_string = !in_string;
            }
            _ if in_string => current.push(c),
            _ => {}
        }
    }
    out
}

/// 是否包含整词（前后不是标识符字符）。
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c));
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

/// 扫描整个目录树，收集候选与未覆盖项。
pub fn scan_repo(mode: Mode) -> ScanReport {
    let root = scan_root(mode);
    let mut report = ScanReport::new(mode, root.clone());
    let owners = plugin_owners(&root);
    // 业务表名清单由插件 DDL 推导（框架层不得直接引用业务表）
    let tables = plugin_tables(&root.join("plugins"));
    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);
    files.sort();
    let mut other_macro_calls = 0usize;
    for path in files {
        let rel = path
            .strip_prefix(repo_root())
            .or_else(|_| path.strip_prefix(&root))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        let Ok(source) = std::fs::read_to_string(&path) else {
            *report
                .notes
                .entry(format!("无法读取文件：{rel}"))
                .or_insert(0) += 1;
            continue;
        };
        report.files += 1;
        report.bytes += source.len();
        if mode.wants_code() {
            report
                .candidates
                .extend(foreign_table_candidates(&rel, &source, &tables));
        }
        let mut outcome = scan_source(mode, &rel, &source, &owners);
        if outcome.parse_failed {
            *report
                .notes
                .entry(format!("解析失败（该文件的候选未纳入统计）：{rel}"))
                .or_insert(0) += 1;
        }
        report.candidates.append(&mut outcome.candidates);
        report.clone_count += outcome.clone_count;
        other_macro_calls += outcome.other_macro_calls;
        for note in outcome.notes {
            *report.notes.entry(note).or_insert(0) += 1;
        }
    }
    if other_macro_calls > 0 {
        *report
            .notes
            .entry(format!(
                "宏调用点未展开分析（自定义/派生宏 {other_macro_calls} 处）：宏体内候选不可见，需人工确认"
            ))
            .or_insert(0) += 1;
    }
    report.candidates.sort();
    report
}

/// 递归收集 `.rs` 文件。
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// 单文件扫描结果（夹具直接复用这条路径，保证夹具与真实扫描同源）。
#[derive(Debug, Default, Clone)]
pub struct SourceOutcome {
    /// 该文件产出的候选。
    pub candidates: Vec<Candidate>,
    /// 未覆盖/歧义说明。
    pub notes: Vec<String>,
    /// `.clone()` 计数。
    pub clone_count: usize,
    /// 未展开的第三方/自定义宏调用点数（报告汇总用，不按文件刷屏）。
    pub other_macro_calls: usize,
    /// 是否解析失败（失败时候选不完整，必须显式暴露）。
    pub parse_failed: bool,
}

/// 扫描一段源码。`owners` 为插件目录名，用于跨插件 import 判定。
pub fn scan_source(mode: Mode, rel_path: &str, source: &str, owners: &[String]) -> SourceOutcome {
    let file = match syn::parse_file(source) {
        Ok(f) => f,
        Err(_) => {
            return SourceOutcome {
                parse_failed: true,
                ..SourceOutcome::default()
            }
        }
    };
    let lines: Vec<&str> = source.lines().collect();
    let mut visitor = Visitor {
        mode,
        rel: rel_path,
        owners,
        lines: &lines,
        file_has_module_doc: file.attrs.iter().any(|a| a.path().is_ident("doc")),
        symbols: Vec::new(),
        in_trait_impl: Vec::new(),
        candidates: Vec::new(),
        notes: Vec::new(),
        other_macro_calls: 0,
        risky_macros: BTreeSet::new(),
        // 架构守卫只看生产前缀：文件内联测试的夹具不算违规
        production_lines: lines
            .iter()
            .position(|l| l.trim_start().starts_with("#[cfg(test)]"))
            .unwrap_or(lines.len()),
        risky_macro_calls: BTreeSet::new(),
        clone_count: 0,
    };
    visitor.visit_file(&file);
    let mut notes = visitor.notes;
    // 第三方/自定义宏的调用点无法靠 syn 展开：只汇总数量，具体宏体候选不可见。
    let other_macro_calls = visitor.other_macro_calls;
    for name in visitor.risky_macro_calls.iter() {
        notes.push(format!(
            "宏未展开：`{name}!` 调用点未展开分析（宏体内含潜在候选）"
        ));
    }
    SourceOutcome {
        candidates: visitor.candidates,
        notes,
        clone_count: visitor.clone_count,
        other_macro_calls,
        parse_failed: false,
    }
}

/// panic 类方法名 -> 细分类别。
pub const PANIC_METHODS: &[(&str, &str)] = &[
    ("unwrap", "unwrap"),
    ("expect", "expect"),
    ("unwrap_err", "unwrap_err"),
    ("expect_err", "unwrap_err"),
    ("unwrap_unchecked", "unwrap_unchecked"),
    ("expect_unchecked", "unwrap_unchecked"),
];

/// panic 类宏名 -> 细分类别。
pub const PANIC_MACROS: &[(&str, &str)] = &[
    ("panic", "panic"),
    ("unreachable", "unreachable"),
    ("todo", "todo"),
    ("unimplemented", "todo"),
    ("assert", "assert"),
    ("assert_eq", "assert"),
    ("assert_ne", "assert"),
    ("debug_assert", "assert_debug"),
    ("debug_assert_eq", "assert_debug"),
    ("debug_assert_ne", "assert_debug"),
];

/// 生命周期逃逸调用的路径末段集合（末两段或末一段匹配即命中）。
const ESCAPE_TAILS: &[&[&str]] = &[
    &["Box", "leak"],
    &["mem", "forget"],
    &["mem", "transmute"],
    &["mem", "transmute_copy"],
];

/// 访问器：一次遍历同时支撑代码规则与文档规则，避免两套解析口径漂移。
struct Visitor<'a> {
    mode: Mode,
    rel: &'a str,
    owners: &'a [String],
    lines: &'a [&'a str],
    /// 文件级属性里是否已有 `//!` 文档属性（只看 doc，`#![allow]`/`#![cfg_attr]` 不算）。
    file_has_module_doc: bool,
    symbols: Vec<String>,
    /// impl 栈：true = trait 实现（其方法不能 pub，不要求文档）。
    in_trait_impl: Vec<bool>,
    candidates: Vec<Candidate>,
    notes: Vec<String>,
    /// 未识别的宏调用点数（一次性汇总，避免逐条刷屏）。
    other_macro_calls: usize,
    /// 宏体内含潜在候选的自定义宏名。
    risky_macros: BTreeSet<String>,
    /// 上述宏的调用点。
    risky_macro_calls: BTreeSet<String>,
    /// `.clone()` 出现次数（仅报告，不判定）：clone 按数据规模与所有权评审，不数数量。
    clone_count: usize,
    /// 生产前缀行数：首个 `#[cfg(test)]` 标记之前（测试体内的代码不参与架构守卫）。
    production_lines: usize,
}

impl Visitor<'_> {
    /// 当前归属符号（模块/类型/函数链）。
    fn symbol(&self) -> String {
        if self.symbols.is_empty() {
            "<module>".to_string()
        } else {
            self.symbols.join("::")
        }
    }

    /// 记录一个候选。
    fn report(&mut self, rule: &'static str, kind: &'static str, line: usize, detail: String) {
        self.candidates.push(Candidate {
            rule,
            kind,
            path: self.rel.to_string(),
            symbol: self.symbol(),
            line,
            detail,
        });
    }

    /// 项自身的候选：归属符号取「父链 + 本项名」，与函数体内候选的归属口径一致。
    /// （体内候选在符号栈已含本项名时报出，所以这里必须把本项名补上。）
    fn report_item(
        &mut self,
        rule: &'static str,
        kind: &'static str,
        line: usize,
        detail: String,
        name: &str,
    ) {
        let symbol = if self.symbols.is_empty() {
            name.to_string()
        } else {
            format!("{}::{name}", self.symbols.join("::"))
        };
        self.candidates.push(Candidate {
            rule,
            kind,
            path: self.rel.to_string(),
            symbol,
            line,
            detail,
        });
    }

    /// 收集一行源码做证据片段。
    fn snippet(&self, line: usize) -> String {
        self.lines
            .get(line.saturating_sub(1))
            .map(|l| l.trim().chars().take(120).collect())
            .unwrap_or_default()
    }

    /// 判断属性的 cfg 结构：Some(true) = 字面 `#[cfg(test)]` 子树（跳过）；
    /// Some(false) = 含 test 但不作真假推断（保守扫描 + 记录）；None = 无关。
    fn cfg_test(&mut self, attrs: &[Attribute], line: usize) -> Option<bool> {
        for attr in attrs {
            if !attr.path().is_ident("cfg") {
                continue;
            }
            let text = match &attr.meta {
                syn::Meta::List(list) => list.tokens.to_string(),
                _ => continue,
            };
            if text == "test" {
                return Some(true);
            }
            if text.contains("test") {
                self.notes.push(format!(
                    "cfg 组合不作真假推断，已保守扫描：{}:{line} #[cfg({text})]",
                    self.rel
                ));
                return Some(false);
            }
        }
        None
    }

    /// `unsafe` 上方是否有紧邻的 `// SAFETY:` 论证（允许中间夹属性行与空行）。
    fn safety_note_above(&self, line: usize) -> bool {
        let mut i = line as isize - 2;
        let mut skipped = 0usize;
        while i >= 0 {
            let t = self.lines[i as usize].trim();
            if t.starts_with("//") {
                if t.contains("SAFETY:") {
                    return true;
                }
                i -= 1;
                continue;
            }
            if t.is_empty() || t.starts_with('#') || t.starts_with(')') || t.starts_with(']') {
                i -= 1;
                continue;
            }
            // 多行属性的续行（`feature = "x",`）允许有限跳过，避免误报。
            if t.ends_with(',') && skipped < 10 {
                skipped += 1;
                i -= 1;
                continue;
            }
            return false;
        }
        false
    }

    /// unsafe 检查：缺紧邻 SAFETY 论证即候选（零容忍）。
    fn check_unsafe(&mut self, line: usize, detail: &str) {
        if !self.mode.wants_code() {
            return;
        }
        if !self.safety_note_above(line) {
            self.report(
                "unsafe_without_safety",
                "unsafe_without_safety",
                line,
                format!("{detail}（上方无紧邻 `// SAFETY:` 论证）"),
            );
        }
    }

    /// 文档规则：`///` 缺失即报 api_doc（调用方已判定该项要求文档）。
    fn require_doc(&mut self, attrs: &[Attribute], line: usize, what: &str, name: &str) {
        if !self.mode.wants_docs() || has_doc_attr(attrs) {
            return;
        }
        self.report_item(
            "api_doc",
            "api_doc",
            line,
            format!("{what} `{name}` 缺少 /// 职责注释"),
            name,
        );
    }

    /// 文档规则：按可见性判定是否要求 `///`（pub / pub(crate) / pub(super) 要求）。
    fn check_api_doc(
        &mut self,
        attrs: &[Attribute],
        vis: &Visibility,
        line: usize,
        what: &str,
        name: &str,
    ) {
        if vis_requires_doc(vis) {
            self.require_doc(attrs, line, what, name);
        }
    }

    /// 文档规则：文件必须带 `//!`（每个文件检查一次）。
    fn check_file_doc(&mut self) {
        if !self.mode.wants_docs() || self.file_has_module_doc {
            return;
        }
        self.report(
            "module_doc",
            "module_doc",
            1,
            "缺少 //! 文件头（模块职责）注释".to_string(),
        );
    }

    /// 层级依赖：`crate::plugins::<owner>` 的引用是否越界。
    fn check_layer(&mut self, segments: &[String], line: usize, detail: &str) {
        if segments.len() < 3 || segments[0] != "crate" || segments[1] != "plugins" {
            return;
        }
        let owner = segments[2].clone();
        // 仅在插件目录名命中时才判定越界，避免把 plugins/mod.rs、plugins::ipc_registry 误判成插件。
        if !self.owners.contains(&owner) {
            return;
        }
        // 候选与例外登记都用仓库锚定路径（src-tauri/src/…）；层级判定只看 src 内的相对位置。
        let scoped = self.rel.strip_prefix("src-tauri/src/").unwrap_or(self.rel);
        if scoped.starts_with("framework/") {
            self.report(
                "layer_violation",
                "layer_violation",
                line,
                format!("framework 不得引用插件 {owner}（{detail}）"),
            );
            return;
        }
        let current = scoped
            .strip_prefix("plugins/")
            .and_then(|r| r.split('/').next())
            .map(str::to_string);
        if let Some(current) = current {
            if current != owner && self.owners.contains(&current) {
                self.report(
                    "layer_violation",
                    "layer_violation",
                    line,
                    format!("插件 {current} 不得 import 插件 {owner} 内部（{detail}）"),
                );
            }
        }
    }
}

/// 该可见性是否要求文档（pub / pub(crate) / pub(super) / pub(self)）。
fn vis_requires_doc(vis: &Visibility) -> bool {
    match vis {
        Visibility::Public(_) => true,
        Visibility::Restricted(r) => matches!(
            r.path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .as_deref(),
            Some("crate") | Some("super") | Some("self")
        ),
        Visibility::Inherited => false,
    }
}

/// 是否有 `///` / `//!` 文档属性。
fn has_doc_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("doc"))
}

/// 取 span 的 1 起始行号。
fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

/// 路径是否为禁止的生命周期逃逸调用。
fn check_escape_path(visitor: &mut Visitor<'_>, path: &syn::Path, line: usize) {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let is_escape = ESCAPE_TAILS.iter().any(|tail| {
        segments.len() >= tail.len() && segments[segments.len() - tail.len()..] == **tail
    });
    if is_escape {
        visitor.report(
            "lifetime_escape",
            "lifetime_escape",
            line,
            format!("禁止调用 `{}`", segments.join("::")),
        );
    }
}

/// 把 `use` 的树形结构摊平成「分段路径 + 行号」，供层级规则判定。
///
/// `use crate::plugins::{ssh::x, dns::y};` 会摊平成两条 `crate::plugins::…` 路径；
/// 分组的花括号本身不产生候选。
fn flatten_use_tree(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, proc_macro2::LineColumn)>,
) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten_use_tree(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut segments = prefix.clone();
            segments.push(n.ident.to_string());
            out.push((segments, n.ident.span().start()));
        }
        syn::UseTree::Rename(r) => {
            let mut segments = prefix.clone();
            segments.push(r.ident.to_string());
            out.push((segments, r.ident.span().start()));
        }
        syn::UseTree::Glob(g) => {
            let mut segments = prefix.clone();
            segments.push("*".to_string());
            out.push((segments, g.star_token.span.start()));
        }
        syn::UseTree::Group(g) => {
            for item in g.items.iter() {
                flatten_use_tree(item, prefix, out);
            }
        }
    }
}

/// 取类型名（impl 的 self 类型），用于符号前缀。
fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "<impl>".to_string()),
        _ => "<impl>".to_string(),
    }
}

impl<'ast> Visit<'ast> for Visitor<'_> {
    fn visit_file(&mut self, node: &'ast syn::File) {
        self.check_file_doc();
        visit::visit_file(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        // `mod` 声明的职责由目标文件的 `//!` 文件头承担（module_doc 规则），
        // 这里不重复要求 ///，避免同一个职责写两处、把 60+ 个声明刷成噪声。
        self.symbols.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.symbols.pop();
    }

    /// impl 内的方法走 ImplItemFn（与自由函数是不同的 AST 类型）：
    /// 固有 impl 的 pub 方法算公开 API，trait impl 的方法不算（可见性由 trait 决定）。
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let line = line_of(node.sig.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.sig.ident.to_string();
        if !self.in_trait_impl.last().copied().unwrap_or(false) {
            self.check_api_doc(&node.attrs, &node.vis, line, "方法", &name);
        }
        if let Some(tok) = &node.sig.unsafety {
            self.check_unsafe(line_of(tok.span()), "unsafe 方法声明");
        }
        self.symbols.push(name);
        visit::visit_impl_item_fn(self, node);
        self.symbols.pop();
    }

    /// trait 方法构成公开契约，无论是否有默认实现都要求文档。
    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        let line = line_of(node.sig.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.sig.ident.to_string();
        self.require_doc(&node.attrs, line, "trait 方法", &name);
        self.symbols.push(name);
        visit::visit_trait_item_fn(self, node);
        self.symbols.pop();
    }

    /// 使用树（`use` 语句）里的路径不是 `syn::Path`，层级规则必须单独处理，
    /// 否则 `use crate::plugins::ssh::…` 这类越界会整类漏掉。
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        if self.mode.wants_code() {
            let mut prefix = Vec::new();
            let mut flat = Vec::new();
            flatten_use_tree(&node.tree, &mut prefix, &mut flat);
            for (segments, lc) in flat {
                let detail = self.snippet(lc.line);
                self.check_layer(&segments, lc.line, &detail);
            }
        }
        visit::visit_item_use(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let line = line_of(node.sig.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.sig.ident.to_string();
        if !self.in_trait_impl.last().copied().unwrap_or(false) {
            self.check_api_doc(&node.attrs, &node.vis, line, "函数", &name);
        }
        if let Some(tok) = &node.sig.unsafety {
            let uline = line_of(tok.span());
            self.check_unsafe(uline, "unsafe fn 声明");
        }
        self.symbols.push(name);
        visit::visit_item_fn(self, node);
        self.symbols.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let line = line_of(node.self_ty.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        if let Some(tok) = &node.unsafety {
            let uline = line_of(tok.span());
            self.check_unsafe(uline, "unsafe impl 声明");
        }
        self.symbols.push(type_name(&node.self_ty));
        self.in_trait_impl.push(node.trait_.is_some());
        visit::visit_item_impl(self, node);
        self.in_trait_impl.pop();
        self.symbols.pop();
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "trait", &name);
        if let Some(tok) = &node.unsafety {
            let uline = line_of(tok.span());
            self.check_unsafe(uline, "unsafe trait 声明");
        }
        self.symbols.push(name);
        visit::visit_item_trait(self, node);
        self.symbols.pop();
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "结构体", &name);
        if self.mode.wants_docs() && vis_requires_doc(&node.vis) {
            if let syn::Fields::Named(named) = &node.fields {
                let mut missing: Vec<(usize, String)> = Vec::new();
                for field in named.named.iter() {
                    if self.cfg_test(&field.attrs, line_of(field.span())) == Some(true) {
                        continue;
                    }
                    if !has_doc_attr(&field.attrs) {
                        let fname = field
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| "<字段>".to_string());
                        missing.push((line_of(field.span()), fname));
                    }
                }
                for (fline, fname) in missing {
                    self.report_item(
                        "field_doc",
                        "field_doc",
                        fline,
                        format!("结构体 {name} 的字段 `{fname}` 缺少 /// 语义注释（IPC/持久化 DTO 字段必须有）"),
                        &format!("{name}::{fname}"),
                    );
                }
            }
        }
        self.symbols.push(name);
        visit::visit_item_struct(self, node);
        self.symbols.pop();
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "枚举", &name);
        self.symbols.push(name);
        visit::visit_item_enum(self, node);
        self.symbols.pop();
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "类型别名", &name);
        visit::visit_item_type(self, node);
    }

    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "常量", &name);
        visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "静态量", &name);
        visit::visit_item_static(self, node);
    }

    fn visit_item_union(&mut self, node: &'ast syn::ItemUnion) {
        let line = line_of(node.ident.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        let name = node.ident.to_string();
        self.check_api_doc(&node.attrs, &node.vis, line, "union", &name);
        visit::visit_item_union(self, node);
    }

    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        let line = line_of(node.span());
        if self.cfg_test(&node.attrs, line) == Some(true) {
            return;
        }
        // macro_rules! 定义体是 token 流，syn 不展开：单独提示，不假装已覆盖。
        if let Some(ident) = &node.ident {
            // 宏体经 token 流重新拼接后带空格（`$e . expect ("boom")`），先归一化再匹配。
            let body = node.mac.tokens.to_string().replace(' ', "");
            let hits: Vec<&str> = PANIC_MACROS
                .iter()
                .map(|(m, _)| *m)
                .chain(PANIC_METHODS.iter().map(|(m, _)| *m))
                .filter(|name| {
                    body.contains(&format!("{name}!")) || body.contains(&format!(".{name}("))
                })
                .collect();
            if !hits.is_empty() {
                self.risky_macros.insert(ident.to_string());
                self.notes.push(format!(
                    "宏未展开：`{ident}` 的宏体内出现 {}，需人工确认其展开结果",
                    hits.join("、")
                ));
            }
        }
        visit::visit_item_macro(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        let line = line_of(node.path.span());
        if let Some((_, kind)) = PANIC_MACROS.iter().find(|(m, _)| *m == name.as_str()) {
            if self.mode.wants_code() {
                let detail = format!("宏 `{name}!` 是潜在 panic 入口：{}", self.snippet(line));
                self.report("panic", kind, line, detail);
            }
        } else if name != "macro_rules" {
            if self.risky_macros.contains(&name) {
                self.risky_macro_calls.insert(name);
            } else {
                self.other_macro_calls += 1;
            }
        }
        visit::visit_macro(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        if name == "clone" {
            self.clone_count += 1;
        }
        if let Some((_, kind)) = PANIC_METHODS.iter().find(|(m, _)| *m == name.as_str()) {
            if self.mode.wants_code() {
                let line = line_of(node.method.span());
                let detail = format!("`{name}` 直接 panic：{}", self.snippet(line));
                self.report("panic", kind, line, detail);
            }
        }
        // 架构守卫：落盘根必须经 framework::paths 解析（自举文件除外）
        let method_line = line_of(node.method.span());
        if self.mode.wants_code()
            && PATHS_BYPASS_METHODS.contains(&name.as_str())
            && !is_paths_bootstrap(self.rel)
            && !is_test_only_file(self.rel)
            && method_line <= self.production_lines
        {
            let detail = format!("`{name}` 直接取落盘根：{}", self.snippet(method_line));
            self.report("paths_bypass", "paths_bypass", method_line, detail);
        }
        visit::visit_expr_method_call(self, node);
    }

    /// 生命周期逃逸只看路径表达式：`Box::leak(..)` 的 func 本身就是路径表达式，
    /// 再在 visit_expr_call 里判一次会重复计数。
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if self.mode.wants_code() {
            let line = line_of(node.path.span());
            check_escape_path(self, &node.path, line);
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        let line = line_of(Spanned::span(&node.unsafe_token));
        self.check_unsafe(line, "unsafe 块");
        visit::visit_expr_unsafe(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        if self.mode.wants_code() {
            let segments: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
            let line = line_of(node.span());
            let detail = self.snippet(line);
            self.check_layer(&segments, line, &detail);
        }
        visit::visit_path(self, node);
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        // match 分支上的 #[cfg(test)] 只影响该分支，按字面排除，不截断整个后续。
        if self.cfg_test(&node.attrs, line_of(node.span())) == Some(true) {
            return;
        }
        visit::visit_arm(self, node);
    }
}

/// 便捷入口：扫一次并套用基线（真实仓库用）。
pub fn scan_with_baseline(mode: Mode, baseline: &Baseline) -> ScanReport {
    let mut report = scan_repo(mode);
    report.apply_exceptions(baseline);
    report
}

/// 夹具入口：对一段源码跑同一套规则，返回排序后的候选（不套基线）。
pub fn candidates_for(mode: Mode, rel: &str, source: &str, owners: &[String]) -> Vec<Candidate> {
    let mut out = scan_source(mode, rel, source, owners).candidates;
    out.sort();
    out
}

/// 夹具入口：对一段源码套用基线，返回报告（含失效例外）。
pub fn report_for(
    mode: Mode,
    rel: &str,
    source: &str,
    owners: &[String],
    baseline: &Baseline,
) -> ScanReport {
    let outcome = scan_source(mode, rel, source, owners);
    let mut report = ScanReport::new(mode, PathBuf::from("<fixture>"));
    report.files = 1;
    report.candidates = outcome.candidates;
    report.clone_count = outcome.clone_count;
    for note in outcome.notes {
        *report.notes.entry(note).or_insert(0) += 1;
    }
    report.candidates.sort();
    report.apply_exceptions(baseline);
    report
}
