//! 固定版本 Oracle 原生 agent 安装器。
//!
//! 产物来自 DBX `agents-v0.2.126`，只接受清单中固定的 0.1.66 tar.zst；
//! 解包使用 tar/zstd crate，不调用系统命令。tar 提取仅读取普通文件，不使用 unpack。

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex as AsyncMutex, Notify};

use crate::framework::storage::access;
use crate::plugins::database::models::{
    DbType, DriverInstallPhase, DriverInstallProgress, DriverKind, DriverStatus,
};

use super::client::{AgentChildHandle, AgentClient};
use super::manager::DriverStore;

const ORACLE_VERSION: &str = "0.1.66";
const DBX_RELEASE_TAG: &str = "agents-v0.2.126";
const R2_BASE: &str = "https://dl.dbxio.com/";
const GITHUB_RELEASE_BASE: &str = "https://github.com/t8y2/dbx/releases/download/";
const MAX_REGISTRY_BYTES: u64 = 1024 * 1024;
const MAX_DRIVER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 96 * 1024 * 1024;
const INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
const HTTP_TIMEOUT: Duration = Duration::from_secs(75);
const VALIDATION_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const CANCELLED: &str = "Oracle 驱动安装已取消";
const REQUEST_ACTIVE: u8 = 0;
const REQUEST_CANCELLED: u8 = 1;
const REQUEST_COMMITTING: u8 = 2;
const REQUEST_FINISHED: u8 = 3;

