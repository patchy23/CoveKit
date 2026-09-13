//! 系统密钥库跨进程持久化验证（T04-2）
//!
//! 单进程内存桩无法证明「主密钥真的进了系统凭据管理器」：keyring 在缺平台特性时会退回
//! 进程内 mock，写后回读在同一进程里也可能成功。这里让**子进程**重新打开一个新 Entry 读父进程写入的值，
//! 只有真正落到系统密钥库才读得到。
//!
//! 约定（避免污染用户真实条目）：
//! - 写 / 读 / 删**全程只用测试专用 service**（`com.patchy23.patchybox.tests`）+ 固定 account，用完即删；
//! - 用例开始先删一次、结束再删一次，上一次中断留下的条目不会累积；
//! - 非 Windows/macOS 编译目标没有原生后端，用例显式跳过并打印原因（不冒充通过）；
//! - 子进程通过重入当前测试可执行文件的同名用例完成读取。
//!
//! 用例整体包在 `#[cfg(test)] mod tests` 里：规范检查器按字面属性排除测试子树，不替它做 cfg 求值。

/// 测试专用 service 名（与生产 service 完全隔离）
const PROBE_SERVICE: &str = "com.patchy23.patchybox.tests";
/// 测试用固定 account（固定名才能被下次运行清理掉）
const PROBE_ACCOUNT: &str = "cross-process-probe";
/// 父进程传给子进程的 account 环境变量
const ENV_ACCOUNT: &str = "PATCHYBOX_KEYRING_PROBE_ACCOUNT";
/// 父进程传给子进程的期望密钥（十六进制）环境变量
const ENV_EXPECTED: &str = "PATCHYBOX_KEYRING_PROBE_EXPECTED_HEX";
/// 子进程用例名（父进程用它重入自身可执行文件）
const CHILD_TEST: &str = "child_reads_key_written_by_parent";

#[cfg(test)]
mod tests {
    use super::super::key::{native_backend_available, MasterKeyStore, ScopedKeyringStore};
    use super::*;
    use rand::RngCore;

    /// 十六进制编码（仅测试诊断用，不引入依赖）
    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// 父进程：写入测试密钥后由子进程读取，验证真的落到系统密钥库
    #[test]
    fn cross_process_persistence() {
        if !native_backend_available() {
            eprintln!(
                "[keyring-probe] 跳过：当前目标平台未启用系统密钥库原生后端（非 Windows/macOS 编译）"
            );
            return;
        }
        let store = ScopedKeyringStore::new(PROBE_SERVICE);
        // 先清一次：上一次中断运行留下的条目不该影响本次判定
        store.delete(PROBE_ACCOUNT).expect("清理上一次测试条目");

        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        store
            .write(PROBE_ACCOUNT, &key)
            .expect("写入测试密钥到系统密钥库");
        assert_eq!(
            store.read(PROBE_ACCOUNT).unwrap(),
            Some(key),
            "同进程回读应一致"
        );

        // 父用例名（测试线程名即用例全名），据此重入同名可执行文件里的子用例
        let own_name = std::thread::current()
            .name()
            .map(str::to_string)
            .unwrap_or_default();
        let child_name = match own_name.rsplit_once("::") {
            Some((module, _)) => format!("{module}::{CHILD_TEST}"),
            None => {
                store.delete(PROBE_ACCOUNT).ok();
                panic!("无法确定用例名，跳过跨进程验证（own={own_name}）");
            }
        };

        let output =
            std::process::Command::new(std::env::current_exe().expect("测试可执行文件路径"))
                .args(["--exact", &child_name, "--nocapture", "--test-threads=1"])
                .env(ENV_ACCOUNT, PROBE_ACCOUNT)
                .env(ENV_EXPECTED, to_hex(&key))
                .output();
        // 无论子进程结果如何都清理测试条目，避免污染用户密钥库
        let cleanup = store.delete(PROBE_ACCOUNT);
        let output = output.expect("启动子进程失败");
        assert!(
            output.status.success(),
            "子进程读取系统密钥库失败：\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        cleanup.expect("清理测试密钥库条目");
        // 清理后必须真的读不到（证明删除生效，没留垃圾条目）
        assert_eq!(
            store.read(PROBE_ACCOUNT).unwrap(),
            None,
            "清理后不应还能读到"
        );
    }

    /// 子进程：读取父进程写入的系统密钥库条目（正常测试运行时不带环境变量，直接返回）
    #[test]
    fn child_reads_key_written_by_parent() {
        let Ok(account) = std::env::var(ENV_ACCOUNT) else {
            eprintln!("[keyring-probe] 未以子进程方式运行，跳过（由父用例重入时才生效）");
            return;
        };
        let expected = std::env::var(ENV_EXPECTED).expect("父进程应传入期望密钥");
        let found = ScopedKeyringStore::new(PROBE_SERVICE)
            .read(&account)
            .expect("子进程读取密钥库");
        assert_eq!(
            found.map(|key| to_hex(&key)).as_deref(),
            Some(expected.as_str()),
            "新建 Entry 必须能读到父进程写入系统密钥库的密钥"
        );
        eprintln!("[keyring-probe] 子进程已从系统密钥库读到父进程写入的密钥");
    }
}
