//! Windows 原生进程树采样；句柄限于同步查询并由 RAII 关闭。

use super::{descendants, ProcessSample, ResourceSnapshot};
use crate::framework::windows_process::{filetime, snapshot_time};
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX, PROCESS_MEMORY_COUNTERS_EX2,
};
use windows_sys::Win32::System::Threading::{
    GetActiveProcessorCount, GetProcessHandleCount, GetProcessTimes, OpenProcess,
    ALL_PROCESSOR_GROUPS, PROCESS_QUERY_LIMITED_INFORMATION,
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: 仅持有有效的独占快照或进程句柄，不跨 await，析构恰好关闭一次。
        if unsafe { CloseHandle(self.0) } == 0 {
            log::warn!(
                "资源采样查询句柄关闭失败：{}",
                std::io::Error::last_os_error()
            );
        }
    }
}

fn read_process(
    pid: u32,
    threads: u32,
    kind: &'static str,
) -> Result<(u64, ProcessSample), String> {
    // SAFETY: 只申请查询权限，不继承句柄；返回值检查后交给 RAII 管理。
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err(format!(
            "进程资源暂不可读：{}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = Handle(raw);
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut start, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: 有效查询句柄和四个独立可写的 FILETIME，输出只在同步调用期间使用。
    if unsafe { GetProcessTimes(handle.0, &mut start, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(format!(
            "进程 CPU 时间读取失败：{}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: 此 C 结构仅含整数，零初始化合法；系统调用前填入实际结构大小。
    let mut memory: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    memory.cb = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>())
        .map_err(|_| "内存计数结构大小超出范围")?;
    // SAFETY: EX 与基础结构前缀 ABI 兼容，传入完整 EX 大小与可写缓冲区。
    if unsafe {
        K32GetProcessMemoryInfo(
            handle.0,
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            memory.cb,
        )
    } == 0
    {
        return Err(format!(
            "进程内存读取失败：{}",
            std::io::Error::last_os_error()
        ));
    }
    // EX2 仅在较新系统可用；保留哨兵，避免旧系统只写基础前缀时误报零。
    // SAFETY: EX2 仅含整数；零初始化合法，系统调用前设置结构大小。
    let mut extended: PROCESS_MEMORY_COUNTERS_EX2 = unsafe { std::mem::zeroed() };
    extended.cb = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>())
        .map_err(|_| "扩展内存计数结构大小超出范围")?;
    extended.PrivateWorkingSetSize = usize::MAX;
    // SAFETY: EX2 与基础结构前缀 ABI 兼容，缓冲区大小与传入大小一致。
    let private_resident_bytes = if unsafe {
        K32GetProcessMemoryInfo(
            handle.0,
            (&mut extended as *mut PROCESS_MEMORY_COUNTERS_EX2).cast(),
            extended.cb,
        )
    } != 0
        && extended.PrivateWorkingSetSize != usize::MAX
    {
        Some(u64::try_from(extended.PrivateWorkingSetSize).map_err(|_| "私有工作集计数超出范围")?)
    } else {
        None
    };
    let mut handles = 0;
    // SAFETY: 有效查询句柄与可写 DWORD；读取失败仅该可选指标为空。
    let handle_count = if unsafe { GetProcessHandleCount(handle.0, &mut handles) } != 0 {
        Some(handles)
    } else {
        None
    };
    let started = filetime(start);
    // u64 转浮点只用于秒数统计，允许亚微秒精度损失，不进行整数回转。
    let cpu_seconds = (filetime(kernel) as f64 + filetime(user) as f64) / 10_000_000.0;
    Ok((
        started,
        ProcessSample {
            pid,
            identity: started.to_string(),
            kind,
            private_resident_bytes,
            resident_bytes: u64::try_from(memory.WorkingSetSize)
                .map_err(|_| "工作集计数超出范围")?,
            private_bytes: Some(
                u64::try_from(memory.PrivateUsage).map_err(|_| "私有提交计数超出范围")?,
            ),
            cpu_seconds,
            threads: Some(threads),
            handles: handle_count,
        },
    ))
}

/// 读取主进程及当前可验证的后代；退出竞态标为覆盖不完整。
pub(super) fn sample() -> Result<ResourceSnapshot, String> {
    let captured_at = snapshot_time();
    // SAFETY: 创建只读进程快照，无借用指针；有效返回值由 RAII 释放。
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(format!(
            "无法读取进程列表：{}",
            std::io::Error::last_os_error()
        ));
    }
    let snapshot = Handle(raw);
    // SAFETY: PROCESSENTRY32W 是整数与固定字符数组组成的 C 结构，零初始化合法。
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize =
        u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).map_err(|_| "进程结构过大")?;
    let mut entries = Vec::new();
    // SAFETY: 快照有效，entry 大小已初始化且在调用期间独占可写。
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        entries.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            entry.cntThreads,
            String::from_utf16_lossy(&entry.szExeFile[..end]),
        ));
        // SAFETY: 同一有效快照和已初始化的输出结构，调用同步完成。
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    // SAFETY: 紧接失败的枚举调用读取线程错误码，无指针访问。
    if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
        return Err(format!(
            "进程列表读取中断：{}",
            std::io::Error::last_os_error()
        ));
    }
    let root = std::process::id();
    let parents: Vec<_> = entries
        .iter()
        .map(|(pid, parent, _, _)| (*pid, *parent))
        .collect();
    let candidates = descendants(root, &parents);
    let mut read = HashMap::new();
    for (pid, _, threads, name) in &entries {
        if !candidates.contains(pid) {
            continue;
        }
        let kind = if *pid == root {
            "main"
        } else if name.eq_ignore_ascii_case("msedgewebview2.exe") {
            "webview"
        } else {
            "child"
        };
        match read_process(*pid, *threads, kind) {
            Ok(value) => {
                // 快照后新生的同 PID 不能继承旧父子关系；下一帧重新确认。
                if value.0 <= captured_at {
                    read.insert(*pid, value);
                }
            }
            Err(error) if *pid == root => return Err(error),
            // 子进程退出和权限变化很常见；通过 missing_processes 报告，不逐秒写日志。
            Err(_) => {}
        }
    }
    let mut verified = std::collections::BTreeSet::from([root]);
    if !read.contains_key(&root) {
        return Err("进程快照中缺少应用主进程，请重试".into());
    }
    loop {
        let before = verified.len();
        for (pid, parent) in &parents {
            if !verified.contains(parent) {
                continue;
            }
            if let (Some((child_start, _)), Some((parent_start, _))) =
                (read.get(pid), read.get(parent))
            {
                if child_start >= parent_start {
                    verified.insert(*pid);
                }
            }
        }
        if verified.len() == before {
            break;
        }
    }
    let processes: Vec<_> = read
        .into_iter()
        .filter(|(pid, _)| verified.contains(pid))
        .map(|(_, (_, value))| value)
        .collect();
    // SAFETY: 系统只读查询，无指针；跨处理器组统计整机核心数，不受应用亲和性限制。
    let logical_cpus = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
    if logical_cpus == 0 {
        return Err("无法读取整机逻辑核心数".into());
    }
    Ok(ResourceSnapshot {
        memory_metric: "privateWorkingSet",
        partial: false,
        missing_processes: candidates.len().saturating_sub(processes.len()),
        processes,
        logical_cpus: usize::try_from(logical_cpus).map_err(|_| "逻辑核心数超出范围")?,
        coverage: "统计本进程及可验证的子进程；内存及峰值采用私有工作集，系统不支持时不可用；完整工作集与私有提交单独列示。",
    })
}
