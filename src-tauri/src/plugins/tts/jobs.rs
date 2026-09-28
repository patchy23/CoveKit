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
    tokio::sync::Mutex<Option<crate::framework::temp_instance::TempInstance>>,
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
    pub(super) async fn temporary_directory(
        &self,
        root: std::path::PathBuf,
    ) -> Result<std::path::PathBuf, String> {
        let mut instance = self.1.lock().await;
        if instance.is_none() {
            *instance = Some(
                crate::framework::storage::access::spawn_blocking(move || {
                    crate::framework::temp_instance::TempInstance::open(&root)
                })
                .await
                .map_err(|e| format!("初始化音频临时实例失败: {e}"))??,
            );
        }
        instance
            .as_ref()
            .map(|instance| instance.directory().to_path_buf())
            .ok_or_else(|| "音频临时实例尚未初始化".to_string())
    }

    /// 维护入口已冻结并排空操作后释放存活文件锁；后续请求在新根重新创建实例。
    pub(super) fn release_storage(&self) -> Result<(), String> {
        let jobs = self
            .0
            .try_lock()
            .map_err(|e| format!("合成任务仍在使用: {e}"))?;
        if !jobs.is_empty() {
            return Err("仍有语音合成任务，请等待取消完成后重试".into());
        }
        let mut instance = self
            .1
            .try_lock()
            .map_err(|e| format!("音频临时实例仍在使用: {e}"))?;
        // TempInstance 持有的 File 随 drop 关闭，解除旧根 alive.lock 的独占锁。
        *instance = None;
        Ok(())
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
    async fn maintenance_releases_old_instance_and_reopens_in_new_root() {
        let root = std::env::temp_dir().join(format!("covekit-tts-reset-{}", uuid::Uuid::new_v4()));
        let old_root = root.join("old");
        let new_root = root.join("new");
        let jobs = TtsJobs::default();
        let old_path = jobs.temporary_directory(old_root.clone()).await.unwrap();
        assert_eq!(
            jobs.temporary_directory(old_root.clone()).await.unwrap(),
            old_path
        );
        let alive = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(old_path.join("alive.lock"))
            .unwrap();
        assert!(matches!(
            alive.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        jobs.release_storage().unwrap();
        alive.try_lock().unwrap();
        drop(alive);
        let new_path = jobs.temporary_directory(new_root.clone()).await.unwrap();
        assert!(new_path.starts_with(&new_root));
        assert_ne!(new_path, old_path);
        jobs.release_storage().unwrap();
        // 旧实例释放后，原根再次打开能回收旧目录，不遗留存活锁。
        let reopened = crate::framework::temp_instance::TempInstance::open(&old_root).unwrap();
        assert!(!old_path.exists());
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn maintenance_rejects_pending_jobs_and_busy_instance() {
        let jobs = TtsJobs::default();
        let id = jobs.prepare().unwrap();
        assert!(jobs.release_storage().is_err());
        jobs.cancel(&id).unwrap();
        let instance = jobs.1.lock().await;
        assert!(jobs.release_storage().is_err());
        drop(instance);
        jobs.release_storage().unwrap();
    }

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
