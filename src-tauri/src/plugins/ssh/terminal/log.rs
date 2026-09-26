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
    /// UTF-8 尾部与 ANSI 状态随录制文件持有，不随 SSH 数据块重置。
    decoder: super::Utf8ChunkDecoder,
    ansi: AnsiFilter,
}

/// 终端日志共享状态（TerminalHandle 持有，后台任务与命令层共享同一份）
pub(crate) type SharedLog = Arc<Mutex<Option<LogSink>>>;

/// 新建一个「未录制」的共享状态
pub(crate) fn new_shared() -> SharedLog {
    Arc::new(Mutex::new(None))
}

/// ANSI 只保留解析状态，不积攒未结束的控制序列正文。
#[derive(Default)]
enum AnsiFilter {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
    Charset,
}

impl AnsiFilter {
    /// 原地压缩 UTF-8 字符串，不为清洗再分配一份整块文本。
    fn retain_text(&mut self, text: &mut String) {
        text.retain(|ch| {
            *self = match *self {
                Self::Text => match ch {
                    '\x1b' => Self::Escape,
                    '\r' => Self::Text,
                    _ => return true,
                },
                Self::Escape => match ch {
                    '[' => Self::Csi,
                    ']' => Self::Osc,
                    '(' | ')' | '*' | '+' | '#' => Self::Charset,
                    _ => Self::Text,
                },
                Self::Csi if ('\u{40}'..='\u{7e}').contains(&ch) => Self::Text,
                Self::Csi => Self::Csi,
                Self::Osc | Self::OscEscape if ch == '\x07' => Self::Text,
                Self::OscEscape if ch == '\\' => Self::Text,
                Self::Osc | Self::OscEscape if ch == '\x1b' => Self::OscEscape,
                Self::Osc | Self::OscEscape => Self::Osc,
                Self::Charset => Self::Text,
            };
            false
        });
    }
}

#[cfg(test)]
fn strip_ansi(input: &str) -> String {
    let mut text = input.to_string();
    AnsiFilter::default().retain_text(&mut text);
    text
}

impl LogSink {
    async fn write_text(&mut self, mut text: String) -> Result<(), String> {
        self.ansi.retain_text(&mut text);
        if text.is_empty() {
            return Ok(());
        }
        self.file
            .write_all(text.as_bytes())
            .await
            .map_err(|e| format!("写入日志失败（{}）：{e}", self.path.display()))?;
        self.file
            .flush()
            .await
            .map_err(|e| format!("刷新日志失败（{}）：{e}", self.path.display()))?;
        self.bytes += text.len() as u64;
        Ok(())
    }

    async fn finish(&mut self) -> Result<(), String> {
        let tail = self.decoder.finish();
        self.write_text(tail).await?;
        self.file
            .flush()
            .await
            .map_err(|e| format!("刷新日志失败（{}）：{e}", self.path.display()))
    }
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
    let mut guard = sink.lock().await;
    let Some(log) = guard.as_mut() else {
        return Ok(());
    };
    // 未录制时不做 UTF-8 解码或 ANSI 清洗；同一把锁保证检查到写入之间不会换录制文件。
    let text = log.decoder.feed(data);
    log.write_text(text).await
}