const ORACLE_ARTIFACTS: &[OracleArtifact] = &[
    OracleArtifact {
        platform: "windows-x64",
        filename: "dbx-agent-oracle-0.1.66-windows-x64.tar.zst",
        sha256: "d7c14ba82c6c9ec4ce5ac2d33912f17ea47b0acfdc08525e48e554a7d59e0e6c",
        size: 2_643_664,
    },
    OracleArtifact {
        platform: "windows-aarch64",
        filename: "dbx-agent-oracle-0.1.66-windows-aarch64.tar.zst",
        sha256: "d7ca63afb962c3292da3a9363e5085fb657be237908a794fd52a1ed717b09ea9",
        size: 2_389_571,
    },
    OracleArtifact {
        platform: "macos-x64",
        filename: "dbx-agent-oracle-0.1.66-macos-x64.tar.zst",
        sha256: "0a16b2a8b0b6c579091b350dd8b1225f4ecbf8fc4dcacaeb7f44a5cea31812d3",
        size: 2_672_196,
    },
    OracleArtifact {
        platform: "macos-aarch64",
        filename: "dbx-agent-oracle-0.1.66-macos-aarch64.tar.zst",
        sha256: "c29c5e6549d84250e20c6c63625dfb5897364e587f0bd77508e5d00fa0e3e3b5",
        size: 2_616_238,
    },
    OracleArtifact {
        platform: "linux-x64",
        filename: "dbx-agent-oracle-0.1.66-linux-x64.tar.zst",
        sha256: "56f3d60a9ed4372cc03f1aafc1b6242d58dfc8f72cb6eedd1dad732ef9334233",
        size: 2_599_568,
    },
    OracleArtifact {
        platform: "linux-aarch64",
        filename: "dbx-agent-oracle-0.1.66-linux-aarch64.tar.zst",
        sha256: "b8452fd09868d90c0e9aaa111b6ab72e525b4f3b5c0c7a533437f606031695bb",
        size: 2_358_482,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OracleArtifact {
    platform: &'static str,
    filename: &'static str,
    sha256: &'static str,
    size: u64,
}

/// Oracle 下载请求与其取消/验证子进程生命周期的 database owner 状态。
#[derive(Default)]
pub struct DriverInstallState {
    requests: Mutex<HashMap<String, Arc<InstallRequest>>>,
    oracle_gate: AsyncMutex<()>,
}

struct InstallRequest {
    request_id: String,
    db_type: String,
    progress: Channel<DriverInstallProgress>,
    cancelled: AtomicBool,
    lifecycle: AtomicU8,
    cancellation: Notify,
    finished: Notify,
    validation_child: Mutex<Option<AgentChildHandle>>,
}

impl InstallRequest {
    fn new(
        request_id: String,
        db_type: String,
        progress: Channel<DriverInstallProgress>,
    ) -> Self {
        Self {
            request_id,
            db_type,
            progress,
            cancelled: AtomicBool::new(false),
            lifecycle: AtomicU8::new(REQUEST_ACTIVE),
            cancellation: Notify::new(),
            finished: Notify::new(),
            validation_child: Mutex::new(None),
        }
    }

    fn emit(
        &self,
        phase: DriverInstallPhase,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    ) -> Result<(), String> {
        self.progress
            .send(DriverInstallProgress {
                request_id: self.request_id.clone(),
                db_type: self.db_type.clone(),
                phase,
                downloaded_bytes,
                total_bytes,
            })
            .map_err(|error| format!("发送 Oracle 驱动安装进度失败: {error}"))
    }

    fn cancel(&self) -> Result<(), String> {
        loop {
            match self.lifecycle.load(Ordering::Acquire) {
                REQUEST_ACTIVE => {
                    if self
                        .lifecycle
                        .compare_exchange(
                            REQUEST_ACTIVE,
                            REQUEST_CANCELLED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.cancelled.store(true, Ordering::Release);
                        self.cancellation.notify_waiters();
                        return self.kill_validation_child();
                    }
                }
                REQUEST_CANCELLED => {
                    self.cancelled.store(true, Ordering::Release);
                    self.cancellation.notify_waiters();
                    return self.kill_validation_child();
                }
                REQUEST_COMMITTING => {
                    return Err("Oracle 驱动已进入最终发布阶段，无法取消".into())
                }
                REQUEST_FINISHED => return Ok(()),
                _ => return Err("Oracle 驱动安装状态无效".into()),
            }
        }
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.cancellation.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }

    fn set_validation_child(&self, child: AgentChildHandle) -> Result<(), String> {
        {
            let mut current = self
                .validation_child
                .lock()
                .map_err(|_| "Oracle agent 验证进程表锁不可用".to_string())?;
            *current = Some(child);
        }
        // 取消先于登记时也保留弱句柄，取消分支才能等待 OS 确认进程退出。
        if self.cancelled.load(Ordering::Acquire) {
            self.kill_validation_child()?;
        }
        Ok(())
    }

    fn clear_validation_child(&self) -> Result<(), String> {
        let mut current = self
            .validation_child
            .lock()
            .map_err(|_| "Oracle agent 验证进程表锁不可用".to_string())?;
        *current = None;
        Ok(())
    }

    fn kill_validation_child(&self) -> Result<(), String> {
        let child = self
            .validation_child
            .lock()
            .map_err(|_| "Oracle agent 验证进程表锁不可用".to_string())?
            .as_ref()
            .and_then(Weak::upgrade);
        if let Some(child) = child {
            let mut child = child
                .lock()
                .map_err(|_| "Oracle agent 子进程锁不可用".to_string())?;
            if child
                .try_wait()
                .map_err(|error| format!("检查 Oracle agent 验证进程失败: {error}"))?
                .is_none()
            {
                child
                    .start_kill()
                    .map_err(|error| format!("结束 Oracle agent 验证进程失败: {error}"))?;
            }
        }
        Ok(())
    }

    async fn stop_validation_child(&self, wait: Duration) -> Result<bool, String> {
        let child = self
            .validation_child
            .lock()
            .map_err(|_| "Oracle agent 验证进程表锁不可用".to_string())?
            .as_ref()
            .and_then(Weak::upgrade);
        let Some(child) = child else {
            return Ok(false);
        };
        {
            let mut child = child
                .lock()
                .map_err(|_| "Oracle agent 子进程锁不可用".to_string())?;
            if child
                .try_wait()
                .map_err(|error| format!("检查 Oracle agent 验证进程失败: {error}"))?
                .is_some()
            {
                return Ok(true);
            }
            if let Err(error) = child.start_kill() {
                if child
                    .try_wait()
                    .map_err(|error| format!("检查 Oracle agent 验证进程失败: {error}"))?
                    .is_some()
                {
                    return Ok(true);
                }
                return Err(format!("结束 Oracle agent 验证进程失败: {error}"));
            }
        }
        tokio::time::timeout(wait, async {
            loop {
                let exited = {
                    child
                        .lock()
                        .map_err(|_| "Oracle agent 子进程锁不可用".to_string())?
                        .try_wait()
                        .map_err(|error| {
                            format!("等待 Oracle agent 验证进程退出失败: {error}")
                        })?
                        .is_some()
                };
                if exited {
                    return Ok::<_, String>(());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| "Oracle agent 验证进程未在时限内退出".to_string())??;
        Ok(true)
    }

    fn begin_commit(&self) -> Result<(), String> {
        match self.lifecycle.compare_exchange(
            REQUEST_ACTIVE,
            REQUEST_COMMITTING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(REQUEST_CANCELLED) => Err(CANCELLED.into()),
            Err(REQUEST_COMMITTING) => Err("Oracle 驱动已进入最终发布阶段".into()),
            Err(_) => Err("Oracle 驱动安装已结束".into()),
        }
    }

    fn finish(&self) {
        self.lifecycle.store(REQUEST_FINISHED, Ordering::Release);
        self.finished.notify_waiters();
    }

    async fn wait_finished(&self) {
        loop {
            let notified = self.finished.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.lifecycle.load(Ordering::Acquire) == REQUEST_FINISHED {
                return;
            }
            notified.await;
        }
    }
}

struct RequestGuard<'a> {
    state: &'a DriverInstallState,
    request: Arc<InstallRequest>,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        if !matches!(
            self.request.lifecycle.load(Ordering::Acquire),
            REQUEST_COMMITTING | REQUEST_FINISHED
        ) {
            if self.request.cancel().is_err() {
                log::warn!("Oracle 驱动请求收尾时取消验证进程失败");
            }
        }
        self.request.finish();
        match self.state.requests.lock() {
            Ok(mut requests) => {
                if requests
                    .get(&self.request.request_id)
                    .is_some_and(|registered| Arc::ptr_eq(registered, &self.request))
                {
                    requests.remove(&self.request.request_id);
                }
            }
            Err(_) => log::error!("Oracle 驱动安装请求表锁不可用"),
        }
    }
}

impl DriverInstallState {
    fn register(
        &self,
        request_id: String,
        db_type: String,
        progress: Channel<DriverInstallProgress>,
    ) -> Result<RequestGuard<'_>, String> {
        if request_id.is_empty()
            || request_id.len() > 128
            || !request_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err("驱动安装 requestId 格式无效".into());
        }
        if db_type.is_empty() || db_type.len() > 32 {
            return Err("驱动安装 dbType 格式无效".into());
        }
        let request = Arc::new(InstallRequest::new(request_id.clone(), db_type, progress));
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| "Oracle 驱动安装请求表锁不可用".to_string())?;
        if requests.contains_key(&request_id) {
            return Err("驱动安装 requestId 已在使用".into());
        }
        requests.insert(request_id, Arc::clone(&request));
        drop(requests);
        Ok(RequestGuard {
            state: self,
            request,
        })
    }

    /// 取消指定安装请求；等待同一 Oracle gate 的其它请求不会受影响。
    pub async fn cancel(&self, request_id: &str) -> Result<(), String> {
        let request = self
            .requests
            .lock()
            .map_err(|_| "Oracle 驱动安装请求表锁不可用".to_string())?
            .get(request_id)
            .cloned();
        if let Some(request) = request {
            request.cancel()?;
            request.wait_finished().await;
        }
        Ok(())
    }

    /// 同步取消当前 owner 的安装请求并终止其验证子进程。
    pub fn cancel_all(&self) -> Vec<String> {
        let requests = match self.requests.lock() {
            Ok(requests) => requests.values().cloned().collect::<Vec<_>>(),
            Err(_) => return vec!["Oracle 驱动安装请求表锁不可用".into()],
        };
        requests
            .into_iter()
            .filter_map(|request| match request.cancel() {
                Err(_)
                    if matches!(
                        request.lifecycle.load(Ordering::Acquire),
                        REQUEST_COMMITTING | REQUEST_FINISHED
                    ) =>
                {
                    None
                }
                Err(error) => Some(error),
                Ok(()) => None,
            })
            .collect()
    }
}

