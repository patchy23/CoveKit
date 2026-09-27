//! TTS 插件 · 数据模型（serde 契约结构，与前端 contracts.ts 逐字段对应）

/// 合成阶段；停滞提示不改变任务生命周期。
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsPhase {
    Connecting,
    Sending,
    Receiving,
    Writing,
    Publishing,
}

/// 请求内有序进展，前端按请求与序号隔离迟到事件。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsProgress {
    /// 后端登记的请求标识。
    pub(crate) job_id: String,
    /// 当前请求内严格递增的进展序号。
    pub(crate) sequence: u64,
    /// 当前操作阶段。
    pub(crate) phase: TtsPhase,
    /// 最近一次真实进展的 Unix 毫秒时间。
    pub(crate) last_progress_at: i64,
    /// 已交付写入器的累计音频字节数。
    pub(crate) bytes: u64,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_contract_uses_request_identity_and_camel_case() {
        let value = serde_json::to_value(TtsProgress {
            job_id: "job".into(), sequence: 2, phase: TtsPhase::Receiving,
            last_progress_at: 123, bytes: 456,
        }).unwrap();
        assert_eq!(value, serde_json::json!({
            "jobId":"job", "sequence":2, "phase":"receiving", "lastProgressAt":123, "bytes":456,
        }));
    }
}
