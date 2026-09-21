//! frp 插件 · 运行态（状态机 / 启停 / 状态查询 / 日志转发）
//! 状态机按任务书 §5.2 迁移表实现为纯函数 `next_state`（可单测，不碰真实进程）；进程侧用
//! `tokio::process` spawn `frpc -c <abs>`，监控任务独占 Child、按行推送 `frp://log`，状态迁移推送
//! `frp://state`。Rust 侧不长期缓存日志（只留最后一行），环形缓冲交给前端；日志与 lastError 的
//! 文本归一化（剥 ANSI + 敏感值打码）在 `verify` 模块，两侧共用同一份实现。

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

use crate::plugins::frp::models::{FrpLogPayload, FrpLogStream, FrpRuntimeState, FrpStateName};
use crate::plugins::frp::{clients, profile};

/// `starting` 超时（秒）：迟迟不见成功/失败行即判 error，避免假绿灯
const STARTUP_TIMEOUT_SECS: u64 = 30;
/// 停止时等待监控任务收尾的上限（秒）：也是「停止命令不阻塞 UI」的兜底
const STOP_WAIT_SECS: u64 = 3;
/// 成功行关键字（小写包含匹配）：出现即认为已连上服务端
const SUCCESS_MARKERS: &[&str] = &["login to server success", "start proxy success"];
/// 失败行关键字（小写包含匹配）：出现即转 error，该行脱敏后写入 `lastError`
const FAILURE_MARKERS: &[&str] = &[
    "login to server failed",
    "token is incorrect",
    "authentication failed",
    "port already used",
    "bind: address already in use",
    "start error",
];

/// 状态机事件（覆盖任务书 §5.2 迁移表每一行触发）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrpEvent<'a> {
    /// 启动事件（仅测试断言用：`frp_start` 内部走 `Starting` 直推，不构造本事件）
    #[cfg(test)]
    Spawned,
    /// 收到一行日志（已剥 ANSI）
    Line(&'a str),
    /// 进程自然退出（`None` = 未取得退出码，如被强杀）
    Exited(Option<i32>),
    /// 由本工具发起停止后进程结束（收尾为 stopped 而非 error）
    ExitedByStop,
    /// `starting` 持续超时
    StartupTimeout,
}

/// 状态机判定（纯函数）：返回新状态与可选的 `lastError` 更新；未知行不改状态
pub fn next_state(current: FrpStateName, event: &FrpEvent<'_>) -> (FrpStateName, Option<String>) {
    match event {
        // 测试专用：等同「刚把进程拉起来」，任何状态都推进到 starting
        #[cfg(test)]
        FrpEvent::Spawned => (FrpStateName::Starting, None),
        FrpEvent::Line(text) => {
            let lower = text.to_ascii_lowercase();
            if SUCCESS_MARKERS.iter().any(|marker| lower.contains(marker)) {
                (FrpStateName::Running, None)
            } else if FAILURE_MARKERS.iter().any(|marker| lower.contains(marker)) {
                (
                    FrpStateName::Error,
                    Some(crate::plugins::frp::verify::clean_line(text)),
                )
            } else {
                // 信息/调试日志保持现态：frpc 正常也会静默，不制造假红灯
                (current, None)
            }
        }
        FrpEvent::Exited(Some(0)) | FrpEvent::ExitedByStop => (FrpStateName::Stopped, None),
        FrpEvent::Exited(code) => (
            FrpStateName::Error,
            Some(match code {
                Some(code) => format!("frpc 异常退出（退出码 {code}）"),
                None => "frpc 进程被终止（未获得退出码）".to_string(),
            }),
        ),
        // 超时判定后计时器失活：非 starting 的迟到超时不再改状态
        FrpEvent::StartupTimeout if current == FrpStateName::Starting => (
            FrpStateName::Error,
            Some(format!(
                "{STARTUP_TIMEOUT_SECS} 秒内未连接成功，请检查网络与服务端"
            )),
        ),
        FrpEvent::StartupTimeout => (current, None),
    }
}

