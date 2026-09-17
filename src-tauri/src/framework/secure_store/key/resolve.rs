//! 主密钥解析：系统密钥库 → 本地降级文件 → 首次生成（T04/T05 的核心判定都在这里）

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use rand::rngs::OsRng;
use rand::RngCore;

use crate::framework::secure_store::crypto::authenticates_with_aad;
use crate::framework::secure_store::file::{read_optional, replace_file, restrict_to_current_user};

use super::{KeySource, KeySpec, MasterKeyStore, ResolvedKey};

fn promotion_attempted() -> &'static Mutex<HashSet<&'static str>> {
    static ATTEMPTED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    ATTEMPTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 测试用：清掉「本进程已尝试登记」的记录。
/// 断言「旧密钥被登记进密钥库」的用例必须调用，否则结果取决于同进程内谁先跑（顺序依赖即假失败）。
/// 生产语义不变：同一进程内每个 account 仍只尝试一次。
#[cfg(test)]
pub(crate) fn reset_promotion_attempts() {
    if let Ok(mut guard) = promotion_attempted().lock() {
        guard.clear();
    }
}

/// 描述一次密钥库回读结果，**只含存在性与长度信息，不含任何密钥字节**。
/// 单独成函数是为了让「日志不泄露秘密」可以被单测直接断言。
pub(crate) fn describe_readback(scope: &str, result: &Result<Option<[u8; 32]>, String>) -> String {
    match result {
        Ok(Some(_)) => format!("[{scope}] 回读到的主密钥与写入值不一致（长度正确）"),
        Ok(None) => format!("[{scope}] 回读时找不到刚写入的记录"),
        Err(error) => format!("[{scope}] 回读失败：{error}"),
    }
}

/// 读降级密钥文件（不存在返回 None；长度不是 32 字节视为损坏并报错，文件保持原样）
fn read_fallback_key(dir: &Path, spec: &KeySpec) -> Result<Option<[u8; 32]>, String> {
    let path = dir.join(spec.fallback_file);
    let Some(bytes) = read_optional(&path)? else {
        return Ok(None);
    };
    let key: [u8; 32] = bytes.try_into().map_err(|_| {
        format!(
            "降级主密钥文件损坏（长度不是 32 字节，文件保持原样）：{}",
            path.display()
        )
    })?;
    Ok(Some(key))
}

/// 生成 32B 随机主密钥并写入系统密钥库；写失败或写后回读校验不过时降级本地文件（收紧权限）并告警
fn create_master_key(
    dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    // 写入后立刻回读校验：Windows 凭据管理器存在「写返回成功但条目没落库」的静默丢失场景，
    // 不校验的话密文会用一把没存住的密钥加密，下次启动永远无法解锁（死数据）
    let persisted = match store.write(spec.account, &key) {
        Ok(()) => match store.read(spec.account) {
            Ok(Some(readback)) if readback == key => true,
            other => {
                eprintln!(
                    "{}；主密钥降级为本地密钥文件",
                    describe_readback(spec.scope, &other)
                );
                false
            }
        },
        Err(e) => {
            eprintln!(
                "[{}] 密钥库写入失败（{e}）；主密钥降级为本地密钥文件",
                spec.scope
            );
            false
        }
    };
    if !persisted {
        let path = dir.join(spec.fallback_file);
        replace_file(&path, &key)?;
        if let Err(e) = restrict_to_current_user(&path) {
            eprintln!(
                "[{}] 降级密钥文件权限收紧失败（文件已写入，权限维持默认）: {e}",
                spec.scope
            );
        }
    }
    Ok(key)
}

/// 把已验证可用的降级密钥登记进系统密钥库（写后回读校验），不删除降级文件
fn promote_fallback_key(
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    key: &[u8; 32],
    notes: &mut Vec<String>,
) {
    let already = promotion_attempted()
        .lock()
        .map(|mut guard| !guard.insert(spec.account))
        .unwrap_or(false);
    if already {
        notes.push("本地降级密钥仍在使用（本次进程已尝试登记过系统密钥库）".into());
        return;
    }
    match store.write(spec.account, key) {
        Ok(()) => match store.read(spec.account) {
            Ok(Some(readback)) if readback == *key => {
                notes.push("已将本地降级密钥登记到系统密钥库，未删除本地密钥文件".into())
            }
            other => notes.push(describe_readback(spec.scope, &other)),
        },
        Err(e) => notes.push(format!("[{}] 降级密钥登记失败：{e}", spec.scope)),
    }
}

/// 锁死错误文案：区分「两处都没有密钥」「候选对不上密文」等情形，都指向备份恢复入口
fn locked_message(
    spec: &KeySpec,
    has_keyring: bool,
    has_fallback: bool,
    notes: &[String],
) -> String {
    let mut message = match (has_keyring, has_fallback) {
        (true, true) => format!(
            "无法解锁（{}）：系统密钥库与本地密钥文件中的主密钥都无法解密现有密文（可能密文损坏或被替换）。未生成新密钥，文件均保持原样；可用「凭证管理 → 导入」恢复备份",
            spec.scope
        ),
        (true, false) => format!(
            "无法解锁（{}）：系统密钥库中的主密钥无法解密现有密文。未生成新密钥，文件保持原样；可用「凭证管理 → 导入」恢复备份，或确认后删除密文重新初始化",
            spec.scope
        ),
        (false, true) => format!(
            "无法解锁（{}）：本地密钥文件中的主密钥无法解密现有密文。未生成新密钥，文件保持原样；可用「凭证管理 → 导入」恢复备份",
            spec.scope
        ),
        (false, false) => format!(
            "无法解锁（{}）：系统密钥库与本地都没有主密钥，而密文仍在（可能换机 / 重装 / 密钥被清空）。未生成新密钥；可用「凭证管理 → 导入」恢复备份，或确认后删除密文重新初始化",
            spec.scope
        ),
    };
    for note in notes {
        message.push('；');
        message.push_str(note);
    }
    message
}

/// 解析主密钥。`evidence` 是该域现有密文字节（主文件与备份；空表表示还没有密文），
/// `aad` 是密文的认证绑定（空间 uid；2026-09-16 起密文与 uid 数学绑定）。
/// 降级密钥通过认证时会被登记进系统密钥库（见 `promote_fallback_key`）。
pub(crate) fn resolve_master_key(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
    aad: &[u8],
) -> Result<ResolvedKey, String> {
    resolve_master_key_inner(key_dir, spec, store, evidence, aad, true)
}

/// 只读解析主密钥：与 `resolve_master_key` 同判据，但**不写系统密钥库**。
/// 供保护状态查询这类不该有写副作用的调用点使用（否则「打开设置页」会悄悄改密钥库）。
pub(crate) fn resolve_master_key_readonly(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
    aad: &[u8],
) -> Result<ResolvedKey, String> {
    resolve_master_key_inner(key_dir, spec, store, evidence, aad, false)
}

/// 解析主密钥的统一实现；`allow_promotion` 决定是否允许把降级密钥登记进系统密钥库。
fn resolve_master_key_inner(
    key_dir: &Path,
    spec: &KeySpec,
    store: &dyn MasterKeyStore,
    evidence: &[Vec<u8>],
    aad: &[u8],
    allow_promotion: bool,
) -> Result<ResolvedKey, String> {
    std::fs::create_dir_all(key_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let mut notes = Vec::new();
    let keyring = store.read(spec.account);
    if let Err(e) = &keyring {
        // 读取失败只告警继续降级（无桌面环境、密钥库被策略禁用等场景）
        notes.push(format!("[{}] {e}，已尝试本地密钥文件", spec.scope));
    }
    let fallback = read_fallback_key(key_dir, spec);
    if let Err(e) = &fallback {
        notes.push(e.clone());
    }
    let keyring_key = keyring.as_ref().ok().and_then(|option| *option);
    let fallback_key = fallback.as_ref().ok().and_then(|option| *option);
    let has_ciphertext = evidence.iter().any(|data| !data.is_empty());

    if has_ciphertext {
        // 有密文：以能否认证解出既有密文为准，绝不生成新密钥。
        // 只认 uid 绑定格式（2026-09-16 裁决：不做旧格式兼容，旧数据放弃）
        let worth = |key: [u8; 32]| {
            evidence
                .iter()
                .any(|data| authenticates_with_aad(&key, data, aad))
        };
        if let Some(key) = keyring_key.filter(|key| worth(*key)) {
            return Ok(ResolvedKey {
                key,
                source: KeySource::Keyring,
                notes,
            });
        }
        if let Some(key) = fallback_key.filter(|key| worth(*key)) {
            // 密钥库里的版本不对或用不上时，用本地文件里认证通过的那把覆盖登记，
            // 否则下次读取仍要从降级路径兜底
            if allow_promotion {
                promote_fallback_key(spec, store, &key, &mut notes);
            }
            return Ok(ResolvedKey {
                key,
                source: KeySource::FallbackFile,
                notes,
            });
        }
        return Err(locked_message(
            spec,
            keyring_key.is_some(),
            fallback_key.is_some(),
            &notes,
        ));
    }

    // 无密文：可以安全初始化，顺序为密钥库 → 降级文件 → 首次生成
    if let Some(key) = keyring_key {
        return Ok(ResolvedKey {
            key,
            source: KeySource::Keyring,
            notes,
        });
    }
    if let Some(key) = fallback_key {
        if allow_promotion {
            promote_fallback_key(spec, store, &key, &mut notes);
        }
        return Ok(ResolvedKey {
            key,
            source: KeySource::FallbackFile,
            notes,
        });
    }
    // 降级文件存在但损坏时不要静默换一把新密钥：先把损坏事实暴露出来
    let _ = fallback?;
    let key = create_master_key(key_dir, spec, store)?;
    Ok(ResolvedKey {
        key,
        source: KeySource::CreatedNow,
        notes,
    })
}
