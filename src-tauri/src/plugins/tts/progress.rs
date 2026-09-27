//! 请求私有的进展通道；只发送阶段和计数，不发送正文或音频。

use tauri::ipc::Channel;

use super::models::{TtsPhase, TtsProgress};

pub(super) struct Progress {
    job_id: String,
    sequence: u64,
    channel: Option<Channel<TtsProgress>>,
}

impl Progress {
    pub(super) fn new(job_id: &str, channel: Option<Channel<TtsProgress>>) -> Self {
        Self { job_id: job_id.to_string(), sequence: 0, channel }
    }

    /// 仅在真实阶段推进或音频读写完成时报告；连接存活消息不重置接收停滞时间。
    pub(super) fn report(&mut self, phase: TtsPhase, bytes: u64) -> Result<(), String> {
        self.sequence = self.sequence.checked_add(1).ok_or("语音进展序号溢出")?;
        if let Some(channel) = &self.channel {
            channel.send(TtsProgress {
                job_id: self.job_id.clone(),
                sequence: self.sequence,
                phase,
                last_progress_at: chrono::Utc::now().timestamp_millis(),
                bytes,
            }).map_err(|_| "语音合成进展通道已关闭".to_string())?;
        }
        Ok(())
    }
}