/// 单个档案的运行条目（内存态；Child 由监控任务独占，这里只留控制句柄）
pub struct FrpRun {
    /// 当前状态
    pub state: FrpStateName,
    /// frpc 子进程 pid
    pub pid: Option<u32>,
    /// 启动时刻（毫秒时间戳）
    pub started_at: Option<i64>,
    /// 最近一行日志（已脱敏）
    pub last_line: Option<String>,
    /// 最近一次失败原因（已脱敏）
    pub last_error: Option<String>,
    /// 退出码（自然退出时记录）
    pub exit_code: Option<i32>,
    /// 停止信号发送端（发送即请求监控任务结束子进程）
    stop_tx: Option<oneshot::Sender<()>>,
    /// 监控任务句柄（停止时 await 它，确保状态收尾完成）
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl FrpRun {
    /// 更新日志状态；连接恢复后清除旧错误，历史详情仍由日志保留。
    fn record_line(&mut self, line: String) {
        let (next, error) = next_state(self.state, &FrpEvent::Line(&line));
        self.state = next;
        if next == FrpStateName::Running {
            self.last_error = None;
        } else if let Some(error) = error {
            self.last_error = Some(error);
        }
        self.last_line = Some(line);
    }

    /// 新建条目（启动瞬间状态为 starting）
    fn new(pid: Option<u32>) -> Self {
        Self {
            state: FrpStateName::Starting,
            pid,
            started_at: Some(chrono::Utc::now().timestamp_millis()),
            last_line: None,
            last_error: None,
            exit_code: None,
            stop_tx: None,
            handle: None,
        }
    }

    /// 转成契约快照（前端展示用）
    fn snapshot(&self, file_name: &str) -> FrpRuntimeState {
        FrpRuntimeState {
            file_name: file_name.to_string(),
            state: self.state,
            pid: self.pid,
            started_at: self.started_at,
            last_line: self.last_line.clone(),
            last_error: self.last_error.clone(),
            exit_code: self.exit_code,
        }
    }
}

/// 运行态总表（Tauri 托管；键 = 档案文件名）
#[derive(Default)]
pub struct FrpState(pub AsyncMutex<HashMap<String, FrpRun>>);

/// 未运行档案的默认快照（列表与状态查询的兜底值）
pub fn stopped_state(file_name: &str) -> FrpRuntimeState {
    FrpRuntimeState {
        file_name: file_name.to_string(),
        state: FrpStateName::Stopped,
        pid: None,
        started_at: None,
        last_line: None,
        last_error: None,
        exit_code: None,
    }
}

/// 更新条目并在字段有变化时推送 `frp://state`；条目不存在时静默跳过
async fn update<F>(app: &AppHandle, file_name: &str, mutate: F)
where
    F: FnOnce(&mut FrpRun),
{
    let frp_state = app.state::<FrpState>();
    let mut map = frp_state.0.lock().await;
    let Some(run) = map.get_mut(file_name) else {
        return;
    };
    let before = (
        run.state,
        run.pid,
        run.last_line.clone(),
        run.last_error.clone(),
        run.exit_code,
    );
    mutate(run);
    let changed = before
        != (
            run.state,
            run.pid,
            run.last_line.clone(),
            run.last_error.clone(),
            run.exit_code,
        );
    let snapshot = changed.then(|| run.snapshot(file_name));
    drop(map);
    if let Some(snapshot) = snapshot {
        // 状态错误不一定伴随子进程输出（异常退出、启动超时），后台也必须保留原因。
        // 仅状态/错误发生变化时打印，逐行日志更新及状态轮询不重复刷屏。
        if snapshot.state == FrpStateName::Error
            && (before.0 != snapshot.state || before.3 != snapshot.last_error)
        {
            log::error!("FRP 运行状态异常 pid={:?}", snapshot.pid);
        } else if before.0 != snapshot.state && snapshot.state == FrpStateName::Running {
            log::info!("已连接服务端");
        } else if before.1.is_some()
            && snapshot.pid.is_none()
            && snapshot.state == FrpStateName::Stopped
        {
            log::info!("进程已停止，退出码 {:?}", snapshot.exit_code);
        }
        let _ = app.emit("frp://state", snapshot);
    }
}

/// 按行读取一个流：每行推送 `frp://log` 并喂给状态机（流关闭即结束）
fn spawn_reader<R>(
    app: AppHandle,
    file_name: String,
    reader: R,
    stream: FrpLogStream,
    config: Arc<super::auth::PreparedConfig>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            let raw = match lines.next_line().await {
                Ok(Some(raw)) => raw,
                Ok(None) => break,
                Err(error) => {
                    log::error!("读取 {stream:?} 输出失败 kind={:?}", error.kind());
                    break;
                }
            };
            let line = config.redact(&raw);
            // 原始行只供业务日志展示，不复制进应用诊断文件。
            let _ = app.emit(
                "frp://log",
                FrpLogPayload {
                    file_name: file_name.clone(),
                    line: line.clone(),
                    ts: chrono::Utc::now().timestamp_millis(),
                    stream,
                },
            );
            update(&app, &file_name, |run| {
                run.record_line(line);
            })
            .await;
        }
    });
}

