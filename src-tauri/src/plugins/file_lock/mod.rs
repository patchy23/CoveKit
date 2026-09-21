//! Windows 单文件占用查询与指定进程关闭；无持久化，资源归本次命令所有。

mod models;
#[cfg(windows)]
mod windows;

use models::FileLockResult;

/// 返回当前编译平台是否支持文件占用查询，供界面明确展示平台限制。
#[tauri::command]
pub fn file_lock_supported() -> bool {
    cfg!(windows)
}

/// 查询完整文件路径的使用进程；同步系统调用不占用异步执行线程。
#[tauri::command]
pub async fn file_lock_query(path: String) -> Result<FileLockResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<FileLockResult, String> = async {
        #[cfg(windows)]
        {
            // 跨页签重开也只允许一个查询；不在阻塞线程池内排队堆积会话。
            let permit = windows::QueryPermit::acquire()?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                windows::query(&path)
            })
            .await
            .map_err(|e| format!("文件占用查询任务失败：{e}"))?
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Err("文件占用查询目前仅支持 Windows".into())
        }
    }
    .await;
    match &result {
        Ok(_value) => log::debug!(
            "操作完成 operation=file_lock_query elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) if !cfg!(windows) => {}
        Err(_) => log::warn!(
            "操作未完成 operation=file_lock_query elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 关闭用户确认的文件使用进程；后端再次核对文件关系、启动时间和系统关键进程状态。
#[tauri::command(rename_all = "camelCase")]
pub async fn file_lock_terminate(path: String, pid: u32, started_at: String) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = async {
        #[cfg(windows)]
        {
            let permit = windows::QueryPermit::acquire()?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                windows::terminate(&path, pid, &started_at)
            })
            .await
            .map_err(|e| format!("关闭进程任务失败：{e}"))?
        }
        #[cfg(not(windows))]
        {
            let _ = (path, pid, started_at);
            Err("关闭文件占用进程目前仅支持 Windows".into())
        }
    }
    .await;
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=file_lock_terminate elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) if !cfg!(windows) => {}
        Err(_) => log::warn!(
            "操作未完成 operation=file_lock_terminate elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

crate::covekit_module! {
    owner: "file_lock",
    feature: "file-lock",
    commands: {
        file_lock_supported => "查询文件占用工具的平台支持状态",
        file_lock_query => "查询指定文件的使用进程信息",
        file_lock_terminate => "核对文件使用关系与进程身份后关闭指定进程",
    },
}

/// 注册查询与关闭进程命令；无常驻资源或退出清理职责。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    builder
}
