//! frp 插件 · 配置校验（调 `frpc verify -c <path>`，输出解析为结构化错误列表）
//! 分层：纯函数解析（可单测、不依赖真实 frpc）与进程封装分离——`parse_verify_output` 只吃字符串，
//! `run_verify` 才碰真实进程。解析必须能降级：frpc 各版本文案不一致时，宁可把原始输出作为一条错误
//! 抛出，也不静默吞掉（任务书 §10 风险对策）。

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use crate::plugins::frp::models::{FrpVerifyError, FrpVerifyResult};

/// 校验超时（秒）：frpc verify 只解析本地文件，正常毫秒级返回；卡死视为环境异常
const VERIFY_TIMEOUT_SECS: u64 = 15;

/// 错误行关键字（小写匹配）：命中即视为一条错误；frpc 日志等级标记 `[E]` / `[F]` 也计入
const ERROR_MARKERS: &[&str] = &[
    "error",
    "invalid",
    "incorrect",
    "failed",
    "failure",
    "unable to",
    "cannot",
    "unknown",
    "expected",
    "fatal",
    "[e]",
    "[f]",
];

/// 剥除 ANSI 转义序列（frpc 带色输出；不去掉会污染错误信息与关键字匹配）
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // ESC 后紧跟 `[` 即 CSI 序列：吞到第一个位于 @~ 区间的终止字符
        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }
    out
}

/// 取关键字后紧跟的十进制数字（如 `line 3` / `column 5`）；无数字返回 None
fn number_after(text: &str, keyword: &str) -> Option<u32> {
    let index = text.find(keyword)?;
    let rest = &text[index + keyword.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace() || *c == ':' || *c == '=')
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok()
}

/// 提取一行里的「行 / 列」位置：只认 `line N[, column M]` 这种显式写法。
/// 刻意不认 `N:M`——frpc 日志前缀自带 `12:00:00` 时间戳，按冒号解析会误报成行列。
fn position_of(line: &str) -> (Option<u32>, Option<u32>) {
    let lower = line.to_ascii_lowercase();
    (number_after(&lower, "line"), number_after(&lower, "column"))
}

/// 解析 frpc verify 输出为结构化错误（纯函数）。
/// 规则：① 命中错误关键字或带 `line N` 的行 → 一条错误（能取到位置就带行列）；
/// ② 输出非空但一条都没认出来且退出码非 0 → 降级为「最后一行原文」单条；
/// ③ 空输出且退出失败 → 显式错误；仅成功退出时返回空列表。
pub fn parse_verify_output(raw: &str, exit_ok: bool) -> Vec<FrpVerifyError> {
    let cleaned = strip_ansi(raw);
    let lines: Vec<&str> = cleaned
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return if exit_ok {
            Vec::new()
        } else {
            vec![FrpVerifyError {
                line: None,
                column: None,
                message: "frpc verify 失败（无输出）".into(),
            }]
        };
    }
    let mut errors = Vec::new();
    for line in &lines {
        let lower = line.to_ascii_lowercase();
        let (line_no, column) = position_of(line);
        let matched = ERROR_MARKERS.iter().any(|marker| lower.contains(marker));
        if matched || line_no.is_some() {
            errors.push(FrpVerifyError {
                line: line_no,
                column,
                message: line.to_string(),
            });
        }
    }
    if errors.is_empty() && !exit_ok {
        // 降级：版本文案变了也别吞掉原因，把最后一行原文当唯一错误交给用户判读
        let tail = lines
            .last()
            .copied()
            .unwrap_or("frpc verify 失败（无输出）")
            .to_string();
        errors.push(FrpVerifyError {
            line: None,
            column: None,
            message: tail,
        });
    }
    errors
}

/// 文本归一化（日志行与状态行共用）：剥离 ANSI 转义 + 敏感值打码。
///
/// `runtime.rs` 推送的 `frp://log` 与写入 `lastError` 的文本都经此处理：
/// 避免终端配色残留，也避免 token 与密码随错误提示外泄。
pub fn clean_line(text: &str) -> String {
    mask_secrets(&strip_ansi(text))
}

/// 敏感值打码：仅当关键字后的取值「看起来像密钥」时才替换为 `***`。
///
/// 判定规则：跳过空白与分隔符后，取值长度 ≥ 6 且以空白 / 逗号 / 引号 / 右花括号终止。
/// 这样 `auth.token = "s3cr3t"`、`--token abc123`、`password: hunter2` 会被打码，
/// 而 frp 的自然语句提示（`token is incorrect`）保持原文，不把错误信息本身打废。
fn mask_secrets(text: &str) -> String {
    /// 触发打码的关键字（小写比较）
    const SECRET_KEYS: [&str; 4] = ["token", "password", "passwd", "secret"];
    /// 取值最短长度（短于此长度的连续词按普通文本处理）
    const MIN_SECRET_LEN: usize = 6;

    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while cursor < text.len() {
        let mut hit: Option<(usize, &str)> = None;
        for key in SECRET_KEYS {
            if let Some(pos) = lower[cursor..].find(key) {
                let absolute = cursor + pos;
                if hit.is_none_or(|(best, _)| absolute < best) {
                    hit = Some((absolute, key));
                }
            }
        }
        let Some((start, key)) = hit else {
            out.push_str(&text[cursor..]);
            break;
        };
        let after = start + key.len();
        out.push_str(&text[cursor..after]);
        let rest = &text[after..];
        let trimmed = rest.trim_start_matches([' ', '\t', '=', ':', '"', '\'']);
        let value_start = after + (rest.len() - trimmed.len());
        let tail = &text[value_start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == ',' || c == '"' || c == '\'' || c == '}')
            .unwrap_or(tail.len());
        if end < MIN_SECRET_LEN {
            // 短词按普通文本处理：从关键字末尾继续，保留中间的空格等原字符
            cursor = after;
            continue;
        }
        out.push_str("***");
        cursor = value_start + end;
    }
    out
}

