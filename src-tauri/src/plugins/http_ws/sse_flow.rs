//! SSE 按前端消费确认推进发送窗口；容量只约束途中副本，不截断事件或历史。
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;

/// UTF-8 正文与元数据的在途预算，单个合法大事件可独占窗口。
const WINDOW_BYTES: usize = 1024 * 1024;

/// 会话持有确认入口，接收任务独占窗口计数。
pub(super) struct Acknowledgements {
    sender: watch::Sender<u64>,
    pub(super) sent: AtomicU64,
}

impl Acknowledgements {
    pub(super) fn new() -> (Self, Window) {
        let (sender, receiver) = watch::channel(0);
        (
            Self {
                sender,
                sent: AtomicU64::new(0),
            },
            Window {
                receiver,
                pending: VecDeque::new(),
                bytes: 0,
                sequence: 0,
            },
        )
    }

    /// 累积确认可合并、重发，不能提前确认尚未发送的批次。
    pub(super) fn acknowledge(&self, sequence: u64) -> Result<(), String> {
        if sequence > self.sent.load(Ordering::Acquire) {
            return Err("SSE 消费确认超出已发送范围".into());
        }
        self.sender.send_if_modified(|current| {
            if sequence > *current {
                *current = sequence;
                true
            } else {
                false
            }
        });
        Ok(())
    }
}

/// 只保存每批序号与大小，不再保存一份正文；关闭任务会释放挂起的等待。
pub(super) struct Window {
    receiver: watch::Receiver<u64>,
    pending: VecDeque<(u64, usize)>,
    bytes: usize,
    sequence: u64,
}

impl Window {
    fn reclaim(&mut self) {
        let acknowledged = *self.receiver.borrow_and_update();
        while self
            .pending
            .front()
            .is_some_and(|(seq, _)| *seq <= acknowledged)
        {
            if let Some((_, bytes)) = self.pending.pop_front() {
                self.bytes -= bytes;
            }
        }
    }

    /// 空窗口允许完整大事件通过；其余情况下等待消费，而不是丢弃或降频。
    pub(super) async fn reserve(&mut self, bytes: usize) -> Result<u64, String> {
        loop {
            self.reclaim();
            if self.pending.is_empty() || bytes <= WINDOW_BYTES.saturating_sub(self.bytes) {
                self.sequence = self.sequence.checked_add(1).ok_or("SSE 批次序号耗尽")?;
                self.pending.push_back((self.sequence, bytes));
                self.bytes += bytes;
                return Ok(self.sequence);
            }
            self.receiver
                .changed()
                .await
                .map_err(|_| "SSE 消费端已关闭")?;
        }
    }

    pub(super) async fn finish(&mut self) -> Result<(), String> {
        loop {
            self.reclaim();
            if self.pending.is_empty() {
                return Ok(());
            }
            self.receiver
                .changed()
                .await
                .map_err(|_| "SSE 消费端已关闭")?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Poll;

    #[tokio::test]
    async fn full_window_waits_for_actual_cumulative_consumption() {
        let (ack, mut window) = Acknowledgements::new();
        assert_eq!(window.reserve(WINDOW_BYTES).await.unwrap(), 1);
        ack.sent.store(1, Ordering::Release);
        assert!(ack.acknowledge(2).is_err());
        let next = window.reserve(1);
        tokio::pin!(next);
        assert!(matches!(futures_util::poll!(&mut next), Poll::Pending));
        ack.acknowledge(0).unwrap();
        assert!(matches!(futures_util::poll!(&mut next), Poll::Pending));
        ack.acknowledge(1).unwrap();
        assert_eq!(next.await.unwrap(), 2);
    }

    #[tokio::test]
    async fn oversized_event_is_preserved_and_closed_consumer_releases_waiter() {
        let (ack, mut window) = Acknowledgements::new();
        assert_eq!(window.reserve(WINDOW_BYTES * 3).await.unwrap(), 1);
        ack.sent.store(1, Ordering::Release);
        ack.acknowledge(1).unwrap();
        window.finish().await.unwrap();
        assert_eq!(window.bytes, 0);
        window.reserve(WINDOW_BYTES).await.unwrap();
        drop(ack);
        assert!(window.reserve(1).await.is_err());
    }
}
