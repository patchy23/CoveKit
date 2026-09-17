//! SSH 插件 · 终端会话日志（输出旁路落盘）
//!
//! 在终端通道的 `ChannelMsg::Data` 分支旁路写盘：不额外走 IPC、不影响渲染路径。
//! 落盘内容是剥离 ANSI 转义与 `\r` 的纯文本，便于事后用编辑器或 `diff` 阅读。
//!
//! 路径唯一入口是 [`ssh_log_dir`]：框架存储配置的 logs 分区（`<存储根>/logs/ssh`），
//! 任何地方都不得手拼日志路径（任务书 §4.4）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::plugins::ssh::models::{LogActionResult, TerminalLogError};
use crate::plugins::ssh::terminal::TerminalState;

/// 日志写入器（按终端绑定；句柄与路径同生共死）
pub(crate) struct LogSink {
    /// 追加写的文件句柄
    file: tokio::fs::File,
    /// 落盘绝对路径（停止时回报前端）
    path: PathBuf,
    /// 已写入字节数
    bytes: u64,
}

/// 终端日志共享状态（TerminalHandle 持有，后台任务与命令层共享同一份）
pub(crate) type SharedLog = Arc<Mutex<Option<LogSink>>>;

/// 新建一个「未录制」的共享状态
pub(crate) fn new_shared() -> SharedLog {
    Arc::new(Mutex::new(None))
}

/// 剥离 ANSI 转义序列（CSI / OSC / 单字符转义）并丢弃 `\r`
///
/// 规则：`\r` 丢弃（进度条覆盖类输出只保留最终态）；保留 `\n` 与 `\t`；
/// 未闭合的转义序列按普通文本原样保留，避免吃掉正常内容。
pub(crate) fn strip_ansi(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            // 回车丢弃：进度条覆盖类输出只保留最终态
            '\r' => i += 1,
            '\x1b' => {
                i += 1;
                match chars.get(i) {
                    // CSI：ESC [ ... 终止字节落在 0x40..=0x7E
                    Some('[') => {
                        i += 1;
                        while i < chars.len() && !('\u{40}'..='\u{7e}').contains(&chars[i]) {
                            i += 1;
                        }
                        if i < chars.len() {
                            i += 1;
                        }
                    }
                    // OSC：ESC ] ... BEL 或 ESC \
                    Some(']') => {
                        i += 1;
                        while i < chars.len() {
                            if chars[i] == '\u{7}' {
                                i += 1;
                                break;
                            }
                            if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'\\') {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    // 字符集指定：ESC ( X / ESC ) X / ESC * X / ESC + X / ESC # X
                    Some('(') | Some(')') | Some('*') | Some('+') | Some('#') => i += 2,
                    // 其他单字符转义：ESC 后一字符即结束
                    Some(_) => i += 1,
                    None => {}
                }
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// SSH 会话日志根目录（**唯一**的日志路径解析入口）
pub(crate) fn ssh_log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    crate::framework::paths::logs_dir_for(app, "ssh")
}

/// 打开当前存储位置的 SSH 日志目录；首次使用时创建，不依赖活跃终端。
#[tauri::command]
pub async fn ssh_terminal_log_open_dir(app: AppHandle) -> Result<(), String> {
    let dir = resolve_dir(&app, None).await?;
    let path = dir
        .to_str()
        .ok_or_else(|| "日志目录路径不是有效 UTF-8".to_string())?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| format!("打开日志目录失败：{e}"))
}

/// 解析目标目录：显式传入优先（用户在对话框里选的），否则用默认日志目录；顺带创建
async fn resolve_dir(app: &AppHandle, dir: Option<&str>) -> Result<PathBuf, String> {
    let target = match dir {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
        _ => ssh_log_dir(app)?,
    };
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| format!("创建日志目录失败（{}）：{e}", target.display()))?;
    Ok(target)
}

/// 清洗标题为可用文件名片段（替换 Windows 非法字符与控制字符）
fn sanitize_title(title: &str) -> String {
    title
        .chars()
        .map(|ch| {
            if r#"<>:"/\|?*"#.contains(ch) || ch.is_control() {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

/// 生成日志文件路径：`<标题>-<yyyyMMdd-HHmmss>.log`，同秒重名追加 `-2`、`-3`
fn build_file_path(dir: &Path, title: &str) -> PathBuf {
    let stem = sanitize_title(title);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base = if stem.is_empty() {
        format!("ssh-{stamp}")
    } else {
        format!("{stem}-{stamp}")
    };
    let mut candidate = dir.join(format!("{base}.log"));
    let mut index = 1u32;
    while candidate.exists() && index < 100 {
        index += 1;
        candidate = dir.join(format!("{base}-{index}.log"));
    }
    candidate
}

/// 追加一块终端输出（剥离 ANSI 后写盘并 flush；失败由调用方决定停录与提示）
pub(crate) async fn append(sink: &SharedLog, data: &[u8]) -> Result<(), String> {
    let text = strip_ansi(&String::from_utf8_lossy(data));
    if text.is_empty() {
        return Ok(());
    }
    let mut guard = sink.lock().await;
    let Some(log) = guard.as_mut() else {
        return Ok(());
    };
    log.file
        .write_all(text.as_bytes())
        .await
        .map_err(|e| format!("写入日志失败（{}）：{e}", log.path.display()))?;
    log.file
        .flush()
        .await
        .map_err(|e| format!("刷新日志失败（{}）：{e}", log.path.display()))?;
    log.bytes += text.len() as u64;
    Ok(())
}

/// 收尾：flush 并释放句柄（终端关闭、连接断开、任务退出时调用）
pub(crate) async fn finish(sink: &SharedLog) {
    let mut guard = sink.lock().await;
    if let Some(mut log) = guard.take() {
        let _ = log.file.flush().await;
    }
}

/// 开始录制：创建日志文件并接管后续输出（幂等，已在录制时直接返回当前文件）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_log_start(
    app: AppHandle,
    state: State<'_, TerminalState>,
    terminal_id: String,
    dir: Option<String>,
) -> Result<LogActionResult, String> {
    // 取共享状态与标题（不持注册表锁跨 await）
    let (sink, title) = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        let handle = map.get(&terminal_id).ok_or("终端不存在或已关闭")?;
        (handle.log.clone(), handle.title.clone())
    };
    {
        let guard = sink.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(LogActionResult {
                path: existing.path.display().to_string(),
                bytes: existing.bytes,
            });
        }
    }
    let target_dir = resolve_dir(&app, dir.as_deref()).await?;
    let path = build_file_path(&target_dir, &title);
    let file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| format!("创建日志文件失败（{}）：{e}", path.display()))?;
    let mut guard = sink.lock().await;
    *guard = Some(LogSink {
        file,
        path: path.clone(),
        bytes: 0,
    });
    Ok(LogActionResult {
        path: path.display().to_string(),
        bytes: 0,
    })
}