/// 收尾：flush 并释放句柄（终端关闭、连接断开、任务退出时调用）
pub(crate) async fn finish(sink: &SharedLog) -> Result<(), String> {
    let mut guard = sink.lock().await;
    if let Some(mut log) = guard.take() {
        log.finish().await?;
    }
    Ok(())
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
        decoder: super::Utf8ChunkDecoder::default(),
        ansi: AnsiFilter::default(),
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
            log.finish().await?;
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

    /// 丢弃 `\r`，仍保留按到达顺序写入的全部正文
    #[test]
    fn drops_carriage_return() {
        assert_eq!(strip_ansi("10%\r50%\r100%\ndone"), "10%50%100%\ndone");
        assert_eq!(strip_ansi("progress\rline"), "progressline");
    }

    /// 未闭合序列沿用丢弃至块末的语义
    #[test]
    fn keeps_text_after_unclosed_sequence() {
        assert_eq!(strip_ansi("\x1b[31"), "");
        assert_eq!(strip_ansi("ok\x1b"), "ok");
    }

    /// 字符迭代不应截断多字节正文或改变 OSC 内嵌 ESC 的边界。
    #[test]
    fn keeps_unicode_around_escape_sequences() {
        assert_eq!(strip_ansi("中文\x1b[31m🙂\x1b[0m\n"), "中文🙂\n");
        assert_eq!(strip_ansi("前\x1b]标题\x1bX内容\x1b\\后"), "前后");
        assert_eq!(strip_ansi("前\x1b(中后"), "前后");
        assert_eq!(strip_ansi("\x1b("), "");
    }

    /// 任意 SSH 字节边界都不拆坏 Unicode，也不会将转义序列尾部写入正文。
    #[test]
    fn stream_filter_preserves_text_at_every_byte_boundary() {
        let input = "中文\x1b[31m🙂\x1b[0m\r\n前\x1b]标题\x1bX内容\x1b\\后\x1b(中末";
        let expected = "中文🙂\n前后末";
        for split in 0..=input.len() {
            let mut decoder = super::super::Utf8ChunkDecoder::default();
            let mut ansi = AnsiFilter::default();
            let mut output = String::new();
            for bytes in [&input.as_bytes()[..split], &input.as_bytes()[split..]] {
                let mut text = decoder.feed(bytes);
                ansi.retain_text(&mut text);
                output.push_str(&text);
            }
            let mut tail = decoder.finish();
            ansi.retain_text(&mut tail);
            output.push_str(&tail);
            assert_eq!(output, expected, "split={split}");
        }
        let mut decoder = super::super::Utf8ChunkDecoder::default();
        let mut ansi = AnsiFilter::default();
        let mut output = String::new();
        for bytes in input.as_bytes().chunks(1) {
            let mut text = decoder.feed(bytes);
            ansi.retain_text(&mut text);
            output.push_str(&text);
        }
        assert_eq!(output, expected);
    }

    /// 结束录制才将未完成的 UTF-8 尾部转为替换符，文件内容和字节统计保持一致。
    #[tokio::test]
    async fn recording_flushes_incomplete_utf8_on_finish() {
        let path = std::env::temp_dir().join(format!("covekit-log-{}.txt", uuid::Uuid::new_v4()));
        let file = tokio::fs::File::create(&path).await.unwrap();
        let sink = new_shared();
        *sink.lock().await = Some(LogSink {
            file,
            path: path.clone(),
            bytes: 0,
            decoder: super::super::Utf8ChunkDecoder::default(),
            ansi: AnsiFilter::default(),
        });
        append(&sink, &[b'a', 0xe4, 0xb8]).await.unwrap();
        assert_eq!(sink.lock().await.as_ref().unwrap().bytes, 1);
        let mut log = sink.lock().await.take().unwrap();
        log.finish().await.unwrap();
        assert_eq!(log.bytes, 4);
        drop(log);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "a\u{fffd}");
        tokio::fs::remove_file(path).await.unwrap();
    }

    /// 开关录制不改变正文与字节统计，停止后不再追加。
    #[tokio::test]
    async fn appends_only_while_recording() {
        let sink = new_shared();
        append(&sink, b"before recording").await.unwrap();
        assert!(sink.lock().await.is_none());
        let path = std::env::temp_dir().join(format!("covekit-log-{}.txt", uuid::Uuid::new_v4()));
        let file = tokio::fs::File::create(&path).await.unwrap();
        *sink.lock().await = Some(LogSink {
            file,
            path: path.clone(),
            bytes: 0,
            decoder: super::super::Utf8ChunkDecoder::default(),
            ansi: AnsiFilter::default(),
        });
        append(&sink, "\x1b[31m中文🙂\x1b[0m\r\n".as_bytes())
            .await
            .unwrap();
        append(&sink, b"\x1b[0m").await.unwrap();
        assert_eq!(sink.lock().await.as_ref().unwrap().bytes, 11);
        finish(&sink).await.unwrap();
        append(&sink, b"after recording").await.unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "中文🙂\n");
        tokio::fs::remove_file(path).await.unwrap();
    }

    /// 标题清洗：Windows 非法字符与控制字符替换为下划线
    #[test]
    fn sanitizes_file_name() {
        assert_eq!(sanitize_title("prod:22/root*"), "prod_22_root_");
        assert_eq!(sanitize_title("  web-1  "), "web-1");
        assert_eq!(sanitize_title(""), "");
    }
}