/// 返回当前平台固定的 DBX Oracle 0.1.66 安装产物。
fn current_artifact() -> Option<&'static OracleArtifact> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let platform = "windows-x64";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    let platform = "windows-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let platform = "macos-x64";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let platform = "macos-aarch64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let platform = "linux-x64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let platform = "linux-aarch64";
    #[cfg(not(any(
        all(target_os = "windows", any(target_arch = "x86_64", target_arch = "aarch64")),
        all(target_os = "macos", any(target_arch = "x86_64", target_arch = "aarch64")),
        all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))
    )))]
    let platform = "";
    ORACLE_ARTIFACTS.iter().find(|artifact| artifact.platform == platform)
}

/// 返回本地驱动可用状态及该类型是否支持 Oracle 安装器。
pub fn status(store: &DriverStore, db_type: DbType) -> Result<DriverStatus, String> {
    if !db_type.is_agent() {
        return Ok(DriverStatus {
            ready: true,
            kind: DriverKind::Native,
            dir: None,
            version: None,
            auto_install: None,
        });
    }
    let dir = store.driver_dir(db_type)?;
    let version = store
        .versions()?
        .get(super::driver_key(db_type))
        .cloned();
    Ok(DriverStatus {
        ready: store.agent_binary(db_type).is_some(),
        kind: DriverKind::Agent,
        dir: Some(dir.display().to_string()),
        version,
        auto_install: (db_type == DbType::Oracle).then_some(current_artifact().is_some()),
    })
}

/// 安装固定 Oracle agent；请求注册及等待进度发送都在状态检查之前完成。
pub async fn install_driver(
    app: &tauri::AppHandle,
    state: &DriverInstallState,
    db_type_name: String,
    request_id: String,
    progress: Channel<DriverInstallProgress>,
) -> Result<DriverStatus, String> {
    let deadline = tokio::time::Instant::now() + INSTALL_TIMEOUT;
    let started = std::time::Instant::now();
    let guard = state.register(request_id, db_type_name.clone(), progress)?;
    let request = Arc::clone(&guard.request);
    request.emit(DriverInstallPhase::Waiting, 0, None)?;

    let result = async {
        let db_type = DbType::parse(&db_type_name)
            .ok_or_else(|| format!("未知数据库类型：{db_type_name}"))?;
        if db_type != DbType::Oracle {
            return Err("自动安装目前只支持 Oracle agent 驱动".into());
        }
        let store = DriverStore::new(app)?;
        let current_status = status(&store, db_type)?;
        if current_status.ready {
            request.finish();
            let _ = request.emit(DriverInstallPhase::Complete, 0, None);
            return Ok(current_status);
        }
        let artifact = current_artifact()
            .ok_or_else(|| "当前平台暂不支持自动安装 Oracle agent 驱动".to_string())?;

        let _gate = tokio::select! {
            biased;
            _ = request.cancelled() => return Err(CANCELLED.into()),
            _ = tokio::time::sleep_until(deadline) => return Err("Oracle 驱动安装等待超时".into()),
            guard = state.oracle_gate.lock() => guard,
        };
        if request.cancelled.load(Ordering::Acquire) {
            return Err(CANCELLED.into());
        }
        let current_status = status(&store, db_type)?;
        if current_status.ready {
            request.finish();
            let _ = request.emit(DriverInstallPhase::Complete, 0, None);
            return Ok(current_status);
        }

        log::info!(
            "Oracle 驱动安装开始 version={ORACLE_VERSION} platform={}",
            artifact.platform
        );
        install_oracle(&store, artifact, Arc::clone(&request), deadline).await?;
        let installed = status(&store, DbType::Oracle)?;
        if !installed.ready {
            return Err("Oracle 驱动发布后未检测到可执行文件".into());
        }
        let _ = request.emit(
            DriverInstallPhase::Complete,
            artifact.size,
            Some(artifact.size),
        );
        Ok(installed)
    }
    .await;
    request.finish();
    match &result {
        Ok(_) => log::info!(
            "Oracle 驱动安装操作完成 elapsed_ms={}",
            started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "Oracle 驱动安装操作未完成 elapsed_ms={}",
            started.elapsed().as_millis()
        ),
    }
    drop(guard);
    result
}

