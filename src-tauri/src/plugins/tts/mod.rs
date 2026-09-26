//! TTS 插件 · 文字转语音（微软 Edge TTS 免费服务）命令门面
//! 模块结构：mod.rs（命令薄层 + 注册）+ models.rs（serde 契约）+ synth.rs（合成能力）

mod models;
mod synth;

use tokio::io::AsyncWriteExt;

pub(crate) use models::{TtsResult, TtsVoice};

/// 语音列表
#[tauri::command(rename_all = "camelCase")]
pub fn tts_voices() -> Vec<TtsVoice> {
    synth::builtin_voices()
}

/// 合成语音：文本 + 语音 + 语速/音调 → mp3 文件
#[tauri::command(rename_all = "camelCase")]
pub async fn tts_synthesize(
    app: tauri::AppHandle,
    text: String,
    voice: String,
    rate: Option<i32>,
    pitch: Option<i32>,
) -> Result<TtsResult, String> {
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
        let id = uuid::Uuid::new_v4();
        let path = dir.join(format!("{id}.mp3"));
        let temporary = dir.join(format!("{id}.part"));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|e| format!("创建音频文件失败: {e}"))?;
        log_stage = "synthesize";
        let written: Result<u64, String> = async {
            // 缓冲合并小音频帧的磁盘写入，不限制音频长度。
            let mut writer = tokio::io::BufWriter::with_capacity(64 * 1024, file);
            let bytes = synth::synth_to_writer(&text, &voice, rate, pitch, &mut writer).await?;
            writer
                .flush()
                .await
                .map_err(|e| format!("写入音频失败: {e}"))?;
            drop(writer);
            tokio::fs::rename(&temporary, &path)
                .await
                .map_err(|e| format!("保存音频失败: {e}"))?;
            Ok(bytes)
        }
        .await;
        let bytes = match written {
            Ok(bytes) => bytes,
            Err(error) => {
                if let Err(cleanup) = tokio::fs::remove_file(&temporary).await {
                    return Err(format!("{error}；临时音频清理失败: {cleanup}"));
                }
                return Err(error);
            }
        };

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
        tts_synthesize => "合成语音（文本 + 语音 + 语速/音调 → mp3 文件）",
    },
}

/// 插件注册：命令入库（无 State，纯函数式）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    // 关闭清理：本插件无状态表、无子进程（请求在后端跑完即结束），故不登记关闭钩子
    // （AR06 方案 §5；新增常驻资源时必须回来补登记）
    builder
}
