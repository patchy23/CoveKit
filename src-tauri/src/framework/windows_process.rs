//! Windows 进程公开能力：查询身份与程序路径、在资源 owner 校验后关闭进程。

use std::path::Path;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, IsProcessCritical, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

const ZERO_TIME: FILETIME = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
};

/// 查询成功的进程身份与路径；调用者自行映射本 owner 的传输模型。
#[derive(Clone, Debug)]
pub(crate) struct ProcessInfo {
    /// Windows FILETIME，单位为自 1601 年起的 100 纳秒。
    pub(crate) started_at: u64,
    pub(crate) executable_path: String,
    pub(crate) process_name: String,
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: 非空 OpenProcess 句柄由本对象独占，所有同步操作结束后仅关闭一次。
        if unsafe { CloseHandle(self.0) } == 0 {
            log::warn!("关闭进程查询句柄失败：{}", std::io::Error::last_os_error());
        }
    }
}

/// 合并 FILETIME，不发生截断或有符号转换。
pub(crate) fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

/// 扫描开始时间，用于排除枚举之后才新生的同 PID 进程。
pub(crate) fn snapshot_time() -> u64 {
    let mut time = ZERO_TIME;
    // SAFETY: 输出指向可写 FILETIME，系统调用在返回前完成写入。
    unsafe { GetSystemTimeAsFileTime(&mut time) };
    filetime(time)
}

fn creation_time(handle: &ProcessHandle) -> Result<u64, String> {
    let (mut created, mut exited, mut kernel, mut user) =
        (ZERO_TIME, ZERO_TIME, ZERO_TIME, ZERO_TIME);
    // SAFETY: 有效查询句柄，四个独立 FILETIME 输出在调用期间均可写且不别名。
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(format!(
            "无法核对进程身份：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(filetime(created))
}

fn verify_identity(handle: &ProcessHandle, expected: u64) -> Result<(), String> {
    if creation_time(handle)? != expected {
        return Err("原进程已退出，PID 已被复用；请刷新查询".into());
    }
    Ok(())
}

/// 获取启动时间与程序路径；有已有身份时必须匹配，拒绝附上复用 PID 的新进程详情。
pub(crate) fn inspect(pid: u32, expected_start: Option<u64>) -> Result<ProcessInfo, String> {
    // SAFETY: 只请求查询权限且不继承句柄；空句柄不交给资源守卫。
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err(format!(
            "无法读取进程详情，可能已退出或权限不足：{}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = ProcessHandle(raw);
    let started_at = creation_time(&handle)?;
    if expected_start.is_some_and(|expected| expected != started_at) {
        return Err("原进程已退出，PID 已被复用；请刷新查询".into());
    }
    let mut buffer = vec![0u16; 32768];
    let mut size = 32768u32;
    // SAFETY: 查询句柄有效，size 与可写 UTF-16 缓冲长度一致；flags=0 请求 Win32 路径。
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut size) } == 0 {
        return Err(format!(
            "无法读取程序路径：{}",
            std::io::Error::last_os_error()
        ));
    }
    let len = usize::try_from(size).map_err(|_| "程序路径长度无效")?;
    let value = buffer.get(..len).ok_or("程序路径长度超出缓冲区")?;
    let executable_path = String::from_utf16(value).map_err(|_| "程序路径包含无法显示的字符")?;
    let process_name = Path::new(&executable_path)
        .file_name()
        .ok_or("程序路径不包含文件名")?
        .to_string_lossy()
        .into_owned();
    Ok(ProcessInfo {
        started_at,
        executable_path,
        process_name,
    })
}

fn validate_target(pid: u32, started_at: &str) -> Result<u64, String> {
    if pid <= 4 || pid == std::process::id() {
        return Err("不允许关闭系统保留进程或 CoveKit 自身".into());
    }
    let started = started_at
        .parse::<u64>()
        .map_err(|_| "进程身份无效，请重新查询")?;
    if started == 0 {
        return Err("缺少进程启动时间，请重新查询".into());
    }
    Ok(started)
}

/// 使用同一句柄验证身份、资源关系并关闭；verify_usage 必须由资源 owner 重新读取系统状态。
pub(crate) fn terminate(
    pid: u32,
    started_at: &str,
    verify_usage: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let started = validate_target(pid, started_at)?;
    // SAFETY: 不继承句柄，后续身份校验和关闭都使用这一句柄，避免再次按 PID 打开。
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if raw.is_null() {
        return Err(format!(
            "无法关闭进程，可能已退出或权限不足：{}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = ProcessHandle(raw);
    verify_identity(&handle, started)?;
    let mut critical = 0;
    // SAFETY: 查询句柄有效，BOOL 输出独立可写；检查失败即拒绝关闭。
    if unsafe { IsProcessCritical(handle.0, &mut critical) } == 0 {
        return Err(format!(
            "无法核对系统关键进程状态，已取消关闭：{}",
            std::io::Error::last_os_error()
        ));
    }
    if critical != 0 {
        return Err("不允许关闭系统关键进程".into());
    }
    verify_usage()?;
    // SAFETY: 同一句柄已核对启动时间、关键状态与 owner 的资源关系；仅终止所确认的进程。
    if unsafe { TerminateProcess(handle.0, 1) } == 0 {
        return Err(format!(
            "关闭进程失败，请刷新后重试：{}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: 句柄持有同步权限且在等待期间保持有效；阻塞线程最多等待五秒。
    match unsafe { WaitForSingleObject(handle.0, 5000) } {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err("已发出关闭请求，但进程尚未退出，请稍后刷新确认".into()),
        _ => Err(format!(
            "已发出关闭请求，但无法确认退出状态：{}",
            std::io::Error::last_os_error()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_reserved_self_and_invalid_identity() {
        for pid in [0, 4, std::process::id()] {
            assert!(validate_target(pid, "123").is_err());
        }
        let other = if std::process::id() == 42 { 43 } else { 42 };
        for started in ["", "0", "-1", "bad", "18446744073709551616"] {
            assert!(validate_target(other, started).is_err());
        }
        assert_eq!(
            validate_target(other, "134029000000000000").unwrap(),
            134029000000000000
        );
    }

    #[test]
    fn rejects_a_reused_process_identity() {
        assert!(inspect(std::process::id(), Some(0))
            .unwrap_err()
            .contains("PID 已被复用"));
    }

    #[test]
    fn reads_current_process_and_snapshot_time() {
        let before = snapshot_time();
        let process = inspect(std::process::id(), None).unwrap();
        assert!(process.started_at <= before);
        assert!(!process.executable_path.is_empty());
        assert!(!process.process_name.is_empty());
    }
}
