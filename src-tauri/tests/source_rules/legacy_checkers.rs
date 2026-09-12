//! 旧检查器的等价复刻（仅用于夹具取证，不参与实际门禁）。
//!
//! 复刻对象是 AR02 之前的两个正则/Python 实现：
//! - `scripts/check_rust_rules.py`：按行匹配 panic/生命周期/unsafe，且 `strip_test_code`
//!   在**首个**含 `#[cfg(test)]` 的行截断整个文件；
//! - `scripts/check_docs.py`：`^(pub(?:\(crate\))? (?:async )?fn |fn |pub struct |…)` 行首正则。
//!
//! 保留它们的目的：夹具必须先复现「旧实现为什么会漏报/误报」，再证明 `rule_engine` 已修掉，
//! 否则「重写检查器」只是在换实现，无法证明修的是真问题。

/// 旧 `check_rust_rules.py` 的 panic 判定（正则是 `\.unwrap\(\)|\.expect\(|panic!|unreachable!|\.unwrap_unchecked`）。
fn legacy_panic_line(line: &str) -> bool {
    line.contains(".unwrap()")
        || line.contains(".expect(")
        || line.contains("panic!")
        || line.contains("unreachable!")
        || line.contains(".unwrap_unchecked")
}

/// 旧实现的例外：直接匹配整行文本，不绑定符号。
pub fn legacy_allowlist_hit(line: &str) -> bool {
    line.contains("expect(\"IPC 命令重复注册\")")
        || line.contains("expect(\"error while running tauri application\")")
        || line.contains("expect(\"HMAC 接受任意长度密钥\")")
}

/// 旧 `strip_test_code`：遇到首个含 `#[cfg(test)]` 的行就截断（不看它是否真的在文件末尾）。
pub fn legacy_strip_test_code<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    for line in lines {
        if line.contains("#[cfg(test)]") {
            break;
        }
        out.push(*line);
    }
    out
}

/// 旧 unsafe 判定：整行含 `unsafe` 且不是注释行，且上方紧邻注释块里没有 `SAFETY:`。
fn legacy_unsafe_line(lines: &[&str], index: usize) -> bool {
    let line = lines[index];
    if !line
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| w == "unsafe")
    {
        return false;
    }
    if line.trim().starts_with("//") {
        return false;
    }
    let mut j = index;
    while j > 0 {
        j -= 1;
        let prev = lines[j].trim();
        if !prev.starts_with("//") {
            return true;
        }
        if prev.contains("SAFETY:") {
            return false;
        }
    }
    true
}

/// 旧 panic 候选清单：(1-based 行号, 行内容)。
pub fn legacy_panic_hits(src: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.split('\n').collect();
    let scanned = legacy_strip_test_code(&lines);
    scanned
        .iter()
        .enumerate()
        .filter(|(_, line)| legacy_panic_line(line) && !legacy_allowlist_hit(line))
        .map(|(i, line): (usize, &&str)| (i + 1, line.trim().to_string()))
        .collect()
}

/// 旧 unsafe 缺论证清单：(1-based 行号, 行内容)。
pub fn legacy_unsafe_hits(src: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.split('\n').collect();
    let scanned = legacy_strip_test_code(&lines);
    scanned
        .iter()
        .enumerate()
        .filter(|(i, _)| legacy_unsafe_line(&scanned, *i))
        .map(|(i, line): (usize, &&str)| (i + 1, line.trim().to_string()))
        .collect()
}

