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
use tokio::sync::{oneshot, watch, Mutex as AsyncMutex};
use tokio::task::JoinSet;

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
    /// 监控任务句柄（一直归运行条目所有，停止请求通过完成信号等待收尾）
    handle: Option<tokio::task::JoinHandle<()>>,
    /// 停止等待者只订阅完成信号，不提前取走监控任务的所有权。
    completion: watch::Sender<bool>,
}

/// 监控退出、取消或 panic 均唤醒停止等待者。
struct RunCompletion(watch::Sender<bool>);

impl Drop for RunCompletion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}

impl Drop for FrpRun {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
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
            completion: watch::channel(false).0,
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

/// 日志与其运行状态合并交付；其它状态变化仍推送 `frp://state`。
async fn update<F>(app: &AppHandle, file_name: &str, log_stream: Option<FrpLogStream>, mutate: F)
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
        if log_stream.is_none() { run.last_line.clone() } else { None },
        run.last_error.clone(),
        run.exit_code,
    );
    mutate(run);
    let changed = before.0 != run.state
        || before.1 != run.pid
        || before.2.as_ref() != run.last_line.as_ref()
        || before.3.as_ref() != run.last_error.as_ref()
        || before.4 != run.exit_code;
    let snapshot = (changed || log_stream.is_some()).then(|| run.snapshot(file_name));
    drop(map);
    if let Some(mut snapshot) = snapshot {
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
        if let Some(stream) = log_stream {
            // record_line 在同一临界区设置 last_line；空字符串仍是合法日志行。
            if let Some(line) = snapshot.last_line.take() {
                let payload = log_payload(snapshot, line, stream);
                if app.emit("frp://log", payload).is_err() {
                    log::warn!("FRP 日志事件发送失败");
                }
            } else {
                log::error!("FRP 日志事件缺少对应正文");
            }
        } else {
            let _ = app.emit("frp://state", snapshot);
        }
    }
}

fn log_payload(mut state: FrpRuntimeState, line: String, stream: FrpLogStream) -> FrpLogPayload {
    state.last_line = None;
    let last_error_from_line = state.last_error.as_deref() == Some(line.as_str());
    if last_error_from_line {
        state.last_error = None;
    }
    FrpLogPayload {
        file_name: state.file_name.clone(),
        line,
        ts: chrono::Utc::now().timestamp_millis(),
        stream,
        state,
        last_error_from_line,
    }
}

