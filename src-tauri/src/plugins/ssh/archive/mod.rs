//! 可取消的远端归档任务；每连接串行，命令 Channel 与视图开关无关。
mod stream;
use crate::plugins::ssh::{
    conn::{get_session, SshState},
    models::{ArchiveEvent, ArchiveRequest},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{ipc::Channel, State};
use tokio::sync::{watch, Mutex as AsyncMutex};

/// 活动任务取消句柄。
pub(crate) struct ArchiveJob {
    /// 归属连接
    pub connection_id: String,
    /// 独立取消信号
    pub cancel: watch::Sender<bool>,
}
/// 模块持有排队与执行中任务；已结束记录由连接 UI 有界保存。
#[derive(Default)]
pub struct ArchiveState {
    /// 活动任务
    pub(crate) jobs: Mutex<HashMap<String, ArchiveJob>>,
    /// 同一连接共用执行锁
    queues: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

/// 断开或退出时只取消指定连接；远端监督进程也会在心跳丢失后收尾。
pub(crate) fn cancel_connection(state: &ArchiveState, connection_id: Option<&str>) {
    if let Ok(jobs) = state.jobs.lock() {
        for job in jobs.values() {
            if connection_id.is_none_or(|id| id == job.connection_id) {
                let _ = job.cancel.send(true);
            }
        }
    }
}

/// 开始一个长任务，直到远端确认结束才返回；不得通过客户端浮层生命周期取消。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_archive_run(
    ssh_state: State<'_, SshState>,
    state: State<'_, ArchiveState>,
    connection_id: String,
    job_id: String,
    request: ArchiveRequest,
    progress: Channel<ArchiveEvent>,
) -> Result<ArchiveEvent, String> {
    if job_id.len() > 80
        || !job_id.starts_with("archive-")
        || !job_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("归档任务标识无效".into());
    }
    if request.paths.is_empty()
        || request.paths.len() > 100000
        || !["compress", "extract", "preview"].contains(&request.operation.as_str())
        || !["zip", "gz", "tar.gz"].contains(&request.format.as_str())
        || (request.operation != "compress" && request.paths.len() != 1)
    {
        return Err("归档请求无效".into());
    }
    let session = get_session(&ssh_state, &connection_id)?;
    let (cancel, mut cancelled) = watch::channel(false);
    let queue = {
        let mut queues = state.queues.lock().map_err(|e| e.to_string())?;
        queues.retain(|_, queue| Arc::strong_count(queue) > 1);
        queues.entry(connection_id.clone()).or_default().clone()
    };
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        if jobs.contains_key(&job_id) {
            return Err("任务标识重复".into());
        }
        jobs.insert(
            job_id.clone(),
            ArchiveJob {
                connection_id,
                cancel,
            },
        );
    }
    let result = async {
        progress
            .send(ArchiveEvent {
                kind: "queued".into(),
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
        let _guard = tokio::select! {
            guard = queue.lock() => guard,
            _ = cancelled.changed() => return Ok(cancelled_event()),
        };
        if *cancelled.borrow() {
            return Ok(cancelled_event());
        }
        stream::execute(session, request, progress, cancelled).await
    }
    .await;
    state
        .jobs
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&job_id);
    match &result {
        Ok(event) if matches!(event.status.as_deref(), Some("succeeded" | "cancelled")) => {
            log::info!("归档任务结束 job={job_id}")
        }
        Ok(_) => log::warn!("归档任务失败 job={job_id}"),
        Err(_) => log::warn!("归档任务未完成 job={job_id}"),
    }
    result
}

/// 取消已登记任务；命令返回只表示请求送达，最终状态仍由工作器确认。
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_archive_cancel(state: State<'_, ArchiveState>, job_id: String) -> Result<(), String> {
    if let Some(job) = state.jobs.lock().map_err(|e| e.to_string())?.get(&job_id) {
        let _ = job.cancel.send(true);
    }
    Ok(())
}

fn cancelled_event() -> ArchiveEvent {
    ArchiveEvent {
        kind: "result".into(),
        status: Some("cancelled".into()),
        ..Default::default()
    }
}
