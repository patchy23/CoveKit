//! 测试替身：内存主密钥库（可注入读失败 / 写失败 / 静默丢写）
//! 仅编译测试目标；它同时记录写入次数，用于验证「降级密钥只登记一次」这类行为。

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use super::file::{replace_file, restrict_to_current_user};
use super::key::{KeySpec, MasterKeyStore};

/// 写入降级密钥文件（含权限收紧），模拟「系统密钥库不可用时的历史密钥文件」。
/// 失败交给调用方断言，测试替身自己不 panic。
pub(crate) fn seed_fallback_file(dir: &Path, spec: &KeySpec, key: &[u8; 32]) -> Result<(), String> {
    let path = dir.join(spec.fallback_file);
    replace_file(&path, key)?;
    if let Err(error) = restrict_to_current_user(&path) {
        eprintln!("[secure-store] 测试降级密钥文件权限收紧失败: {error}");
    }
    Ok(())
}

/// 串行化「降级密钥登记」相关用例。
///
/// 「每 account 每进程只登记一次」是进程级共享状态：并行跑时先进入登记分支的用例会把记录置上，
/// 后进入的直接跳过，断言就随调度顺序时绿时红。凡会进入登记分支的用例都必须先取此锁，
/// 需要断言登记结果的再调 `reset_promotion_attempts()`。
pub(crate) fn promotion_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 内存主密钥库：按 account 存 32 字节密钥
pub(crate) struct MemoryKeyStore {
    /// 已存储的密钥（键为 account）
    keys: RefCell<HashMap<String, [u8; 32]>>,
    /// 注入读失败（模拟无桌面环境 / 密钥库被策略禁用）
    fail_read: RefCell<bool>,
    /// 注入写失败
    fail_write: RefCell<bool>,
    /// 注入静默丢写（write 返回 Ok 但不落库，模拟 Windows 凭据管理器丢失场景）
    lose_writes: RefCell<bool>,
    /// 写入调用次数（含失败与丢写）
    write_attempts: RefCell<usize>,
}

impl MemoryKeyStore {
    /// 构造空密钥库
    pub(crate) fn new() -> Self {
        MemoryKeyStore {
            keys: RefCell::new(HashMap::new()),
            fail_read: RefCell::new(false),
            fail_write: RefCell::new(false),
            lose_writes: RefCell::new(false),
            write_attempts: RefCell::new(0),
        }
    }

    /// 构造已含某个 account 密钥的内存密钥库
    pub(crate) fn with_key(account: &str, key: [u8; 32]) -> Self {
        let store = MemoryKeyStore::new();
        store.keys.borrow_mut().insert(account.to_string(), key);
        store
    }

    /// 设置读失败注入
    pub(crate) fn set_fail_read(&self, value: bool) {
        *self.fail_read.borrow_mut() = value;
    }

    /// 设置写失败注入
    pub(crate) fn set_fail_write(&self, value: bool) {
        *self.fail_write.borrow_mut() = value;
    }

    /// 设置静默丢写注入
    pub(crate) fn set_lose_writes(&self, value: bool) {
        *self.lose_writes.borrow_mut() = value;
    }

    /// 查询某 account 当前存着的密钥
    pub(crate) fn key_of(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        if *self.fail_read.borrow() {
            return Err("注入的读失败".into());
        }
        Ok(self.keys.borrow().get(account).copied())
    }

    /// 写入调用次数
    pub(crate) fn write_attempts(&self) -> usize {
        *self.write_attempts.borrow()
    }
}

impl MasterKeyStore for MemoryKeyStore {
    fn read(&self, account: &str) -> Result<Option<[u8; 32]>, String> {
        if *self.fail_read.borrow() {
            return Err("注入的读失败".into());
        }
        Ok(self.keys.borrow().get(account).copied())
    }

    fn write(&self, account: &str, key: &[u8; 32]) -> Result<(), String> {
        *self.write_attempts.borrow_mut() += 1;
        if *self.fail_write.borrow() {
            return Err("注入的写失败".into());
        }
        if !*self.lose_writes.borrow() {
            self.keys.borrow_mut().insert(account.to_string(), *key);
        }
        Ok(())
    }
}