async fn install_oracle(
    store: &DriverStore,
    artifact: &'static OracleArtifact,
    request: Arc<InstallRequest>,
    deadline: tokio::time::Instant,
) -> Result<(), String> {
    let prepare_store = store.clone();
    let prepare_request = Arc::clone(&request);
    let prepare = access::spawn_blocking(move || {
        ensure_not_cancelled(&prepare_request.cancelled)?;
        let driver_dir = prepare_store.driver_dir(DbType::Oracle)?;
        let workspace = TempWorkspace::create(&driver_dir)?;
        ensure_not_cancelled(&prepare_request.cancelled)?;
        let licenses = workspace.path.join("LICENSES.txt");
        fs::write(&licenses, include_str!("licenses/dbx-oracle.txt"))
            .map_err(|error| format!("写入 Oracle agent 许可文件失败: {error}"))?;
        Ok::<_, String>((driver_dir, workspace))
    });
    tokio::pin!(prepare);
    let (driver_dir, workspace) = tokio::select! {
        biased;
        _ = request.cancelled() => {
            let _ = prepare.await;
            return Err(CANCELLED.into());
        }
        _ = tokio::time::sleep_until(deadline) => {
            let _ = request.cancel();
            let _ = prepare.await;
            return Err("Oracle 驱动安装准备超时".into());
        }
        result = &mut prepare => result.map_err(|error| format!("准备 Oracle 驱动目录失败: {error}"))??,
    };
    let archive_path = workspace.path.join("oracle.tar.zst");
    let staged_binary = workspace.path.join(if cfg!(windows) {
        "agent.staged.exe"
    } else {
        "agent.staged"
    });
    let staged_licenses = workspace.path.join("LICENSES.txt");

    download_artifact(artifact, &archive_path, &request, deadline).await?;
    if request.cancelled.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    request.emit(
        DriverInstallPhase::Extracting,
        artifact.size,
        Some(artifact.size),
    )?;

    // 将唯一临时目录交给真正工作的阻塞线程；即使 IPC future 被丢弃，
    // access::spawn_blocking 仍保留存储租约，工作线程也拥有目录清理责任。
    let extract_request = Arc::clone(&request);
    let extract_artifact = *artifact;
    let workspace_for_worker = workspace;
    let archive_for_worker = archive_path.clone();
    let staged_for_worker = staged_binary.clone();
    let worker = access::spawn_blocking(move || {
        let result = extract_and_validate_package(
            &archive_for_worker,
            &staged_for_worker,
            extract_artifact,
            &extract_request.cancelled,
        )
        .and_then(|()| set_executable(&staged_for_worker));
        (result, workspace_for_worker)
    });
    tokio::pin!(worker);
    let (extract_result, workspace) = tokio::select! {
        biased;
        _ = request.cancelled() => {
            let _ = worker.await;
            return Err(CANCELLED.into());
        }
        _ = tokio::time::sleep_until(deadline) => {
            let _ = request.cancel();
            let _ = worker.await;
            return Err("Oracle 驱动解包超时".into());
        }
        result = &mut worker => result.map_err(|error| format!("Oracle agent 解包任务失败: {error}"))?,
    };
    extract_result?;
    if request.cancelled.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }

    request.emit(
        DriverInstallPhase::Validating,
        artifact.size,
        Some(artifact.size),
    )?;
    validate_agent_process(&staged_binary, &workspace.path, &request, deadline).await?;
    if request.cancelled.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }

    let store_for_publish = store.clone();
    let request_for_publish = Arc::clone(&request);
    let binary_for_publish = staged_binary.clone();
    let licenses_for_publish = staged_licenses.clone();
    let workspace_for_publish = workspace;
    let deadline_std = deadline.into_std();
    let publisher = access::spawn_blocking(move || {
        let result = (|| {
            if std::time::Instant::now() >= deadline_std {
                return Err("Oracle 驱动安装超过总时间限制".into());
            }
            ensure_not_cancelled(&request_for_publish.cancelled)?;
            let target_binary = store_for_publish.target_binary_path(DbType::Oracle)?;
            let target_licenses = target_binary
                .parent()
                .ok_or("Oracle agent 目标目录无效")?
                .join("LICENSES.txt");
            publish_install(
                &store_for_publish,
                &request_for_publish,
                &binary_for_publish,
                &licenses_for_publish,
                &target_binary,
                &target_licenses,
                deadline_std,
            )
        })();
        (result, workspace_for_publish)
    });
    let (publish_result, workspace) = publisher
        .await
        .map_err(|error| format!("发布 Oracle 驱动任务失败: {error}"))?;
    let installed = publish_result?;
    if installed {
        log::info!("Oracle agent 驱动发布完成 version={ORACLE_VERSION}");
    } else {
        log::info!("Oracle agent 驱动已由并发操作准备就绪");
    }
    drop(workspace);
    Ok(())
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("读取 Oracle agent 权限失败: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置 Oracle agent 可执行权限失败: {error}"))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn validate_agent_process(
    program: &Path,
    working_dir: &Path,
    request: &Arc<InstallRequest>,
    deadline: tokio::time::Instant,
) -> Result<(), String> {
    let spawn_request = Arc::clone(request);
    let mut spawn = Box::pin(AgentClient::spawn_observed(
        program,
        working_dir,
        move |child| spawn_request.set_validation_child(child),
    ));
    let spawn_result = tokio::select! {
        biased;
        _ = request.cancelled() => {
            return match stop_and_clear_validation_child(request).await {
                Ok(()) => Err(CANCELLED.into()),
                Err(error) => Err(format!("{CANCELLED}；清理 agent 验证进程失败: {error}")),
            };
        }
        _ = tokio::time::sleep_until(deadline) => {
            let _ = request.cancel();
            return match stop_and_clear_validation_child(request).await {
                Ok(()) => Err("Oracle agent 启动验证超时".into()),
                Err(error) => Err(format!("Oracle agent 启动验证超时；清理验证进程失败: {error}")),
            };
        }
        result = &mut spawn => result,
    };
    let client = match spawn_result {
        Ok(client) => Arc::new(client),
        Err(error) => {
            return match stop_and_clear_validation_child(request).await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!("{error}；清理验证进程失败: {cleanup}")),
            };
        }
    };
    let handshake = tokio::select! {
        biased;
        _ = request.cancelled() => Err(CANCELLED.to_string()),
        _ = tokio::time::sleep_until(deadline) => {
            let _ = request.cancel();
            Err("Oracle agent 协议验证超时".to_string())
        }
        result = client.handshake() => result,
    };
    let stopped = client.kill_and_wait(VALIDATION_STOP_TIMEOUT).await;
    let cleared = request.clear_validation_child();
    stopped?;
    cleared?;
    handshake
}

