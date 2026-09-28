//! 日志快照响应：不留后端正文缓存，不改变远端轮询和内容保留能力。
use sha2::{Digest, Sha256};

use crate::plugins::ssh::models::LogSnapshot;

fn build(logs: String, previous: Option<&str>) -> LogSnapshot {
    let fingerprint = hex::encode(Sha256::digest(logs.as_bytes()));
    let unchanged = previous == Some(fingerprint.as_str());
    LogSnapshot {
        ok: true,
        fingerprint,
        unchanged,
        logs: if unchanged { None } else { Some(logs) },
    }
}

/// 摘要计算移出异步执行线程；变化正文直接移动进 DTO，避免 json! 再复制字符串。
pub(crate) async fn snapshot(
    logs: String,
    previous: Option<String>,
) -> Result<LogSnapshot, String> {
    crate::framework::storage::access::spawn_blocking(move || build(logs, previous.as_deref()))
        .await
        .map_err(|e| format!("构建日志响应失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_logs_omit_body_but_changes_and_empty_logs_are_complete() {
        let text = "中文🙂\nline\n".to_string();
        let pointer = text.as_ptr();
        let first = build(text, None);
        assert_eq!(first.logs.as_ref().unwrap().as_ptr(), pointer);
        assert!(!first.unchanged);
        let same = build("中文🙂\nline\n".into(), Some(&first.fingerprint));
        assert!(same.unchanged);
        assert!(same.logs.is_none());
        let changed = build("中文🙂\nnew line\n".into(), Some(&first.fingerprint));
        assert!(!changed.unchanged);
        assert_eq!(changed.logs.as_deref(), Some("中文🙂\nnew line\n"));
        let empty = build(String::new(), Some(&first.fingerprint));
        assert!(!empty.unchanged);
        assert_eq!(empty.logs.as_deref(), Some(""));
    }
}
