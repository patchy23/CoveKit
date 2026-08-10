//! TTS 插件 · 数据模型（serde 契约结构，与前端 contracts.ts 逐字段对应）

/// 语音项（对外契约）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsVoice {
    /// 语音标识（Edge TTS 名称，如 zh-CN-XiaoxiaoNeural）
    pub(crate) name: String,
    /// 展示名（中文描述）
    pub(crate) label: String,
    /// 语言代码
    pub(crate) lang: String,
}

/// 合成结果（对外契约）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsResult {
    /// 是否成功
    pub(crate) ok: bool,
    /// 生成的音频文件路径（前端 convertFileSrc 播放）
    pub(crate) file_path: Option<String>,
    /// 音频字节数
    pub(crate) bytes: u64,
    /// 错误信息（失败时）
    pub(crate) error: Option<String>,
}