async fn stop_and_clear_validation_child(request: &InstallRequest) -> Result<(), String> {
    request
        .stop_validation_child(VALIDATION_STOP_TIMEOUT)
        .await?;
    request.clear_validation_child()
}

async fn download_artifact(
    artifact: &OracleArtifact,
    destination: &Path,
    request: &InstallRequest,
    deadline: tokio::time::Instant,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|error| format!("创建 Oracle 驱动下载器失败: {error}"))?;
    let r2_url = format!(
        "{R2_BASE}agents/drivers/{}?v={ORACLE_VERSION}",
        artifact.filename
    );
    let github_url = format!(
        "{GITHUB_RELEASE_BASE}{DBX_RELEASE_TAG}/{}",
        artifact.filename
    );
    let candidates = [("r2", r2_url), ("github", github_url)];
    let mut failures = Vec::new();
    for (source, url) in candidates {
        if request.cancelled.load(Ordering::Acquire) {
            return Err(CANCELLED.into());
        }
        match download_one(&client, &url, destination, artifact, request, deadline).await {
            Ok(()) => return Ok(()),
            Err(DownloadFailure::Cancelled) => return Err(CANCELLED.into()),
            Err(DownloadFailure::Progress(error)) => return Err(error),
            Err(DownloadFailure::Local(error)) => return Err(error),
            Err(DownloadFailure::TimedOut) => return Err("Oracle 驱动下载超过总时间限制".into()),
            Err(DownloadFailure::Source(reason)) => {
                log::warn!("Oracle 驱动官方下载源失败 source={source} reason={reason}");
                failures.push(format!("{source}: {reason}"));
            }
        }
    }
    Err(format!("Oracle 驱动官方下载源均失败（{}）", failures.join("；")))
}

enum DownloadFailure {
    Cancelled,
    Progress(String),
    Local(String),
    Source(String),
    TimedOut,
}

