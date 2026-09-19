//! Restart Manager 查询及进程信息补全；每次调用独立持有并释放系统资源。

use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_MORE_DATA, ERROR_SUCCESS, FILETIME, HANDLE,
};
use windows_sys::Win32::System::RestartManager::{
    RmEndSession, RmGetList, RmRegisterResources, RmStartSession, RM_PROCESS_INFO,
    RM_UNIQUE_PROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::models::{FileLockResult, FileProcess};

static QUERY_RUNNING: AtomicBool = AtomicBool::new(false);
const MAX_PROCESSES: usize = 4096;
const ZERO_TIME: FILETIME = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
};
const EMPTY_PROCESS: RM_PROCESS_INFO = RM_PROCESS_INFO {
    Process: RM_UNIQUE_PROCESS {
        dwProcessId: 0,
        ProcessStartTime: ZERO_TIME,
    },
    strAppName: [0; 256],
    strServiceShortName: [0; 64],
    ApplicationType: 0,
    AppStatus: 0,
    TSSessionId: 0,
    bRestartable: 0,
};

/// 应用级查询许可；命令退出或失败时自动归还。
pub(super) struct QueryPermit;

impl QueryPermit {
    /// 拒绝并发查询，避免频繁注册系统会话与积压阻塞任务。
    pub(super) fn acquire() -> Result<Self, String> {
        QUERY_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "已有文件占用查询正在进行，请稍后重试".to_string())?;
        Ok(Self)
    }
}

impl Drop for QueryPermit {
    fn drop(&mut self) {
        QUERY_RUNNING.store(false, Ordering::Release);
    }
}

struct Session(Option<u32>);

impl Session {
    fn close(&mut self) -> Result<(), String> {
        if let Some(handle) = self.0.take() {
            // SAFETY: 句柄只来自成功的 RmStartSession，take 保证仅结束一次且没有并行使用者。
            check(unsafe { RmEndSession(handle) }, "结束文件查询会话")?;
        }
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("[file_lock] 资源释放失败：{error}");
        }
    }
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: 非空 OpenProcess 句柄由本对象独占，同步补全结束后仅关闭一次。
        if unsafe { CloseHandle(self.0) } == 0 {
            eprintln!(
                "[file_lock] 关闭进程查询句柄失败：{}",
                std::io::Error::last_os_error()
            );
        }
    }
}

fn check(code: u32, action: &str) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "{action}失败，Windows 错误码 {code}；请稍后重试或检查访问权限"
        ))
    }
}

fn validate_path(path: &str) -> Result<Vec<u16>, String> {
    if path.is_empty() || path.contains('\0') || !Path::new(path).is_absolute() {
        return Err("请选择文件或输入完整的绝对路径".into());
    }
    let mut wide: Vec<u16> = path.encode_utf16().collect();
    if wide.len() >= 32767 {
        return Err("文件路径过长".into());
    }
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("无法读取目标文件信息，请检查文件是否存在及访问权限：{e}"))?;
    if !metadata.is_file() {
        return Err("请选择单个普通文件，暂不支持目录查询".into());
    }
    wide.push(0);
    Ok(wide)
}

/// 查询普通文件；所有返回分支都结束会话，不调用关闭应用或重启接口。
pub(super) fn query(path: &str) -> Result<FileLockResult, String> {
    let wide = validate_path(path)?;
    let mut handle = 0;
    // CCH_RM_SESSION_KEY 为 32，加一个 UTF-16 终止符。
    let mut key = [0u16; 33];
    // SAFETY: 输出句柄和 33 单元的会话键缓冲在同步调用期间有效，flags 按 API 要求为零。
    let status = unsafe { RmStartSession(&mut handle, 0, key.as_mut_ptr()) };
    check(status, "创建文件查询会话")?;
    let mut session = Session(Some(handle));
    let result = (|| {
        let files = [wide.as_ptr()];
        // SAFETY: files 含一个有效的 NUL 结尾 UTF-16 路径；未注册应用/服务，计数和指针均为零。
        let status =
            unsafe { RmRegisterResources(handle, 1, files.as_ptr(), 0, null(), 0, null()) };
        check(status, "登记目标文件")?;
        let entries = read_processes(|buffer, needed, count| {
            let mut reasons = 0;
            let ptr = if buffer.is_empty() {
                null_mut()
            } else {
                buffer.as_mut_ptr()
            };
            // SAFETY: count 等于已初始化切片长度，空切片传 null；所有出参仅在本次同步调用中借用。
            unsafe { RmGetList(handle, needed, count, ptr, &mut reasons) }
        })?;
        let mut processes: Vec<_> = entries.iter().map(describe_process).collect();
        processes.sort_by_key(|process| process.pid);
        Ok(FileLockResult {
            path: path.to_owned(),
            processes,
        })
    })();
    let cleanup = session.close();
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}；{cleanup}")),
    }
}

// 进程列表可在两次 API 调用间增长；重试与内存均有上限，不返回截断的“成功”。
fn read_processes(
    mut fetch: impl FnMut(&mut [RM_PROCESS_INFO], &mut u32, &mut u32) -> u32,
) -> Result<Vec<RM_PROCESS_INFO>, String> {
    let mut buffer = Vec::new();
    for _ in 0..5 {
        let mut count = u32::try_from(buffer.len()).map_err(|_| "进程列表过大")?;
        let mut needed = 0;
        let code = fetch(&mut buffer, &mut needed, &mut count);
        if code == ERROR_SUCCESS {
            let count = usize::try_from(count).map_err(|_| "进程数量无效")?;
            if count > buffer.len() {
                return Err("系统返回的进程数量超出缓冲区".into());
            }
            buffer.truncate(count);
            return Ok(buffer);
        }
        if code != ERROR_MORE_DATA {
            check(code, "读取文件使用进程")?;
        }
        let size = usize::try_from(needed).map_err(|_| "进程数量无效")?;
        if size == 0 || size > MAX_PROCESSES {
            return Err("文件使用进程数量异常或过多，请稍后重试".into());
        }
        buffer.resize(size, EMPTY_PROCESS);
    }
    Err("文件使用进程变化过于频繁，请稍后重新查询".into())
}

