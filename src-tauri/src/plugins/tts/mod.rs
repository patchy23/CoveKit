//! TTS 插件 · 文字转语音（微软 Edge TTS 免费服务）命令门面
//! 模块结构：mod.rs（命令薄层 + 注册）+ models.rs（serde 契约）+ synth.rs（合成能力）

mod jobs;
mod models;
mod output;
mod progress;
mod synth;

use jobs::{TtsJobs, CANCELLED};
use tauri::Manager;
use tokio::io::AsyncWriteExt;

pub(crate) use models::{TtsResult, TtsVoice};

/// 语音列表
#[tauri::command(rename_all = "camelCase")]
pub fn tts_voices() -> Vec<TtsVoice> {
    synth::builtin_voices()
}

/// 先取得请求归属，避免清空先于合成命令登记而丢失取消信号。
#[tauri::command]
pub fn tts_prepare(state: tauri::State<'_, TtsJobs>) -> Result<String, String> {
    state.prepare()
}

/// 幂等取消；已经完成或取消的请求无需再次处理。
#[tauri::command(rename_all = "camelCase")]
pub fn tts_cancel(state: tauri::State<'_, TtsJobs>, job_id: String) -> Result<(), String> {
    state.cancel(&job_id)
}

/// 前端已收到但不再使用的迟到结果；不释放已经呈现给用户的播放或下载源。
#[tauri::command(rename_all = "camelCase")]
pub async fn tts_discard(app: tauri::AppHandle, job_id: String) -> Result<(), String> {
    let directory = crate::framework::paths::cache_dir(&app, "tts")?;
    output::discard(&directory, &job_id).await
}

/// 合成语音：文本 + 语音 + 语速/音调 → mp3 文件
#[tauri::command(rename_all = "camelCase")]
pub async fn tts_synthesize(
    app: tauri::AppHandle,
    state: tauri::State<'_, TtsJobs>,
    job_id: String,
    text: String,
    voice: String,
    rate: Option<i32>,
    pitch: Option<i32>,
    on_progress: Option<tauri::ipc::Channel<models::TtsProgress>>,
) -> Result<TtsResult, String> {
    let (_job, mut cancelled) = state.claim(&job_id)?;
    let mut progress = progress::Progress::new(&job_id, on_progress);
    log::info!("语音合成开始");
    let log_started = std::time::Instant::now();
    let mut log_stage = "synthesize";
    let result: Result<TtsResult, String> = async {
        // 分片写入私有临时文件；完成并 flush 后才发布可播放的路径。
        log_stage = "cache_directory";
        let dir = crate::framework::paths::cache_dir(&app, "tts")?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("创建 tts 目录失败: {e}"))?;
        let path = output::audio_path(&dir, &job_id)?;
        let temporary_dir = state.temporary_directory(dir.join("partial-instances")).await?;
        if *cancelled.borrow() { return Err(CANCELLED.to_string()); }
        let temporary = output::audio_path(temporary_dir, &job_id)?.with_extension("part");
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|e| format!("创建音频文件失败: {e}"))?;
        log_stage = "synthesize";
        // 取消只中断网络合成；文件已经提交的异步写入先收尾，再删除私有文件。
        let mut writer = tokio::io::BufWriter::with_capacity(64 * 1024, file);
        let mut report = |phase, bytes| progress.report(phase, bytes);
        let synthesis = tokio::select! {
            biased;
            _ = cancelled.wait_for(|value| *value) => Err(CANCELLED.to_string()),
            result = synth::synth_to_writer(&text, &voice, rate, pitch, &mut writer, &mut report) => result,
        };
        let written: Result<u64, String> = async {
            let bytes = synthesis?;
            progress.report(models::TtsPhase::Writing, bytes)?;
            writer
                .flush()
                .await
                .map_err(|e| format!("写入音频失败: {e}"))?;
            if *cancelled.borrow() {
                return Err(CANCELLED.to_string());
            }
            Ok(bytes)
        }
        .await;
        // 错误/取消时不把 BufWriter 的剩余缓冲写盘，只等待已开始的文件操作完成。
        let settled = writer.get_mut().flush().await;
        drop(writer);
        let written = match (written, settled) {
            (Ok(bytes), Ok(())) => Ok(bytes),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(format!("音频文件收尾失败: {error}")),
            (Err(error), Err(cleanup)) => Err(format!("{error}；音频文件收尾失败: {cleanup}")),
        };
        // 进展交付失败也走文件收尾，不能在 publish 前直接返回遗留临时文件。
        let written = written.and_then(|bytes| {
            progress.report(models::TtsPhase::Publishing, bytes)?;
            Ok(bytes)
        });
        let bytes = output::publish(&temporary, &path, written, &cancelled).await?;

        Ok(TtsResult {
            ok: true,
            file_path: Some(path.to_string_lossy().to_string()),
            bytes,
            error: None,
        })
    }
    .await;
    match &result {
        Ok(value) if value.ok => log::info!(
            "操作完成 operation=tts_synthesize bytes={} elapsed_ms={}",
            value.bytes,
            log_started.elapsed().as_millis()
        ),
        Ok(_) => log::warn!(
            "操作未完成 operation=tts_synthesize elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(error) if error == CANCELLED => log::debug!("语音合成已取消"),
        Err(error) if error == "请输入要合成的文字" || error == "文本过长（最多 2000 字）" =>
        {
            log::debug!("语音合成输入校验未通过");
        }
        Err(_) => log::error!(
            "语音合成失败 stage={log_stage} elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

// 模块静态清单：命令名、入库元数据与分派 handler 同源生成（AR07 §10.2）
crate::covekit_module! {
    owner: "tts",
    feature: "tts",
    commands: {
        tts_voices => "获取文字转语音可选语音列表",
        tts_prepare => "登记可取消的语音合成请求",
        tts_cancel => "取消语音合成请求",
        tts_discard => "清理未使用的过期语音合成结果",
        tts_synthesize => "合成语音（文本 + 语音 + 语速/音调 → mp3 文件）",
    },
}

/// 工具关闭、退出与空间切换统一取消在途合成。
fn on_dispose(
    app: Option<&tauri::AppHandle>,
    _reason: crate::framework::lifecycle::CloseReason,
) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    app.state::<TtsJobs>().cancel_all().err().into_iter().collect()
}

/// 插件注册：命令、取消状态与关闭责任同时装配。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::for_tool(IPC_OWNER, "tts")
            .with_tab_scope()
            .with_dispose(on_dispose),
    );
    builder.manage(TtsJobs::default())
}