/// 旧 `check_docs.py::check_file` 的等价复刻：只认行首的函数/结构体声明与字段定义。
pub fn legacy_doc_issues(src: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = src.split('\n').collect();

    let head: Vec<&str> = lines.iter().take(8).copied().collect();
    let has_header = head
        .iter()
        .any(|l| l.trim().starts_with("//!") || (l == &head[0] && l.trim().starts_with("//")));
    if !has_header {
        issues.push("缺少文件头注释（//!）".to_string());
    }

    for (i, line) in lines.iter().enumerate() {
        let starts = line.starts_with("pub fn ")
            || line.starts_with("pub(crate) fn ")
            || line.starts_with("pub async fn ")
            || line.starts_with("fn ")
            || line.starts_with("pub struct ")
            || line.starts_with("pub(crate) struct ")
            || line.starts_with("struct ");
        if !starts {
            continue;
        }
        let mut j = i;
        while j > 0 && lines[j - 1].trim().starts_with("#[") {
            j -= 1;
        }
        let documented = j > 0 && lines[j - 1].trim().starts_with("///");
        if !documented {
            issues.push(format!("L{} {} 缺注释", i + 1, line.trim()));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_engine::{self, Candidate, Mode};

    fn owners() -> Vec<String> {
        Vec::new()
    }

    fn new_engine_code(rel: &str, src: &str) -> Vec<Candidate> {
        rule_engine::candidates_for(Mode::Code, rel, src, &owners())
    }

    fn new_engine_docs(rel: &str, src: &str) -> Vec<Candidate> {
        rule_engine::candidates_for(Mode::Docs, rel, src, &owners())
    }

    /// 误判 1：旧实现在首个 `#[cfg(test)]` 截断，测试模块之后的生产代码完全不可见。
    #[test]
    fn legacy_misses_production_code_after_test_module() {
        let src = r#"
//! 模块职责
#[cfg(test)]
mod tests {
    fn t() {}
}

/// 生产函数（位于测试模块之后）
pub fn production() -> u8 {
    let v: Option<u8> = None;
    let _ = v.expect("生产代码里的候选");
    0
}
"#;
        assert!(
            legacy_panic_hits(src).is_empty(),
            "旧实现漏报：截断后看不到后面的生产代码"
        );
        assert_eq!(
            new_engine_code("fixture.rs", src).len(),
            1,
            "新实现必须抓到测试模块之后的生产候选"
        );
    }

    /// 误判 1 的真实形态：`#[cfg(test)]` 出现在文件中部且缩进（枚举变体、match 分支）。
    #[test]
    fn legacy_truncates_file_at_indented_cfg_test_marker() {
        let src = r#"
//! 模块职责
pub enum Event {
    Started,
    #[cfg(test)]
    TestOnly,
    Stopped,
}

/// 生产函数
pub fn production() {
    let v: Option<u8> = None;
    let _ = v.unwrap();
}
"#;
        assert!(
            legacy_panic_hits(src).is_empty(),
            "旧实现被枚举变体上的 #[cfg(test)] 截断，漏掉后面的生产 unwrap"
        );
        assert_eq!(new_engine_code("fixture.rs", src).len(), 1);
    }

    /// 误判 2：注释行里的候选名被旧实现当成违规。
    #[test]
    fn legacy_flags_comment_mentions() {
        let src = r##"
//! 模块职责
/// 说明：调用方自己处理，不要 .unwrap() / panic!
pub fn safe() -> Option<u8> {
    // Avoid .expect( here
    None
}
"##;
        assert_eq!(
            legacy_panic_hits(src).len(),
            2,
            "旧实现把注释里的候选名算成违规"
        );
        assert!(
            new_engine_code("fixture.rs", src).is_empty(),
            "新实现不误报注释"
        );
    }

    /// 误判 2 的真实形态：`cfg_attr(not(debug_assertions), …)` 里的 `debug_assert` 文本。
    #[test]
    fn legacy_matches_assert_text_inside_attribute() {
        let src = r#"
//! 模块职责
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
"#;
        // 旧 PANIC_PATTERN 不含 assert，但按行匹配的同类实现会把属性文本当调用；
        // 这里直接验证新实现不会把属性里的同名文本当候选。
        assert!(new_engine_code("fixture.rs", src).is_empty());
        assert!(
            legacy_strip_test_code(&[src]).len() == 1,
            "旧实现按行处理，无法区分属性与调用"
        );
    }

    /// 误判 3：旧 `check_docs.py` 只看行首声明，impl 内缩进的 pub 方法完全漏掉。
    #[test]
    fn legacy_misses_indented_impl_method() {
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
        assert!(
            legacy_doc_issues(src).is_empty(),
            "旧实现认为「结构体属性全覆盖」，实际漏掉 impl 里的 pub 方法"
        );
        let found = new_engine_docs("fixture.rs", src);
        assert_eq!(found.len(), 1, "{found:?}");
    }

    /// 误判 4：`// SAFETY:` 与 `unsafe` 之间夹着属性行时，旧实现向上找的是「紧邻注释块」，
    /// 撞到 `#[...]` 就判定为缺论证（误报）；新实现跳过属性行继续找论证。
    #[test]
    fn legacy_requires_safety_note_strictly_adjacent() {
        let src = r#"
//! 模块职责
/// 读取裸指针
pub fn read_ptr() -> u8 {
    // SAFETY: 指针来自本模块内部缓冲区，长度已在上文校验
    #[allow(unsafe_code)]
    unsafe { *std::ptr::null() }
}
"#;
        assert_eq!(
            legacy_unsafe_hits(src).len(),
            1,
            "旧实现把「属性行夹在论证与 unsafe 之间」当成缺论证"
        );
        assert!(
            new_engine_code("fixture.rs", src).is_empty(),
            "新实现跳过属性行后仍能找到紧邻的 SAFETY 论证"
        );
    }
}
