//! 合成文件发布与取消收尾：只操作调用方已创建的本次私有文件，不扫描其他任务。

use std::path::Path;
use tokio::sync::watch;

use super::jobs::CANCELLED;

/// 调用前必须完成已开始的写入并关闭句柄；发布前后都核对取消，避免收尾等待漏掉取消。
pub(super) async fn publish(
    temporary: &Path,
    path: &Path,
    written: Result<u64, String>,
    cancelled: &watch::Receiver<bool>,
) -> Result<u64, String> {
    let mut cleanup_path = temporary;
    let is_cancelled = *cancelled.borrow();
    let result = match written {
        Err(error) => Err(error),
        Ok(_) if is_cancelled => Err(CANCELLED.to_string()),
        Ok(bytes) => match tokio::fs::rename(temporary, path).await {
            Err(error) => Err(format!("保存音频失败: {error}")),
            Ok(()) => {
                // rename 已完成后只清理新路径；取消不能留下已发布但无人接收的本次文件。
                cleanup_path = path;
                if *cancelled.borrow() {
                    Err(CANCELLED.to_string())
                } else {
                    Ok(bytes)
                }
            }
        },
    };
    match result {
        Ok(bytes) => Ok(bytes),
        Err(error) => {
            tokio::fs::remove_file(cleanup_path)
                .await
                .map_err(|cleanup| format!("{error}；音频文件清理失败: {cleanup}"))?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_after_writer_settlement_removes_only_its_private_file() {
        let dir = std::env::temp_dir().join(format!("covekit-tts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let temporary = dir.join("own.part");
        let path = dir.join("own.mp3");
        let other = dir.join("other.part");
        tokio::fs::write(&temporary, b"ID3own").await.unwrap();
        tokio::fs::write(&other, b"other task").await.unwrap();
        let (cancel, receiver) = watch::channel(false);
        // 对应网络结束后、写盘收尾等待期间收到的取消。
        cancel.send_replace(true);
        assert_eq!(
            publish(&temporary, &path, Ok(6), &receiver).await.unwrap_err(),
            CANCELLED
        );
        assert!(!temporary.exists());
        assert!(!path.exists());
        assert_eq!(tokio::fs::read(&other).await.unwrap(), b"other task");
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn successful_publication_preserves_bytes_and_rename_failure_cleans_partial_file() {
        let dir = std::env::temp_dir().join(format!("covekit-tts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let temporary = dir.join("own.part");
        let path = dir.join("own.mp3");
        let (_cancel, receiver) = watch::channel(false);
        tokio::fs::write(&temporary, b"ID3data").await.unwrap();
        assert_eq!(publish(&temporary, &path, Ok(7), &receiver).await.unwrap(), 7);
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"ID3data");
        assert!(!temporary.exists());
        tokio::fs::write(&temporary, b"partial").await.unwrap();
        let missing = dir.join("missing").join("output.mp3");
        assert!(publish(&temporary, &missing, Ok(7), &receiver)
            .await
            .unwrap_err()
            .contains("保存音频失败"));
        assert!(!temporary.exists());
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"ID3data");
        tokio::fs::write(&temporary, b"partial").await.unwrap();
        assert_eq!(
            publish(&temporary, &path, Err("写盘失败".into()), &receiver)
                .await
                .unwrap_err(),
            "写盘失败"
        );
        assert!(!temporary.exists());
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"ID3data");
        let error = publish(&temporary, &path, Err("原始失败".into()), &receiver)
            .await
            .unwrap_err();
        assert!(error.contains("原始失败"));
        assert!(error.contains("音频文件清理失败"));
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
