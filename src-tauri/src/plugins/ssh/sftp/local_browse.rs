//! 本地目录读取的请求归属；取消只能命中自己的任务，阻塞系统调用返回后协作退出。
use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use tauri::State;

const CANCELLED: &str = "SSH_LOCAL_LIST_CANCELLED";

struct Request {
    cancel: Arc<AtomicBool>,
    claimed: bool,
}

/// 登记期间不持有目录内容；后台扫描与页面共享取消标志。
#[derive(Clone, Default)]
pub struct LocalBrowseState(Arc<Mutex<HashMap<String, Request>>>);

pub(super) struct ReadGuard {
    state: LocalBrowseState,
    id: String,
    cancel: Arc<AtomicBool>,
}

impl ReadGuard {
    pub(super) fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) { Err(CANCELLED.into()) } else { Ok(()) }
    }
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        if let Err(error) = self.state.cancel(&self.id) {
            log::error!("本地目录读取登记释放失败: {error}");
        }
    }
}

impl LocalBrowseState {
    fn prepare(&self) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.0.lock().map_err(|e| e.to_string())?.insert(id.clone(), Request {
            cancel: Arc::new(AtomicBool::new(false)), claimed: false,
        });
        Ok(id)
    }

    pub(super) fn claim(&self, id: Option<String>) -> Result<ReadGuard, String> {
        let id = match id { Some(id) => id, None => self.prepare()? };
        let cancel = {
            let mut requests = self.0.lock().map_err(|e| e.to_string())?;
            let request = requests.get_mut(&id).ok_or(CANCELLED)?;
            if request.claimed { return Err("本地目录读取请求已使用".into()); }
            request.claimed = true;
            Arc::clone(&request.cancel)
        };
        Ok(ReadGuard { state: self.clone(), id, cancel })
    }

    fn cancel(&self, id: &str) -> Result<(), String> {
        if let Some(request) = self.0.lock().map_err(|e| e.to_string())?.remove(id) {
            request.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    pub(crate) fn cancel_all(&self) -> Result<(), String> {
        for (_, request) in self.0.lock().map_err(|e| e.to_string())?.drain() {
            request.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// 先登记再读取，以覆盖页面关闭早于目录命令发起的情况。
#[tauri::command]
pub fn ssh_local_list_prepare(state: State<'_, LocalBrowseState>) -> Result<String, String> {
    state.prepare()
}

/// 幂等取消单次本地目录读取，不影响其它 SSH 页签。
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_local_list_cancel(state: State<'_, LocalBrowseState>, request_id: String) -> Result<(), String> {
    state.cancel(&request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_before_claim_and_between_entries_is_scoped() {
        let state = LocalBrowseState::default();
        let early = state.prepare().unwrap();
        state.cancel(&early).unwrap();
        assert!(state.claim(Some(early)).is_err());
        let id = state.prepare().unwrap();
        let first = state.claim(Some(id.clone())).unwrap();
        assert!(state.claim(Some(id.clone())).is_err());
        let second = state.claim(None).unwrap();
        state.cancel(&id).unwrap();
        assert_eq!(first.check().unwrap_err(), CANCELLED);
        assert!(second.check().is_ok());
        drop(first);
        assert_eq!(state.0.lock().unwrap().len(), 1);
        state.cancel_all().unwrap();
        assert!(second.check().is_err());
        drop(second);
        assert!(state.0.lock().unwrap().is_empty());
        drop(state.claim(None).unwrap());
        assert!(state.0.lock().unwrap().is_empty());
    }
}
