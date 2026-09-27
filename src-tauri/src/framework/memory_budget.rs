//! 重计算工作内存预留；只按可用物理内存判断，不把交换区或提交限额当作 RAM。

use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Copy)]
struct PhysicalMemory {
    total: u64,
    available: u64,
}

#[derive(Default)]
struct Budget {
    reserved: Mutex<u64>,
    changed: Condvar,
}

/// 守卫覆盖实际重计算生命周期；错误和取消均自动归还预留。
pub(crate) struct MemoryLease<'a> {
    budget: &'a Budget,
    bytes: u64,
}

impl Drop for MemoryLease<'_> {
    fn drop(&mut self) {
        let mut reserved = self.budget.reserved.lock().unwrap_or_else(|e| e.into_inner());
        *reserved = reserved.saturating_sub(self.bytes);
        self.budget.changed.notify_all();
    }
}

fn usable(memory: PhysicalMemory, reserved: u64) -> u64 {
    // 系统余量随机器内存变化，不按任务数量设固定并发上限。
    let margin = (memory.total / 32).clamp(128 * 1024 * 1024, 512 * 1024 * 1024);
    memory.available.min(memory.total).saturating_sub(margin).saturating_sub(reserved)
}

impl Budget {
    fn acquire(
        &self,
        bytes: u64,
        mut sample: impl FnMut() -> Result<PhysicalMemory, String>,
        check: &impl Fn() -> Result<(), String>,
        waiting: &impl Fn(bool, u64) -> Result<(), String>,
    ) -> Result<MemoryLease<'_>, String> {
        let mut announced = false;
        loop {
            check()?;
            let mut reserved = self.reserved.lock().map_err(|e| e.to_string())?;
            let memory = sample()?;
            if usable(memory, *reserved) >= bytes {
                *reserved = reserved.checked_add(bytes).ok_or("工作内存预留计数溢出")?;
                drop(reserved);
                let lease = MemoryLease { budget: self, bytes };
                check()?;
                waiting(false, bytes)?;
                return Ok(lease);
            }
            if !announced {
                drop(reserved);
                waiting(true, bytes)?;
                announced = true;
                continue;
            }
            // 外部进程释放内存没有本进程通知，因此定期重采样；取消最多等待一个短周期。
            let (_guard, _) = self.changed.wait_timeout(reserved, Duration::from_millis(200))
                .map_err(|e| e.to_string())?;
        }
    }
}

/// 阻塞工作线程使用的可取消准入；内存充足时直接领取，可同时运行多个任务。
pub(crate) fn reserve(
    bytes: u64,
    check: &impl Fn() -> Result<(), String>,
    waiting: &impl Fn(bool, u64) -> Result<(), String>,
) -> Result<MemoryLease<'static>, String> {
    static BUDGET: OnceLock<Budget> = OnceLock::new();
    BUDGET.get_or_init(Budget::default).acquire(bytes, physical_memory, check, waiting)
}

#[cfg(windows)]
fn physical_memory() -> Result<PhysicalMemory, String> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: MEMORYSTATUSEX 是纯数值 C 结构，按 Win32 约定先填 dwLength；指针仅在调用内使用。
    let mut memory: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    memory.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: memory 是已初始化、可写且大小正确的独占栈对象。
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
        return Err(format!("读取可用物理内存失败: {}", std::io::Error::last_os_error()));
    }
    if memory.ullTotalPhys == 0 { return Err("物理内存采样无效".into()); }
    Ok(PhysicalMemory { total: memory.ullTotalPhys, available: memory.ullAvailPhys })
}

