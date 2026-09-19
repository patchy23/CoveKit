//! Windows 单文件占用查询；只读、无持久化，资源归本次命令所有。

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

crate::covekit_module! {
    owner: "file_lock",
    feature: "file-lock",
    commands: {
        file_lock_supported => "查询文件占用工具的平台支持状态",
        file_lock_query => "查询指定文件的使用进程信息",
    },
}

/// 注册只读查询命令；无常驻资源或退出清理职责。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    builder
}
