//! 合成请求的取消归属：先登记再提交正文，清空早于合成启动时也不会漏取消。

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::watch;

pub(super) const CANCELLED: &str = "语音合成已取消";

struct Job {
    cancel: watch::Sender<bool>,
    receiver: Option<watch::Receiver<bool>>,
}

/// 只保存活跃请求的取消信号，不缓存文本或音频。
#[derive(Default)]
pub struct TtsJobs(
    Mutex<HashMap<String, Job>>,
    tokio::sync::OnceCell<crate::framework::temp_instance::TempInstance>,
);

pub(super) struct JobGuard<'a> {
    jobs: &'a TtsJobs,
    id: String,
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        match self.jobs.0.lock() {
            Ok(mut jobs) => {
                jobs.remove(&self.id);
            }
            Err(_) => log::error!("释放语音合成请求登记失败"),
        }
    }
}

impl TtsJobs {
    /// 实例随应用状态存活；清理旧实例和创建锁在阻塞池执行，失败可重试。
    pub(super) async fn temporary_directory(&self, root: std::path::PathBuf) -> Result<&std::path::Path, String> {
        let instance = self.1.get_or_try_init(|| async {
            tokio::task::spawn_blocking(move || crate::framework::temp_instance::TempInstance::open(&root))
                .await.map_err(|e| format!("初始化音频临时实例失败: {e}"))?
        }).await?;
        Ok(instance.directory())
    }

    pub(super) fn prepare(&self) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (cancel, receiver) = watch::channel(false);
        self.0.lock().map_err(|e| e.to_string())?.insert(
            id.clone(),
            Job {
                cancel,
                receiver: Some(receiver),
            },
        );
        Ok(id)
    }

    pub(super) fn claim(&self, id: &str) -> Result<(JobGuard<'_>, watch::Receiver<bool>), String> {
        let receiver = self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get_mut(id)
            .and_then(|job| job.receiver.take())
            .ok_or_else(|| "合成任务不存在、已取消或已经开始".to_string())?;
        Ok((
            JobGuard {
                jobs: self,
                id: id.to_string(),
            },
            receiver,
        ))
    }

    pub(super) fn cancel(&self, id: &str) -> Result<(), String> {
        if let Some(job) = self.0.lock().map_err(|e| e.to_string())?.remove(id) {
            job.cancel.send_replace(true);
        }
        Ok(())
    }

    pub(super) fn cancel_all(&self) -> Result<(), String> {
        for (_, job) in self.0.lock().map_err(|e| e.to_string())?.drain() {
            job.cancel.send_replace(true);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_before_and_after_claim_is_not_lost() {
        let jobs = TtsJobs::default();
        let before = jobs.prepare().unwrap();
        jobs.cancel(&before).unwrap();
        assert!(jobs.claim(&before).is_err());
        let active = jobs.prepare().unwrap();
        let (guard, mut receiver) = jobs.claim(&active).unwrap();
        assert!(jobs.claim(&active).is_err());
        jobs.cancel(&active).unwrap();
        assert!(*receiver.wait_for(|value| *value).await.unwrap());
        let next = jobs.prepare().unwrap();
        drop(guard);
        assert!(jobs.claim(&next).is_ok());
        assert!(jobs.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn tool_cleanup_cancels_active_and_unclaimed_jobs() {
        let jobs = TtsJobs::default();
        let active = jobs.prepare().unwrap();
        let pending = jobs.prepare().unwrap();
        let (_guard, mut receiver) = jobs.claim(&active).unwrap();
        jobs.cancel_all().unwrap();
        assert!(*receiver.wait_for(|value| *value).await.unwrap());
        assert!(jobs.claim(&pending).is_err());
        assert!(jobs.0.lock().unwrap().is_empty());
    }
}