/// 拼装校验结果（纯函数）：`ok` 取 frpc 退出码，`raw` 由调用方脱敏后传入。
pub fn build_verify_result(file_name: &str, raw: &str, exit_ok: bool) -> FrpVerifyResult {
    FrpVerifyResult {
        ok: exit_ok,
        file_name: file_name.to_string(),
        raw: raw.to_string(),
        errors: parse_verify_output(raw, exit_ok),
    }
}

/// 执行 `frpc verify -c <path>`：stdout/stderr 合并返回，附「是否退出码 0」。
/// 只读操作；不弹控制台窗口（Windows）；超时即返回可读错误，不 panic。
pub(super) async fn run_verify(
    exe: &Path,
    config: &super::auth::PreparedConfig,
) -> Result<(String, bool), String> {
    let mut cmd = tokio::process::Command::new(exe);
    config.configure(&mut cmd);
    cmd.arg("verify")
        .arg("-c")
        .arg(&config.path)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Windows：CREATE_NO_WINDOW，避免校验时闪出控制台窗口
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let output = tokio::time::timeout(Duration::from_secs(VERIFY_TIMEOUT_SECS), cmd.output())
        .await
        .map_err(|_| {
            format!("校验超时（超过 {VERIFY_TIMEOUT_SECS} 秒），请手动执行 frpc verify 排查")
        })?
        .map_err(|e| format!("启动 frpc 失败: {e}"))?;
    let mut merged = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !merged.trim().is_empty() {
            merged.push('\n');
        }
        merged.push_str(&stderr);
    }
    Ok((config.redact(&merged), output.status.success()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 带行列的失败样本（go-toml 风格，frpc 解析错误常见形态）
    const RAW_WITH_POSITION: &str =
        "\u{1b}[31m2026-09-12 10:00:00.000 [E] [config.go:110] \u{1b}[0m\n\
        invalid configuration: toml: line 3, column 5: expected a value";

    /// 无行列的失败样本（服务端/运行期类错误） + 一条多错误
    const RAW_WITHOUT_POSITION: &str = "token is incorrect\nfailed to connect to server";

    #[test]
    fn parses_positioned_error() {
        let errors = parse_verify_output(RAW_WITH_POSITION, false);
        assert_eq!(errors.len(), 2, "两行都命中关键字: {errors:?}");
        // 首行只有 `[E]` 标记与时间戳，无 line N → 无行列
        assert_eq!(errors[0].line, None);
        assert_eq!(errors[0].column, None);
        assert!(
            errors[0].message.starts_with("2026-09-12"),
            "ANSI 应被剥除: {:?}",
            errors[0].message
        );
        // 次行是 toml 解析错误 → 行列齐全
        assert_eq!(errors[1].line, Some(3));
        assert_eq!(errors[1].column, Some(5));
    }

    #[test]
    fn parses_multiple_errors_without_position() {
        let errors = parse_verify_output(RAW_WITHOUT_POSITION, false);
        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .all(|e| e.line.is_none() && e.column.is_none()));
        assert_eq!(errors[0].message, "token is incorrect");
    }

    #[test]
    fn unparseable_output_degrades_to_single_error() {
        // 无法识别的文案（不含任何错误标记）+ 退出码非 0 → 降级为最后一行单条错误，绝不静默
        let errors = parse_verify_output("hello world\n第二行", false);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "第二行");
        // 退出码 0 且无可识别错误 → 空列表
        assert!(parse_verify_output("load config from file: a.toml", true).is_empty());
        // 完全空输出 → 空列表（视为 ok）
        assert!(parse_verify_output("", true).is_empty());
        assert_eq!(
            parse_verify_output("   \n  ", false)[0].message,
            "frpc verify 失败（无输出）"
        );
    }

    #[test]
    fn build_result_keeps_raw_and_ok_flag() {
        let result = build_verify_result("a.toml", RAW_WITH_POSITION, false);
        assert!(!result.ok);
        assert_eq!(result.file_name, "a.toml");
        assert!(
            result.raw.contains("invalid configuration"),
            "raw 必须原样返回"
        );
        let ok = build_verify_result("b.toml", "", true);
        assert!(ok.ok && ok.errors.is_empty());
    }

    #[test]
    fn strip_ansi_removes_color_sequences() {
        assert_eq!(strip_ansi("\u{1b}[31mred\u{1b}[0m"), "red");
        assert_eq!(strip_ansi("no-escape"), "no-escape");
        assert_eq!(strip_ansi("a\u{1b}[1;32mb\u{1b}[0mc"), "abc");
    }
}