fn wide_string(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
}

fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn executable_path(process: &RM_UNIQUE_PROCESS) -> Result<String, String> {
    // SAFETY: 仅请求查询权限、不继承句柄；失败的空句柄不构造资源守卫。
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process.dwProcessId) };
    if raw.is_null() {
        return Err(format!(
            "无法读取进程详情，可能已退出或权限不足：{}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = ProcessHandle(raw);
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
    if filetime(created) != filetime(process.ProcessStartTime) {
        return Err("原进程已退出，PID 已被复用；请刷新查询".into());
    }
    let mut buffer = vec![0u16; 32768];
    let mut size = 32768u32;
    // SAFETY: 有效查询句柄；size 等于可写 UTF-16 缓冲区长度，flags=0 请求 Win32 路径。
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut size) } == 0 {
        return Err(format!(
            "无法读取程序路径：{}",
            std::io::Error::last_os_error()
        ));
    }
    let len = usize::try_from(size).map_err(|_| "程序路径长度无效")?;
    let path = buffer.get(..len).ok_or("程序路径长度超出缓冲区")?;
    String::from_utf16(path).map_err(|_| "程序路径包含无法显示的字符".into())
}

fn describe_process(info: &RM_PROCESS_INFO) -> FileProcess {
    let (path, error) = match executable_path(&info.Process) {
        Ok(path) => (Some(path), None),
        Err(error) => (None, Some(error)),
    };
    let name = path
        .as_ref()
        .and_then(|path| Path::new(path).file_name())
        .map(|name| name.to_string_lossy().into_owned());
    let service = wide_string(&info.strServiceShortName);
    FileProcess {
        pid: info.Process.dwProcessId,
        started_at: filetime(info.Process.ProcessStartTime).to_string(),
        app_name: wide_string(&info.strAppName),
        service_name: if service.is_empty() {
            None
        } else {
            Some(service)
        },
        executable_path: path,
        process_name: name,
        detail_error: error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_nul_missing_and_directory_paths() {
        assert!(validate_path("relative.txt").is_err());
        assert!(validate_path("C:\\a\0b").is_err());
        assert!(validate_path(&std::env::temp_dir().to_string_lossy()).is_err());
        let missing = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        assert!(validate_path(&missing.to_string_lossy()).is_err());
    }

    #[test]
    fn retries_a_growing_list_and_uses_actual_count() {
        let mut calls = 0;
        let result = read_processes(|buffer, needed, count| {
            calls += 1;
            match calls {
                1 => {
                    *needed = 1;
                    ERROR_MORE_DATA
                }
                2 => {
                    *needed = 3;
                    ERROR_MORE_DATA
                }
                _ => {
                    buffer[0].Process.dwProcessId = 42;
                    *count = 1;
                    ERROR_SUCCESS
                }
            }
        })
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].Process.dwProcessId, 42);
        assert_eq!(calls, 3);
    }

    #[test]
    fn distinguishes_empty_from_failure_and_bounds_retries() {
        assert!(read_processes(|_, _, count| {
            *count = 0;
            ERROR_SUCCESS
        })
        .unwrap()
        .is_empty());
        assert!(read_processes(|_, _, _| 5).is_err_and(|error| error.contains("错误码 5")));
        assert!(read_processes(|_, needed, _| {
            *needed = 4097;
            ERROR_MORE_DATA
        })
        .is_err());
        let mut calls = 0;
        assert!(read_processes(|_, needed, _| {
            calls += 1;
            *needed = calls;
            ERROR_MORE_DATA
        })
        .is_err());
        assert_eq!(calls, 5);
        assert!(read_processes(|_, _, count| {
            *count = 1;
            ERROR_SUCCESS
        })
        .is_err());
    }

    #[test]
    fn query_permit_is_released_on_drop() {
        let permit = QueryPermit::acquire().unwrap();
        assert!(QueryPermit::acquire().is_err());
        drop(permit);
        assert!(QueryPermit::acquire().is_ok());
    }

    #[test]
    fn does_not_attach_details_to_a_different_process_identity() {
        let identity = RM_UNIQUE_PROCESS {
            dwProcessId: std::process::id(),
            // 当前进程不可能从 Windows 纪元起点开始运行，用它模拟 PID 被复用。
            ProcessStartTime: ZERO_TIME,
        };
        assert!(executable_path(&identity)
            .unwrap_err()
            .contains("PID 已被复用"));
    }

    // Windows CI 使用隔离的临时文件验证原生调用链，不接触用户文件或关闭任何进程。
    #[test]
    fn finds_a_locked_file_and_refreshes_after_release() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

        let path = std::env::temp_dir().join(format!("covekit 占用 {}.txt", uuid::Uuid::new_v4()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .unwrap();
        let occupied = query(path.to_str().unwrap());
        drop(file);
        let released = query(path.to_str().unwrap());
        // 先清理隔离文件，再对结果断言，查询失败也不遗留夹具。
        std::fs::remove_file(&path).unwrap();
        let occupied = occupied.unwrap();
        let current = occupied
            .processes
            .iter()
            .find(|process| process.pid == std::process::id())
            .expect("应找到持有文件的本测试进程");
        assert!(
            current.executable_path.is_some(),
            "{:?}",
            current.detail_error
        );
        assert!(!released
            .unwrap()
            .processes
            .iter()
            .any(|process| process.pid == std::process::id()));
    }
}