async fn download_one(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    artifact: &OracleArtifact,
    request: &InstallRequest,
    deadline: tokio::time::Instant,
) -> Result<(), DownloadFailure> {
    request
        .emit(
            DriverInstallPhase::Downloading,
            0,
            Some(artifact.size),
        )
        .map_err(DownloadFailure::Progress)?;
    let response = tokio::select! {
        biased;
        _ = request.cancelled() => return Err(DownloadFailure::Cancelled),
        _ = tokio::time::sleep_until(deadline) => return Err(DownloadFailure::TimedOut),
        response = client
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                concat!("CoveKit/", env!("CARGO_PKG_VERSION")),
            )
            .send() =>
        {
            response.map_err(|_| DownloadFailure::Source("网络请求失败".into()))?
        }
    };
    if response.status() != StatusCode::OK {
        return Err(DownloadFailure::Source(format!("HTTP {}", response.status().as_u16())));
    }
    if response
        .content_length()
        .is_some_and(|length| length != artifact.size)
    {
        return Err(DownloadFailure::Source("Content-Length 与固定大小不符".into()));
    }
    let mut response = response;
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| DownloadFailure::Local(format!("创建下载临时文件失败: {error}")))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    loop {
        let chunk = tokio::select! {
            biased;
            _ = request.cancelled() => return Err(DownloadFailure::Cancelled),
            _ = tokio::time::sleep_until(deadline) => return Err(DownloadFailure::TimedOut),
            chunk = response.chunk() => chunk.map_err(|_| DownloadFailure::Source("读取响应内容失败".into()))?,
        };
        let Some(chunk) = chunk else { break };
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > artifact.size {
            return Err(DownloadFailure::Source("下载内容超过固定大小".into()));
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| DownloadFailure::Local(format!("写入驱动下载文件失败: {error}")))?;
        hasher.update(&chunk);
        request
            .emit(
                DriverInstallPhase::Downloading,
                downloaded,
                Some(artifact.size),
            )
            .map_err(DownloadFailure::Progress)?;
    }
    file.flush()
        .await
        .map_err(|error| DownloadFailure::Local(format!("刷新驱动下载文件失败: {error}")))?;
    file.sync_all()
        .await
        .map_err(|error| DownloadFailure::Local(format!("同步驱动下载文件失败: {error}")))?;
    drop(file);
    if tokio::time::Instant::now() >= deadline {
        return Err(DownloadFailure::TimedOut);
    }
    request
        .emit(
            DriverInstallPhase::Verifying,
            downloaded,
            Some(artifact.size),
        )
        .map_err(DownloadFailure::Progress)?;
    if downloaded != artifact.size {
        return Err(DownloadFailure::Source("下载字节数与固定大小不符".into()));
    }
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(artifact.sha256) {
        return Err(DownloadFailure::Source("SHA-256 与固定摘要不符".into()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct PackageRegistry {
    #[serde(default)]
    jres: HashMap<String, serde_json::Value>,
    drivers: HashMap<String, PackageDriver>,
}

#[derive(Debug, Deserialize)]
struct PackageDriver {
    version: String,
    #[serde(default)]
    native: HashMap<String, PackageArtifact>,
    #[serde(default)]
    jar: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PackageArtifact {
    url: String,
    sha256: Option<String>,
    size: u64,
    format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageInfo {
    entry_name: String,
    binary_sha256: String,
    binary_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TarEntryInfo {
    name: String,
    is_file: bool,
    is_dir: bool,
    size: u64,
}

fn inspect_package(
    archive_path: &Path,
    artifact: &OracleArtifact,
    cancelled: &AtomicBool,
) -> Result<PackageInfo, String> {
    let file = File::open(archive_path).map_err(|error| format!("打开 Oracle 驱动包失败: {error}"))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .map_err(|error| format!("Oracle 驱动包不是有效 zstd 数据: {error}"))?;
    let mut archive = tar::Archive::new(decoder);
    let mut registry_bytes = None;
    let mut entries = Vec::new();
    let mut names = HashSet::new();
    let mut total_size = 0_u64;
    let tar_entries = archive
        .entries()
        .map_err(|error| format!("读取 Oracle tar 清单失败: {error}"))?;
    for entry in tar_entries {
        ensure_not_cancelled(cancelled)?;
        let mut entry = entry.map_err(|error| format!("Oracle tar 文件项损坏: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("读取 Oracle tar 路径失败: {error}"))?;
        let name = safe_entry_name(&path)?;
        if !names.insert(name.clone()) {
            return Err(format!("Oracle 驱动包包含重复路径: {name}"));
        }
        let kind = entry.header().entry_type();
        let info = TarEntryInfo {
            name: name.clone(),
            is_file: kind.is_file(),
            is_dir: kind.is_dir(),
            size: entry.size(),
        };
        total_size = total_size
            .checked_add(info.size)
            .filter(|size| *size <= MAX_PACKAGE_BYTES)
            .ok_or("Oracle 驱动包解压大小超过 96 MiB 上限")?;
        if name == "agent-registry.json" {
            if !info.is_file || info.size > MAX_REGISTRY_BYTES {
                return Err("Oracle 驱动包 registry 必须是小于 1 MiB 的普通文件".into());
            }
            let mut bytes = Vec::with_capacity(info.size as usize);
            entry
                .take(MAX_REGISTRY_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("读取 Oracle 包 registry 失败: {error}"))?;
            if bytes.len() as u64 > MAX_REGISTRY_BYTES {
                return Err("Oracle 驱动包 registry 超过 1 MiB".into());
            }
            registry_bytes = Some(bytes);
        } else if name != "drivers" && !name.starts_with("drivers/") {
            return Err(format!("Oracle 驱动包包含意外路径: {name}"));
        }
        if info.size > MAX_DRIVER_BYTES {
            return Err("Oracle 驱动包中的文件超过 64 MiB 上限".into());
        }
        entries.push(info);
        if entries.len() > 8 {
            return Err("Oracle 驱动包文件项过多".into());
        }
    }
    let bytes = registry_bytes.ok_or("Oracle 驱动包缺少 agent-registry.json")?;
    let registry: PackageRegistry = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Oracle 包 registry 无效: {error}"))?;
    if !registry.jres.is_empty() {
        return Err("Oracle 原生驱动包不应携带 JRE".into());
    }
    if registry.drivers.len() != 1 {
        return Err("Oracle 驱动包必须只包含一个数据库驱动".into());
    }
    let driver = registry
        .drivers
        .get("oracle")
        .ok_or("Oracle 驱动包数据库类型不匹配")?;
    if driver.version != ORACLE_VERSION {
        return Err(format!("Oracle 驱动包版本不匹配: {}", driver.version));
    }
    if driver.jar.is_some() || driver.native.len() != 1 {
        return Err("Oracle 驱动包必须只包含一个原生平台产物".into());
    }
    let packaged_artifact = driver
        .native
        .get(artifact.platform)
        .ok_or_else(|| format!("Oracle 驱动包不支持平台 {}", artifact.platform))?;
    if packaged_artifact.format.is_some() {
        return Err("Oracle 包 registry 不得嵌套压缩包".into());
    }
    if packaged_artifact.size == 0 || packaged_artifact.size > MAX_DRIVER_BYTES {
        return Err("Oracle agent 二进制大小超出限制".into());
    }
    let binary_sha256 = packaged_artifact
        .sha256
        .as_deref()
        .filter(|value| is_sha256(value))
        .ok_or("Oracle 包 registry 缺少有效 agent SHA-256")?
        .to_ascii_lowercase();
    let filename = artifact_filename(&packaged_artifact.url)?;
    let entry_name = format!("drivers/{filename}");
    validate_tar_entries(&entries, &entry_name, packaged_artifact.size)?;
    Ok(PackageInfo {
        entry_name,
        binary_sha256,
        binary_size: packaged_artifact.size,
    })
}

fn safe_entry_name(path: &Path) -> Result<String, String> {
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!("Oracle 驱动包含不安全路径: {}", path.display()));
        }
    }
    let name = path
        .to_str()
        .filter(|name| !name.is_empty() && !name.contains('\\') && !name.starts_with('/'))
        .ok_or_else(|| format!("Oracle 驱动包路径无效: {}", path.display()))?;
    if name.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(format!("Oracle 驱动包路径无效: {name}"));
    }
    Ok(name.to_string())
}

fn artifact_filename(url: &str) -> Result<&str, String> {
    if url.is_empty()
        || !url.is_ascii()
        || !url
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || url.contains('/')
        || url.contains('\\')
        || url.contains(':')
        || url.contains('?')
        || url.contains('#')
        || url == "."
        || url == ".."
    {
        return Err("Oracle 包 registry 的 agent 文件名无效".into());
    }
    Ok(url)
}

fn validate_tar_entries(
    entries: &[TarEntryInfo],
    expected_binary: &str,
    expected_size: u64,
) -> Result<(), String> {
    let mut registry = false;
    let mut driver = false;
    for entry in entries {
        match entry.name.as_str() {
            "agent-registry.json" if !registry && entry.is_file && !entry.is_dir => {
                registry = true;
            }
            "drivers" if entry.is_dir && !entry.is_file && entry.size == 0 => {}
            name if name == expected_binary && !driver && entry.is_file && !entry.is_dir => {
                if entry.size != expected_size {
                    return Err(format!(
                        "Oracle agent 文件大小不匹配: 预期 {expected_size}，实际 {}",
                        entry.size
                    ));
                }
                driver = true;
            }
            name => return Err(format!("Oracle 驱动包条目类型或名称不允许: {name}")),
        }
    }
    if !registry || !driver || entries.len() > 3 {
        return Err("Oracle 驱动包文件清单不完整".into());
    }
    Ok(())
}

fn extract_and_validate_package(
    archive_path: &Path,
    destination: &Path,
    artifact: OracleArtifact,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let package = inspect_package(archive_path, &artifact, cancelled)?;
    ensure_not_cancelled(cancelled)?;
    let file = File::open(archive_path).map_err(|error| format!("打开 Oracle 驱动包失败: {error}"))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .map_err(|error| format!("Oracle 驱动包不是有效 zstd 数据: {error}"))?;
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("读取 Oracle 驱动包失败: {error}"))?;
    let mut extracted = false;
    for entry in entries {
        ensure_not_cancelled(cancelled)?;
        let mut entry = entry.map_err(|error| format!("Oracle 驱动包文件项损坏: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("读取 Oracle 驱动包路径失败: {error}"))?;
        let name = safe_entry_name(&path)?;
        let kind = entry.header().entry_type();
        if name == "agent-registry.json" || (name == "drivers" && kind.is_dir()) {
            continue;
        }
        if name != package.entry_name || !kind.is_file() || extracted {
            return Err(format!("Oracle 驱动包条目与清单不匹配: {name}"));
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|error| format!("创建 Oracle agent 暂存文件失败: {error}"))?;
        let mut hasher = Sha256::new();
        let mut copied = 0_u64;
        let mut buffer = [0_u8; 32 * 1024];
        loop {
            ensure_not_cancelled(cancelled)?;
            let read = entry
                .read(&mut buffer)
                .map_err(|error| format!("读取 Oracle agent 文件失败: {error}"))?;
            if read == 0 {
                break;
            }
            copied = copied.saturating_add(read as u64);
            if copied > package.binary_size || copied > MAX_DRIVER_BYTES {
                return Err("Oracle agent 解包内容超过清单大小".into());
            }
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("写入 Oracle agent 暂存文件失败: {error}"))?;
            hasher.update(&buffer[..read]);
        }
        output
            .flush()
            .and_then(|_| output.sync_all())
            .map_err(|error| format!("同步 Oracle agent 暂存文件失败: {error}"))?;
        drop(output);
        if copied != package.binary_size {
            return Err("Oracle agent 解包大小与 registry 不符".into());
        }
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(&package.binary_sha256) {
            return Err("Oracle agent 二进制 SHA-256 与 registry 不符".into());
        }
        extracted = true;
    }
    if !extracted {
        return Err("Oracle 驱动包中找不到 agent 二进制".into());
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn create(parent: &Path) -> Result<Self, String> {
        for _ in 0..3 {
            let path = parent.join(format!(".oracle-install-{}", uuid::Uuid::new_v4()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("创建 Oracle 驱动临时目录失败: {error}")),
            }
        }
        Err("创建 Oracle 驱动临时目录失败：唯一目录名冲突".into())
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Oracle 驱动临时目录清理失败 kind={:?}", error.kind());
            }
        }
    }
}

fn publish_install(
    store: &DriverStore,
    request: &InstallRequest,
    staged_binary: &Path,
    staged_licenses: &Path,
    target_binary: &Path,
    target_licenses: &Path,
    deadline: std::time::Instant,
) -> Result<bool, String> {
    if std::time::Instant::now() >= deadline {
        return Err("Oracle 驱动安装超过总时间限制".into());
    }
    ensure_not_cancelled(&request.cancelled)?;
    request.begin_commit()?;

    // 以 create-new 语义创建硬链接，避免覆盖其它进程刚准备好的驱动。
    if target_regular_file(target_binary)? {
        return Ok(false);
    }
    match fs::hard_link(staged_binary, target_binary) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return if target_regular_file(target_binary)? {
                Ok(false)
            } else {
                Err("Oracle agent 发布目标已被非普通文件占用".into())
            };
        }
        Err(error) => return Err(format!("原子发布 Oracle agent 失败: {error}")),
    }

    let suffix = uuid::Uuid::new_v4();
    let license_backup = target_licenses.with_file_name(format!(".licenses-backup-{suffix}"));
    let had_licenses = match backup_existing(target_licenses, &license_backup) {
        Ok(value) => value,
        Err(error) => {
            let rollback = remove_published_file(target_binary, "Oracle agent");
            return combine_rollback_errors(error, rollback);
        }
    };
    if let Err(error) = fs::rename(staged_licenses, target_licenses) {
        let rollback = rollback_publish(
            target_binary,
            target_licenses,
            &license_backup,
            had_licenses,
            false,
        );
        return combine_rollback_errors(format!("发布 Oracle agent 许可文件失败: {error}"), rollback);
    }
    if let Err(error) = store.record_version("oracle", ORACLE_VERSION) {
        let rollback = rollback_publish(
            target_binary,
            target_licenses,
            &license_backup,
            had_licenses,
            true,
        );
        return combine_rollback_errors(format!("登记 Oracle 驱动版本失败: {error}"), rollback);
    }
    if had_licenses {
        remove_backup(&license_backup);
    }
    Ok(true)
}

fn target_regular_file(target: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(format!("Oracle agent 发布目标不是普通文件: {}", target.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("检查 Oracle agent 发布目标失败: {error}")),
    }
}

fn backup_existing(target: &Path, backup: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
            fs::rename(target, backup).map_err(|error| format!("暂存原驱动文件失败: {error}"))?;
            Ok(true)
        }
        Ok(_) => Err(format!("驱动安装目标不是普通文件: {}", target.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("检查原驱动文件失败: {error}")),
    }
}