/// 监控任务：等退出 / 等停止信号 / 等 starting 超时，然后收尾状态与句柄
async fn monitor(
    app: AppHandle,
    file_name: String,
    mut child: Child,
    mut stop_rx: oneshot::Receiver<()>,
    config: Arc<super::auth::PreparedConfig>,
) {
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(
            app.clone(),
            file_name.clone(),
            stdout,
            FrpLogStream::Stdout,
            Arc::clone(&config),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(
            app.clone(),
            file_name.clone(),
            stderr,
            FrpLogStream::Stderr,
            Arc::clone(&config),
        );
    }
    let timeout = tokio::time::sleep(Duration::from_secs(STARTUP_TIMEOUT_SECS));
    tokio::pin!(timeout);
    let mut timeout_armed = true;
    let mut stopped_by_user = false;
    let exit_code = loop {
        tokio::select! {
            // 停止请求：kill 后继续等 wait 回收（Windows 上 kill = TerminateProcess）
            _ = &mut stop_rx, if !stopped_by_user => {
                stopped_by_user = true;
                let _ = child.kill().await;
            }
            // starting 超时：只判一次，判定后失活避免忙轮询
            _ = &mut timeout, if timeout_armed => {
                timeout_armed = false;
                update(&app, &file_name, |run| {
                    let (next, error) = next_state(run.state, &FrpEvent::StartupTimeout);
                    run.state = next;
                    if let Some(error) = error {
                        run.last_error = Some(error);
                    }
                })
                .await;
            }
            result = child.wait() => {
                match result {
                    Ok(status) => break status.code(),
                    Err(error) => {
                        log::error!("等待 frpc 退出失败 kind={:?}", error.kind());
                        break None;
                    }
                }
            }
        }
    };
    update(&app, &file_name, |run| {
        let event = if stopped_by_user {
            FrpEvent::ExitedByStop
        } else {
            FrpEvent::Exited(exit_code)
        };
        let last_line = run.last_line.clone();
        let (next, error) = next_state(run.state, &event);
        run.state = next;
        run.pid = None;
        run.exit_code = if stopped_by_user { None } else { exit_code };
        if let Some(mut error) = error {
            // 失败原因附最后一行日志（已脱敏），便于用户判读
            if let Some(line) = last_line {
                error = format!("{error}；最后一行：{line}");
            }
            run.last_error = Some(error);
        }
        run.stop_tx = None;
        run.handle = None;
    })
    .await;
}

