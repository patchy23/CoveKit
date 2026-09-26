//! DNS 只读请求的取消归属；先登记再发起，关闭早于请求启动时也不会漏取消。
use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use tokio::sync::watch;

pub(super) const CANCELLED: &str = "DNS_READ_CANCELLED";

struct Request {
    cancel: watch::Sender<bool>,
    receiver: Option<watch::Receiver<bool>>,
}

/// 只保存活跃请求的取消信号，不保存查询正文或凭证。
#[derive(Default)]
pub struct DnsRequests(Mutex<HashMap<String, Request>>);

struct RequestGuard<'a> {
    requests: &'a DnsRequests,
    id: &'a str,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.requests.cancel(self.id) {
            log::error!("DNS 请求登记释放失败: {error}");
        }
    }
}

impl DnsRequests {
    pub(super) fn prepare(&self) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (cancel, receiver) = watch::channel(false);
        self.0.lock().map_err(|e| e.to_string())?.insert(
            id.clone(),
            Request {
                cancel,
                receiver: Some(receiver),
            },
        );
        Ok(id)
    }

    /// Future 归命令所有，取消即丢弃读请求；不对云端写操作应用这种取消方式。
    pub(super) async fn run<T>(
        &self,
        id: Option<&str>,
        operation: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let Some(id) = id else {
            return operation.await;
        };
        let mut cancelled = self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get_mut(id)
            .and_then(|request| request.receiver.take())
            .ok_or(CANCELLED)?;
        let _guard = RequestGuard { requests: self, id };
        tokio::select! {
            biased;
            _ = cancelled.wait_for(|value| *value) => Err(CANCELLED.into()),
            result = operation => result,
        }
    }

    pub(super) fn cancel(&self, id: &str) -> Result<(), String> {
        if let Some(request) = self.0.lock().map_err(|e| e.to_string())?.remove(id) {
            request.cancel.send_replace(true);
        }
        Ok(())
    }

    pub(super) fn cancel_all(&self) -> Result<(), String> {
        for (_, request) in self.0.lock().map_err(|e| e.to_string())?.drain() {
            request.cancel.send_replace(true);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_before_start_never_polls_operation() {
        let requests = DnsRequests::default();
        let id = requests.prepare().unwrap();
        requests.cancel(&id).unwrap();
        let result: Result<(), _> = requests.run(Some(&id), async {
            panic!("已取消请求不应启动网络操作")
        }).await;
        assert_eq!(result.unwrap_err(), CANCELLED);
    }

    #[tokio::test]
    async fn cancelling_one_request_drops_its_work_without_affecting_another() {
        let requests = DnsRequests::default();
        let id = requests.prepare().unwrap();
        let other = requests.prepare().unwrap();
        let result: Result<(), _> = requests.run(Some(&id), async {
            requests.cancel(&id).unwrap();
            std::future::pending().await
        }).await;
        assert_eq!(result.unwrap_err(), CANCELLED);
        assert_eq!(requests.run(Some(&other), async { Ok(7) }).await.unwrap(), 7);
        assert!(requests.0.lock().unwrap().is_empty());
        let unused = requests.prepare().unwrap();
        requests.cancel_all().unwrap();
        assert!(requests.run(Some(&unused), async { Ok(()) }).await.is_err());
    }
}