/// 按行读取一个流：每行推送 `frp://log` 并喂给状态机（流关闭即结束）
fn spawn_reader<R>(
    tasks: &mut JoinSet<()>,
    app: AppHandle,
    file_name: String,
    reader: R,
    stream: FrpLogStream,
    config: Arc<super::auth::PreparedConfig>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tasks.spawn(async move {
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
            // 日志与状态一次交付，正文不复制进应用诊断文件。
            update(&app, &file_name, Some(stream), |run| {
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
    _completion: RunCompletion,
) {
    // 监控任务取消时同时取消读流；正常退出先读完管道尾部，再发布最终状态。
    let mut readers = JoinSet::new();
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(
            &mut readers,
            app.clone(),
            file_name.clone(),
            stdout,
            FrpLogStream::Stdout,
            Arc::clone(&config),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(
            &mut readers,
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
                update(&app, &file_name, None, |run| {
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
    while let Some(result) = readers.join_next().await {
        if let Err(error) = result {
            log::error!("FRP 日志读取任务异常结束：{error}");
        }
    }
    update(&app, &file_name, None, |run| {
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
            if run.handle.as_ref().is_some_and(|handle| !handle.is_finished()) {
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
    // 同一临界区完成：建条目 → 起快照 → spawn 监控（spawn 同步，不引入等待）
    let snapshot = {
        let mut map = state.0.lock().await;
        // 配置准备有 await，必须在真正 spawn 前复核，避免两个启动各建一个进程。
        if let Some(run) = map.get(file_name) {
            if run.handle.as_ref().is_some_and(|handle| !handle.is_finished()) {
                return Ok(run.snapshot(file_name));
            }
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("启动 frpc 失败：{e}（路径 {}）", exe.display()))?;
        let pid = child.id();
        let (stop_tx, stop_rx) = oneshot::channel();
        let mut run = FrpRun::new(pid);
        run.stop_tx = Some(stop_tx);
        let completion = RunCompletion(run.completion.clone());
        let snapshot = run.snapshot(file_name);
        map.insert(file_name.to_string(), run);
        let handle = tokio::spawn(monitor(
            app.clone(),
            file_name.to_string(),
            child,
            stop_rx,
            config,
            completion,
        ));
        if let Some(run) = map.get_mut(file_name) {
            run.handle = Some(handle);
        }
        snapshot
    };
    // starting 立即外推：前端状态点变黄
    log::info!("frpc 进程已创建，PID {:?}，等待连接服务端", snapshot.pid);
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
    let (sender, completion, existed) = {
        let mut map = state.0.lock().await;
        match map.get_mut(file_name) {
            Some(run) => (
                run.stop_tx.take(),
                run.handle.as_ref().map(|_| run.completion.subscribe()),
                true,
            ),
            None => (None, None, false),
        }
    };
    if !existed {
        return Ok(stopped_state(file_name));
    }
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    if let Some(mut completion) = completion {
        tokio::time::timeout(
            Duration::from_secs(STOP_WAIT_SECS),
            completion.wait_for(|finished| *finished),
        )
        .await
        .map_err(|_| "frpc 停止仍在收尾，请稍后重试".to_string())?
        .map_err(|_| "frpc 停止状态通知已中断".to_string())?;
    }
    let snapshot = state_of(state, file_name).await;
    if snapshot.pid.is_some() {
        return Err("frpc 监控已结束但未完成状态收尾，请重新检查运行状态".to_string());
    }
    Ok(snapshot)
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
pub(crate) async fn shutdown_all(app: &AppHandle) -> Vec<String> {
    let (senders, completions) = {
        let frp_state = app.state::<FrpState>();
        let mut map = frp_state.0.lock().await;
        let mut senders = Vec::new();
        let mut completions = Vec::new();
        for (file_name, run) in map.iter_mut() {
            if let Some(sender) = run.stop_tx.take() {
                senders.push(sender);
            }
            if run.handle.is_some() {
                completions.push((file_name.clone(), run.completion.subscribe()));
            }
        }
        (senders, completions)
    };
    for sender in senders {
        let _ = sender.send(());
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(STOP_WAIT_SECS);
    let mut issues = Vec::new();
    for (file_name, mut completion) in completions {
        if !matches!(
            tokio::time::timeout_at(deadline, completion.wait_for(|finished| *finished)).await,
            Ok(Ok(_))
        ) {
            log::error!("关闭 FRP 工具时进程尚未完成停止收尾");
            issues.push(format!("FRP 档案 {file_name} 尚未完成停止收尾"));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_event_carries_body_once_and_preserves_state_and_error() {
        let mut run = FrpRun::new(Some(7));
        let line = "start error: ".to_string() + &"x".repeat(70000);
        run.record_line(line.clone());
        let payload = log_payload(run.snapshot("a.toml"), line.clone(), FrpLogStream::Stderr);
        let encoded = serde_json::to_value(&payload).unwrap();
        assert_eq!(encoded["line"], line);
        assert_eq!(encoded["state"]["state"], "error");
        assert_eq!(encoded["state"]["pid"], 7);
        assert!(encoded["state"].get("lastLine").is_none());
        assert!(encoded["state"].get("lastError").is_none());
        assert!(payload.last_error_from_line);
        assert_eq!(run.last_line.as_deref(), Some(line.as_str()));
        run.record_line("login to server success".into());
        let recovered = log_payload(run.snapshot("a.toml"), "login to server success".into(), FrpLogStream::Stdout);
        assert_eq!(recovered.state.state, FrpStateName::Running);
        assert!(recovered.state.last_error.is_none());
        assert!(!recovered.last_error_from_line);
    }

    /// 多个停止等待者共享完成状态，某个等待超时不取走任务句柄。
    #[tokio::test]
    async fn stop_waiters_keep_monitor_owned_until_completion() {
        let mut run = FrpRun::new(Some(1));
        let mut first = run.completion.subscribe();
        let mut second = run.completion.subscribe();
        let completion = RunCompletion(run.completion.clone());
        let (release, ready) = oneshot::channel();
        run.handle = Some(tokio::spawn(async move {
            let _completion = completion;
            let _ = ready.await;
        }));
        assert!(tokio::time::timeout(
            Duration::ZERO,
            first.wait_for(|finished| *finished),
        )
        .await
        .is_err());
        assert!(!run.handle.as_ref().unwrap().is_finished());
        release.send(()).unwrap();
        assert!(*second.wait_for(|finished| *finished).await.unwrap());
        assert!(*first.wait_for(|finished| *finished).await.unwrap());
        run.handle.take().unwrap().await.unwrap();
    }

    /// 运行条目释放会取消监控，监控拥有的读流任务也随之释放。
    #[tokio::test]
    async fn dropping_run_cancels_monitor_and_owned_readers() {
        let mut run = FrpRun::new(Some(1));
        let mut finished = run.completion.subscribe();
        let completion = RunCompletion(run.completion.clone());
        let (started_tx, started_rx) = oneshot::channel();
        let (reader_done, mut reader_finished) = watch::channel(false);
        run.handle = Some(tokio::spawn(async move {
            let _completion = completion;
            let mut readers = JoinSet::new();
            let reader_completion = RunCompletion(reader_done);
            readers.spawn(async move {
                let _completion = reader_completion;
                std::future::pending::<()>().await;
            });
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        }));
        started_rx.await.unwrap();
        drop(run);
        tokio::time::timeout(Duration::from_secs(1), async {
            assert!(*finished.wait_for(|value| *value).await.unwrap());
            assert!(*reader_finished.wait_for(|value| *value).await.unwrap());
        })
        .await
        .unwrap();
    }

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