/// 启动档案：重复启动返回当前状态（契约 §3.3，不报错）
pub(crate) async fn start(
    app: &AppHandle,
    state: &State<'_, FrpState>,
    file_name: &str,
) -> Result<FrpRuntimeState, String> {
    profile::validate_file_name(file_name)?;
    {
        let map = state.0.lock().await;
        if let Some(run) = map.get(file_name) {
            if run.handle.is_some() {
                return Ok(run.snapshot(file_name));
            }
        }
    }
    let dir = super::transfer::directory_for(app, file_name)?;
    let config = profile::require_profile(&dir, file_name).await?;
    // 按档案绑定解析客户端：档案指定 → 默认客户端 → 兜底自动探测（保证清单为空也能跑）
    let exe = clients::resolve(app, file_name).await?;
    let config = Arc::new(super::auth::prepare(app, &config).await?);
    let mut cmd = Command::new(&exe);
    config.configure(&mut cmd);
    cmd.arg("-c")
        .arg(&config.path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 兜底：监控任务未能收尾时，Child 被丢弃也会终止子进程，防残留
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let child = cmd
        .spawn()
        .map_err(|e| format!("启动 frpc 失败：{e}（路径 {}）", exe.display()))?;
    let pid = child.id();
    let (stop_tx, stop_rx) = oneshot::channel();
    // 同一临界区完成：建条目 → 起快照 → spawn 监控（spawn 同步，不引入等待）
    let snapshot = {
        let mut map = state.0.lock().await;
        let mut run = FrpRun::new(pid);
        run.stop_tx = Some(stop_tx);
        let snapshot = run.snapshot(file_name);
        map.insert(file_name.to_string(), run);
        let handle = tokio::spawn(monitor(
            app.clone(),
            file_name.to_string(),
            child,
            stop_rx,
            config,
        ));
        if let Some(run) = map.get_mut(file_name) {
            run.handle = Some(handle);
        }
        snapshot
    };
    // starting 立即外推：前端状态点变黄
    log::info!("frpc 进程已创建，PID {pid:?}，等待连接服务端");
    let _ = app.emit("frp://state", snapshot.clone());
    Ok(snapshot)
}

/// 停止档案：发停止信号 → 等监控任务收尾；未运行时直接返回 stopped
pub(crate) async fn stop(
    _app: &AppHandle,
    state: &State<'_, FrpState>,
    file_name: &str,
) -> Result<FrpRuntimeState, String> {
    profile::validate_file_name(file_name)?;
    let (sender, handle, existed) = {
        let mut map = state.0.lock().await;
        match map.get_mut(file_name) {
            Some(run) => (run.stop_tx.take(), run.handle.take(), true),
            None => (None, None, false),
        }
    };
    if !existed {
        return Ok(stopped_state(file_name));
    }
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    if let Some(handle) = handle {
        // 有界等待：监控任务收尾失败也不阻塞命令返回
        let _ = tokio::time::timeout(Duration::from_secs(STOP_WAIT_SECS), handle).await;
    }
    Ok(state_of(state, file_name).await)
}

/// 重启 = 停 + 启（未运行时等价于启动）
pub(crate) async fn restart(
    app: &AppHandle,
    state: &State<'_, FrpState>,
    file_name: &str,
) -> Result<FrpRuntimeState, String> {
    stop(app, state, file_name).await?;
    start(app, state, file_name).await
}

/// 单档当前状态（列表拼装用；内存中不存在视为 stopped）
pub(crate) async fn state_of(state: &State<'_, FrpState>, file_name: &str) -> FrpRuntimeState {
    let map = state.0.lock().await;
    map.get(file_name)
        .map(|run| run.snapshot(file_name))
        .unwrap_or_else(|| stopped_state(file_name))
}

/// 全部档案状态：配置目录内所有档 + 内存中在跑的档（前端 5s 轮询兜底）
pub(crate) async fn status_all(
    app: &AppHandle,
    state: &State<'_, FrpState>,
) -> Vec<FrpRuntimeState> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(dir) = crate::plugins::frp::profile_dir(app) {
        if let Ok(files) = profile::list_profile_files(&dir).await {
            for path in files {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    {
        let map = state.0.lock().await;
        for name in map.keys() {
            if !names.iter().any(|exist| exist == name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    let mut result = Vec::with_capacity(names.len());
    for name in names {
        result.push(state_of(state, &name).await);
    }
    result
}

/// 应用退出兜底：向全部档案发停止信号并等收尾（best-effort，防残留 frpc）
pub(crate) async fn shutdown_all(app: &AppHandle) {
    let (senders, handles) = {
        let frp_state = app.state::<FrpState>();
        let mut map = frp_state.0.lock().await;
        let mut senders = Vec::new();
        let mut handles = Vec::new();
        for run in map.values_mut() {
            if let Some(sender) = run.stop_tx.take() {
                senders.push(sender);
            }
            if let Some(handle) = run.handle.take() {
                handles.push(handle);
            }
        }
        (senders, handles)
    };
    for sender in senders {
        let _ = sender.send(());
    }
    for handle in handles {
        let _ = tokio::time::timeout(Duration::from_secs(STOP_WAIT_SECS), handle).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 普通日志保留失败原因，明确成功后才清除，后续失败仍能重新报告。
    #[test]
    fn recovered_connection_clears_previous_error() {
        let mut run = FrpRun::new(Some(1));
        run.record_line("login to server failed: EOF".to_string());
        run.record_line("retrying connection".to_string());
        assert_eq!(run.state, FrpStateName::Error);
        assert!(run.last_error.is_some());
        run.record_line("login to server success".to_string());
        assert_eq!(run.state, FrpStateName::Running);
        assert!(run.last_error.is_none());
        run.record_line("start error: port already used".to_string());
        assert_eq!(run.state, FrpStateName::Error);
        assert!(run.last_error.is_some());
    }

    /// 迁移表逐行覆盖：成功行 / 失败行 / 退出码 / 超时 / 未知行（任务书 §5.2、§7.1）
    #[test]
    fn state_machine_transition_table() {
        // spawn：stopped 或 error → starting
        assert_eq!(
            next_state(FrpStateName::Stopped, &FrpEvent::Spawned).0,
            FrpStateName::Starting
        );
        assert_eq!(
            next_state(FrpStateName::Error, &FrpEvent::Spawned).0,
            FrpStateName::Starting
        );
        // 成功行：starting → running
        for line in [
            "2026/09/12 [I] login to server success, get run id [abc]",
            "[I] start proxy success",
        ] {
            assert_eq!(
                next_state(FrpStateName::Starting, &FrpEvent::Line(line)).0,
                FrpStateName::Running
            );
        }
        // 失败行：任意状态 → error，lastError 为该行原文（已脱敏）
        for line in [
            "login to server failed: dial tcp: i/o timeout",
            "token is incorrect",
            "authentication failed",
            "port already used",
            "bind: address already in use",
            "start error: proxy [a] already exists",
        ] {
            let (state, error) = next_state(FrpStateName::Running, &FrpEvent::Line(line));
            assert_eq!(state, FrpStateName::Error, "{line}");
            assert_eq!(error.as_deref(), Some(line), "lastError 应为该行原文");
        }
        // 未知行不改状态
        let (state, error) = next_state(
            FrpStateName::Running,
            &FrpEvent::Line("[I] start to handle connection"),
        );
        assert_eq!(state, FrpStateName::Running);
        assert!(error.is_none());
        // 退出码 0 / 非 0 / 无退出码
        assert_eq!(
            next_state(FrpStateName::Running, &FrpEvent::Exited(Some(0))).0,
            FrpStateName::Stopped
        );
        let (state, error) = next_state(FrpStateName::Running, &FrpEvent::Exited(Some(1)));
        assert_eq!(state, FrpStateName::Error);
        assert!(error.unwrap_or_default().contains("退出码 1"));
        assert_eq!(
            next_state(FrpStateName::Running, &FrpEvent::Exited(None)).0,
            FrpStateName::Error
        );
        // 本工具发起停止 → stopped（不误报 error）
        assert_eq!(
            next_state(FrpStateName::Running, &FrpEvent::ExitedByStop).0,
            FrpStateName::Stopped
        );
        // starting 超时 → error；running 时迟到超时无效
        let (state, error) = next_state(FrpStateName::Starting, &FrpEvent::StartupTimeout);
        assert_eq!(state, FrpStateName::Error);
        assert!(error.unwrap_or_default().contains("30 秒"));
        assert_eq!(
            next_state(FrpStateName::Running, &FrpEvent::StartupTimeout).0,
            FrpStateName::Running
        );
    }
}
