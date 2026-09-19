//! 本机端口查询工具：Windows 原生端点表，其他平台明确不支持。

mod models;
#[cfg(windows)]
mod windows;

use models::{PortEndpoint, PortSnapshot};

/// 当前平台是否支持本机端口查询。
#[tauri::command]
pub fn port_viewer_supported() -> bool {
    cfg!(windows)
}

/// 读取 TCP/UDP 的 IPv4/IPv6 快照，系统调用在阻塞线程执行。
#[tauri::command]
pub async fn port_viewer_query() -> Result<PortSnapshot, String> {
    #[cfg(windows)]
    {
        let permit = windows::OperationPermit::acquire()?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            windows::query()
        })
        .await
        .map_err(|e| format!("端口查询任务失败：{e}"))?
    }
    #[cfg(not(windows))]
    {
        Err("端口占用查询目前仅支持 Windows".into())
    }
}

/// 关闭前重新读取指定协议与地址族的端点，核对资源归属及进程身份。
#[tauri::command(rename_all = "camelCase")]
pub async fn port_viewer_terminate(
    endpoint: PortEndpoint,
    started_at: String,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let permit = windows::OperationPermit::acquire()?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            windows::terminate(&endpoint, &started_at)
        })
        .await
        .map_err(|e| format!("关闭端口使用进程失败：{e}"))?
    }
    #[cfg(not(windows))]
    {
        let _ = (endpoint, started_at);
        Err("关闭端口使用进程目前仅支持 Windows".into())
    }
}

crate::covekit_module! {
    owner: "port_viewer",
    feature: "port-viewer",
    commands: {
        port_viewer_supported => "查询端口工具的平台支持状态",
        port_viewer_query => "读取本机 TCP 和 UDP 端口及进程快照",
        port_viewer_terminate => "核对端口归属与进程身份后关闭指定进程",
    },
}

/// 注册命令；无持久化、常驻线程或订阅，临时资源随每次调用释放。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    builder
}