#[cfg(target_os = "macos")]
fn physical_memory() -> Result<PhysicalMemory, String> {
    use std::ffi::{c_char, c_int, c_void};
    // SAFETY: 声明对应 macOS libSystem 的公开 Mach/sysctl ABI，调用方逐一检查缓冲与返回值。
    #[link(name = "System")]
    unsafe extern "C" {
        fn mach_host_self() -> u32;
        static mach_task_self_: u32;
        fn mach_port_deallocate(task: u32, name: u32) -> c_int;
        fn host_page_size(host: u32, size: *mut usize) -> c_int;
        fn host_statistics(host: u32, flavor: c_int, info: *mut c_int, count: *mut u32) -> c_int;
        fn sysctlbyname(name: *const c_char, old: *mut c_void, length: *mut usize, new: *mut c_void, new_length: usize) -> c_int;
    }
    let mut total = 0u64;
    let mut length = std::mem::size_of::<u64>();
    let mut page_size = 0usize;
    // XNU vm_statistics 的稳定 ABI 为 15 个 natural_t；HOST_VM_INFO = 2。
    // 不读取 swap、压缩后虚拟页或累计 pageins/pageouts；free 已包含 speculative，不能重复相加。
    let mut statistics = [0u32; 15];
    let mut count = statistics.len() as u32;
    // SAFETY: 名称以 NUL 结尾，所有输出缓冲大小正确；host send right 在所有返回路径前归还。
    let (total_ok, pages_ok, stats_ok, release_ok) = unsafe {
        let total_ok = sysctlbyname(c"hw.memsize".as_ptr(), (&mut total as *mut u64).cast(), &mut length, std::ptr::null_mut(), 0);
        let host = mach_host_self();
        let pages_ok = host_page_size(host, &mut page_size);
        let stats_ok = host_statistics(host, 2, statistics.as_mut_ptr().cast(), &mut count);
        let release_ok = mach_port_deallocate(mach_task_self_, host);
        (total_ok, pages_ok, stats_ok, release_ok)
    };
    if total_ok != 0 || pages_ok != 0 || stats_ok != 0 || release_ok != 0 || length != 8 || count < 3 || page_size == 0 || total == 0 {
        return Err("读取可用物理内存失败".into());
    }
    // 采用空闲和非活动物理页的保守估计；不将活动匿名页或交换容量作为可用量。
    let available = (u64::from(statistics[0]) + u64::from(statistics[2])).saturating_mul(page_size as u64);
    Ok(PhysicalMemory { total, available })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn physical_memory() -> Result<PhysicalMemory, String> {
    Err("当前平台不支持物理内存准入检查".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn sufficient_physical_memory_preserves_concurrency_and_releases_reservations() {
        let budget = Budget::default();
        let memory = PhysicalMemory { total: 8 << 30, available: 2 << 30 };
        let first = budget.acquire(256 << 20, || Ok(memory), &|| Ok(()), &|waiting, _| { assert!(!waiting); Ok(()) }).unwrap();
        let second = budget.acquire(256 << 20, || Ok(memory), &|| Ok(()), &|waiting, _| { assert!(!waiting); Ok(()) }).unwrap();
        assert_eq!(*budget.reserved.lock().unwrap(), 512 << 20);
        drop(first);
        drop(second);
        assert_eq!(*budget.reserved.lock().unwrap(), 0);
    }

    #[test]
    fn pressure_wait_is_observable_and_cancelled_without_reserving_memory() {
        let budget = Budget::default();
        let cancelled = AtomicBool::new(false);
        let result = budget.acquire(256 << 20,
            || Ok(PhysicalMemory { total: 8 << 30, available: 128 << 20 }),
            &|| if cancelled.load(Ordering::SeqCst) { Err("已取消".into()) } else { Ok(()) },
            &|waiting, bytes| { assert!(waiting); assert_eq!(bytes, 256 << 20); cancelled.store(true, Ordering::SeqCst); Ok(()) });
        assert!(matches!(result, Err(error) if error == "已取消"));
        assert_eq!(*budget.reserved.lock().unwrap(), 0);
    }

    #[test]
    fn recovered_memory_resumes_without_changing_cost() {
        let budget = Budget::default();
        let available = std::cell::Cell::new(128 << 20);
        let states = std::cell::RefCell::new(Vec::new());
        let lease = budget.acquire(256 << 20,
            || Ok(PhysicalMemory { total: 8 << 30, available: available.get() }), &|| Ok(()),
            &|waiting, bytes| { states.borrow_mut().push(waiting); assert_eq!(bytes, 256 << 20); available.set(1 << 30); Ok(()) }).unwrap();
        assert_eq!(*states.borrow(), vec![true, false]);
        assert_eq!(lease.bytes, 256 << 20);
    }

    #[test]
    fn cancellation_after_admission_and_sampler_failure_do_not_leak_reservations() {
        let budget = Budget::default();
        let checks = std::cell::Cell::new(0);
        let result = budget.acquire(256 << 20,
            || Ok(PhysicalMemory { total: 8 << 30, available: 2 << 30 }),
            &|| { checks.set(checks.get() + 1); if checks.get() == 2 { Err("已取消".into()) } else { Ok(()) } },
            &|_, _| Ok(()));
        assert!(matches!(result, Err(error) if error == "已取消"));
        assert_eq!(*budget.reserved.lock().unwrap(), 0);
        let failed = budget.acquire(256 << 20, || Err("采样失败".into()), &|| Ok(()), &|_, _| Ok(()));
        assert!(matches!(failed, Err(error) if error == "采样失败"));
        assert_eq!(*budget.reserved.lock().unwrap(), 0);
    }
}
