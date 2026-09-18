//! TTS 插件 · 文字转语音（微软 Edge TTS 免费服务）命令门面
//! 模块结构：mod.rs（命令薄层 + 注册）+ models.rs（serde 契约）+ synth.rs（合成能力）

mod models;
mod synth;

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
    let audio = synth::synth_bytes(&text, &voice, rate, pitch).await?;

    // 落盘缓存分区 <root>/cache/tts/<ts>.mp3
    let dir = crate::framework::paths::cache_dir(&app, "tts")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 tts 目录失败: {e}"))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("{ts}.mp3"));
    std::fs::write(&path, &audio).map_err(|e| format!("写入音频失败: {e}"))?;

    Ok(TtsResult {
        ok: true,
        file_path: Some(path.to_string_lossy().to_string()),
        bytes: audio.len() as u64,
        error: None,
    })
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