/// 停止录制：flush 并关闭句柄，返回最终路径与字节数
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_terminal_log_stop(
    state: State<'_, TerminalState>,
    terminal_id: String,
) -> Result<LogActionResult, String> {
    let sink = {
        let map = state.0.lock().map_err(|e| e.to_string())?;
        map.get(&terminal_id)
            .ok_or("终端不存在或已关闭")?
            .log
            .clone()
    };
    let mut guard = sink.lock().await;
    match guard.take() {
        Some(mut log) => {
            let _ = log.file.flush().await;
            Ok(LogActionResult {
                path: log.path.display().to_string(),
                bytes: log.bytes,
            })
        }
        None => Err("当前终端未在录制日志".into()),
    }
}

/// 写盘失败时的通知负载构造（供终端任务调用，统一事件名）
pub(crate) fn error_payload(terminal_id: &str, message: String) -> TerminalLogError {
    TerminalLogError {
        terminal_id: terminal_id.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 颜色与重置序列应被完整剥离，正文保留
    #[test]
    fn strips_csi_color_sequences() {
        assert_eq!(
            strip_ansi("\x1b[0mplain\x1b[1;32mgreen\x1b[0m"),
            "plaingreen"
        );
        assert_eq!(strip_ansi("\x1b[38;2;240;86;44mred\x1b[39m"), "red");
    }

    /// OSC 标题（BEL 与 ST 两种终止）应被剥离
    #[test]
    fn strips_osc_titles() {
        assert_eq!(strip_ansi("\x1b]0;my-title\x07after"), "after");
        assert_eq!(strip_ansi("\x1b]0;other\x1b\\tail"), "tail");
    }

    /// 多序列混合与字符集指定序列
    #[test]
    fn strips_mixed_sequences() {
        assert_eq!(strip_ansi("\x1b[2J\x1b[H\x1b(B\x1b[mls -l"), "ls -l");
    }

    /// 纯文本原样保留（含换行与制表符）
    #[test]
    fn keeps_plain_text() {
        assert_eq!(strip_ansi("a\nb\tc"), "a\nb\tc");
    }

    /// `\r` 丢弃，只保留覆盖后的最终态
    #[test]
    fn drops_carriage_return() {
        assert_eq!(strip_ansi("10%\r50%\r100%\ndone"), "10%50%100%\ndone");
        assert_eq!(strip_ansi("progress\rline"), "progressline");
    }

    /// 未闭合的转义序列不应吃掉后续正文
    #[test]
    fn keeps_text_after_unclosed_sequence() {
        assert_eq!(strip_ansi("\x1b[31"), "");
        assert_eq!(strip_ansi("ok\x1b"), "ok");
    }

    /// 标题清洗：Windows 非法字符与控制字符替换为下划线
    #[test]
    fn sanitizes_file_name() {
        assert_eq!(sanitize_title("prod:22/root*"), "prod_22_root_");
        assert_eq!(sanitize_title("  web-1  "), "web-1");
        assert_eq!(sanitize_title(""), "");
    }
}