fn restore_backup(backup: &Path, target: &Path, had_previous: bool) -> Result<(), String> {
    if had_previous {
        fs::rename(backup, target)
            .map_err(|error| format!("恢复原驱动文件 {} 失败: {error}", target.display()))?;
    }
    Ok(())
}

fn remove_backup(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Oracle 驱动旧文件备份暂未清理 kind={:?}", error.kind());
        }
    }
}

fn remove_published_file(path: &Path, description: &str) -> Vec<String> {
    match fs::remove_file(path) {
        Ok(()) => Vec::new(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => vec![format!("回滚已发布的 {description} 失败: {error}")],
    }
}

fn rollback_publish(
    target_binary: &Path,
    target_licenses: &Path,
    license_backup: &Path,
    had_licenses: bool,
    license_published: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    if license_published {
        errors.extend(remove_published_file(target_licenses, "Oracle agent 许可文件"));
    }
    if had_licenses {
        if let Err(error) = restore_backup(license_backup, target_licenses, true) {
            errors.push(error);
        }
    }
    errors.extend(remove_published_file(target_binary, "Oracle agent"));
    errors
}

fn combine_rollback_errors(error: String, rollback: Vec<String>) -> Result<bool, String> {
    if rollback.is_empty() {
        Err(error)
    } else {
        Err(format!("{error}；{}", rollback.join("；")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_oracle_artifact_mapping_covers_supported_release_platforms() {
        assert_eq!(ORACLE_ARTIFACTS.len(), 6);
        assert!(ORACLE_ARTIFACTS
            .iter()
            .all(|artifact| artifact.filename.ends_with(".tar.zst") && is_sha256(artifact.sha256)));
        assert!(ORACLE_ARTIFACTS
            .iter()
            .any(|artifact| artifact.platform == "windows-aarch64"));
        assert!(ORACLE_ARTIFACTS
            .iter()
            .any(|artifact| artifact.platform == "macos-aarch64"));
    }

    #[test]
    fn package_path_validation_rejects_traversal_and_nonlocal_names() {
        assert!(safe_entry_name(Path::new("drivers/agent.exe")).is_ok());
        assert!(safe_entry_name(Path::new("../agent.exe")).is_err());
        assert!(safe_entry_name(Path::new("drivers\\agent.exe")).is_err());
        assert!(artifact_filename("https://example.invalid/agent.exe").is_err());
    }

    #[test]
    fn package_file_list_rejects_symlinks_duplicates_and_unexpected_files() {
        let valid = vec![
            TarEntryInfo {
                name: "agent-registry.json".into(),
                is_file: true,
                is_dir: false,
                size: 100,
            },
            TarEntryInfo {
                name: "drivers/agent.exe".into(),
                is_file: true,
                is_dir: false,
                size: 10,
            },
        ];
        assert!(validate_tar_entries(&valid, "drivers/agent.exe", 10).is_ok());
        let mut duplicate = valid.clone();
        duplicate.push(valid[1].clone());
        assert!(validate_tar_entries(&duplicate, "drivers/agent.exe", 10).is_err());
        let mut symlink = valid.clone();
        symlink[1].is_file = false;
        assert!(validate_tar_entries(&symlink, "drivers/agent.exe", 10).is_err());
        let mut unexpected = valid.clone();
        unexpected.push(TarEntryInfo {
            name: "drivers/extra".into(),
            is_file: true,
            is_dir: false,
            size: 1,
        });
        assert!(validate_tar_entries(&unexpected, "drivers/agent.exe", 10).is_err());
    }

    #[tokio::test]
    async fn cancellation_before_wait_is_observed_without_lost_notification() {
        let request = InstallRequest::new(
            "cancel-test".into(),
            "oracle".into(),
            Channel::new(|_| Ok(())),
        );
        request.cancel().unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(50), request.cancelled())
            .await
            .is_ok());
    }

    #[test]
    fn cancelled_request_cannot_cross_the_publish_commit_point() {
        let request = InstallRequest::new(
            "commit-cancel-test".into(),
            "oracle".into(),
            Channel::new(|_| Ok(())),
        );
        request.cancel().unwrap();
        assert!(request.begin_commit().is_err());
    }

    #[test]
    fn damaged_archive_is_rejected_before_it_can_be_published() {
        let path = std::env::temp_dir().join(format!(
            "oracle-damaged-{}.tar.zst",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, b"not a zstd archive").unwrap();
        let cancelled = AtomicBool::new(false);
        let result = inspect_package(&path, &ORACLE_ARTIFACTS[0], &cancelled);
        let _ = fs::remove_file(&path);
        assert!(result.is_err());
    }
}
