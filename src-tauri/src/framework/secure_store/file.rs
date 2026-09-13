//! 密文文件的落盘与恢复原语（唯一实现：写临时文件 → fsync → 旧文件转 `.bak` → 转正 → 清理 `.bak`）
//!
//! 崩溃点与恢复规则（写路径三步 + 读路径择版）：
//! 1. 写临时文件途中崩溃：主文件与 `.bak` 都还是旧完整版；残留临时文件因**唯一名**不会被下次写覆盖，
//!    读路径也从不读它；
//! 2. 主文件已改名成 `.bak`、临时文件尚未转正时崩溃：主文件缺失而 `.bak` 在 → 读路径把 `.bak` 转正，
//!    拿回旧完整版；
//! 3. 临时文件已转正、`.bak` 尚未清理时崩溃：主文件是新完整版、`.bak` 是旧完整版 → 认证通过则采用主文件，
//!    并清理这次写入遗留的 `.bak`；
//! 4. 主文件在但认证/解析失败、`.bak` 可读：坏文件改名归档（`.corrupt-<时间戳>`），`.bak` 转正；
//! 5. 主文件与 `.bak` 都不可用：报错并保持文件原样，绝不落空表、绝不删备份。
//!
//! 调用前提：**写路径只在对应读路径成功之后调用**，因此替换前的主文件是上一次成功提交的版本，
//! 清理过期 `.bak` 不会丢掉唯一可读副本。

use std::io::Write;
use std::path::{Path, PathBuf};

/// 备份文件路径（主文件名 + `.bak` 扩展）
pub(crate) fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("bak")
}

/// 归档路径：坏文件改名留档，绝不删除（`<主文件名>.corrupt-<unix 秒>`）
fn corrupt_path(path: &Path) -> PathBuf {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("credential-file");
    path.with_file_name(format!("{stem}.corrupt-{}", chrono::Utc::now().timestamp()))
}

/// 读文件内容，缺失返回 `None`（其它 IO 错误上报）
pub(crate) fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("凭证文件读取失败: {error}")),
    }
}

/// 原子替换文件内容：唯一临时名 + fsync + 旧文件转 `.bak`，成功后清理 `.bak`。
/// 失败时尽最大努力回滚（把 `.bak` 还原为主文件），回滚也失败则报出备份所在路径。
pub(crate) fn replace_file(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建凭证目录失败: {e}"))?;
    }
    // 唯一临时名：同目录并发/中断残留都不会互相覆盖（旧实现用固定 `.tmp`，崩溃残留会被误当成新写入）
    let tmp = path.with_file_name(format!(
        "{}.tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("credential-file"),
        uuid::Uuid::new_v4()
    ));
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("临时凭证文件创建失败: {e}"))?;
    file.write_all(content)
        .map_err(|e| format!("临时凭证文件写入失败: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("临时凭证文件刷盘失败: {e}"))?;
    drop(file);

    let backup = backup_path(path);
    if !path.exists() {
        return std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("凭证文件写入失败: {e}")
        });
    }
    // 上一次写入若在清理前中断，会留下过期 `.bak`；此处的旧主文件是已提交版本，可安全让位
    if backup.exists() {
        std::fs::remove_file(&backup).map_err(|e| format!("旧凭证备份清理失败: {e}"))?;
    }
    std::fs::rename(path, &backup).map_err(|e| format!("旧凭证文件备份失败: {e}"))?;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let restore = std::fs::rename(&backup, path);
        return match restore {
            Ok(()) => Err(format!("凭证文件替换失败，已恢复旧数据: {error}")),
            Err(restore_error) => Err(format!(
                "凭证文件替换与恢复均失败: {error}; {restore_error}（旧数据位于 {}）",
                backup.display()
            )),
        };
    }
    // 到这里新版本已提交（文件已转正）；清理 `.bak` 属于收尾，失败只影响下次择版，不阻断写入
    if let Err(error) = std::fs::remove_file(&backup) {
        eprintln!("[secure-store] 凭证已保存，但备份清理失败: {error}");
    }
    Ok(())
}

/// 把坏密文改名归档留档（不删除），返回归档路径
pub(crate) fn archive_corrupt(path: &Path) -> Result<PathBuf, String> {
    let target = corrupt_path(path);
    std::fs::rename(path, &target)
        .map_err(|e| format!("坏凭证文件留档失败（原文件未改动）: {e}"))?;
    Ok(target)
}

/// 按「认证 + 解析」结果读取密文：先恢复中断遗留备份，再决定采用主文件还是备份。
///
/// - 主文件缺失且备份在 → 备份转正后读取；
/// - 主文件可解 → 采用主文件，并清理这次写入遗留的 `.bak`；
/// - 主文件不可解而备份可解 → 主文件改名归档、备份转正后读取（绝不直接删备份）；
/// - 两者都不可解 → 报错，两个文件原样保留。
pub(crate) fn load_verified<T>(
    path: &Path,
    key: &[u8; 32],
    decode: &dyn Fn(&[u8]) -> Result<T, String>,
) -> Result<Option<T>, String> {
    let backup = backup_path(path);
    let decode_with = |data: &[u8]| -> Result<T, String> {
        let plain = super::crypto::decrypt_payload(key, data)?;
        decode(&plain)
    };
    let Some(main) = read_optional(path)? else {
        let Some(backup_data) = read_optional(&backup)? else {
            // 真正的空环境：主文件与备份都不存在
            return Ok(None);
        };
        let value = decode_with(&backup_data).map_err(|e| {
            format!(
                "备份无法读取且主文件不存在（备份保留在 {}）: {e}",
                backup.display()
            )
        })?;
        std::fs::rename(&backup, path).map_err(|e| format!("凭证备份恢复失败: {e}"))?;
        return Ok(Some(value));
    };
    match decode_with(&main) {
        Ok(value) => {
            // 主文件已提交：把残留 `.bak` 当过期版本清理（内容仍在主文件里）
            if backup.exists() {
                if let Err(error) = std::fs::remove_file(&backup) {
                    eprintln!("[secure-store] 过期凭证备份清理失败: {error}");
                }
            }
            Ok(Some(value))
        }
        Err(main_error) => {
            let Some(backup_data) = read_optional(&backup)? else {
                return Err(main_error);
            };
            let value = decode_with(&backup_data).map_err(|backup_error| {
                format!("{main_error}；备份同样无法读取（两个文件均保留原样）: {backup_error}")
            })?;
            let archived = archive_corrupt(path)?;
            std::fs::rename(&backup, path).map_err(|e| {
                format!(
                    "凭证备份转正失败（坏文件已留档于 {}）: {e}",
                    archived.display()
                )
            })?;
            eprintln!(
                "[secure-store] 主凭证文件无法读取，已留档为 {} 并改用备份",
                archived.display()
            );
            Ok(Some(value))
        }
    }
}

/// 收紧文件权限到「仅当前用户」（降级密钥文件用）。
/// Windows 走 DACL 重建，macOS/Linux 走 0600；其它平台不支持则明确报错（不静默假装已收紧）。
pub(crate) fn restrict_to_current_user(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        super::win_acl::restrict_to_current_user(path)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions)
            .map_err(|e| format!("密钥文件权限收紧失败: {e}"))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        Err("当前平台未实现密钥文件权限收紧".into())
    }
}

/// 收集这些路径现有密文（主文件 + 同名备份）作为「以密文为准」的密钥判定证据。
/// 备份也算证据：只剩备份时不能因为「主文件不存在」就把既有数据判为空。
pub(crate) fn ciphertext_evidence(paths: &[PathBuf]) -> Result<Vec<Vec<u8>>, String> {
    let mut evidence = Vec::new();
    for path in paths {
        for candidate in [path.clone(), backup_path(path)] {
            if let Some(data) = read_optional(&candidate)? {
                if !data.is_empty() {
                    evidence.push(data);
                }
            }
        }
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::super::crypto::encrypt_payload;
    use super::*;

    /// 测试用临时目录（进程 id + 名称唯一）
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "secure-store-file-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 明文直通解码器（本文件只验证文件层，不掺加密）
    fn identity(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }

    /// 首次写入不产生备份；二次写入成功后备份被清理；临时文件唯一名不残留
    #[test]
    fn replace_file_keeps_single_complete_version() {
        let dir = temp_dir("replace");
        let path = dir.join("vault.dat");
        replace_file(&path, b"v1").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v1");
        assert!(!backup_path(&path).exists());

        replace_file(&path, b"v2").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"v2");
        assert!(!backup_path(&path).exists());

        // 目录里不留临时文件（唯一名写完后被 rename 掉）
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件: {leftovers:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃点 2：主文件缺失而备份在 → 读路径把备份转正
    #[test]
    fn load_recovers_when_main_missing() {
        let dir = temp_dir("recover-missing");
        let path = dir.join("vault.dat");
        let key = [3u8; 32];
        replace_file(&path, &encrypt_payload(&key, b"v1").unwrap()).unwrap();
        // 模拟「主文件已转 .bak、新文件尚未转正」的中断现场
        std::fs::rename(&path, backup_path(&path)).unwrap();
        let value = load_verified(&path, &key, &identity).unwrap();
        assert_eq!(value.as_deref(), Some(&b"v1"[..]));
        assert!(path.exists());
        assert!(!backup_path(&path).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃点 3：主文件已提交、备份残留 → 采用主文件并清理备份
    #[test]
    fn load_prefers_committed_main_and_cleans_backup() {
        let dir = temp_dir("prefer-main");
        let path = dir.join("vault.dat");
        let key = [3u8; 32];
        std::fs::write(&path, encrypt_payload(&key, b"new").unwrap()).unwrap();
        std::fs::write(backup_path(&path), encrypt_payload(&key, b"old").unwrap()).unwrap();
        let value = load_verified(&path, &key, &identity).unwrap();
        assert_eq!(value.as_deref(), Some(&b"new"[..]));
        assert!(!backup_path(&path).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃点 4：主文件坏而备份可用 → 坏文件归档留档、备份转正，绝不删备份
    #[test]
    fn load_adopts_backup_when_main_corrupt() {
        let dir = temp_dir("adopt-backup");
        let path = dir.join("vault.dat");
        let key = [3u8; 32];
        // 主文件用真实损坏形态（被截断的密文），备份是可用密文
        std::fs::write(&path, b"broken").unwrap();
        let good = encrypt_payload(&key, b"good").unwrap();
        std::fs::write(backup_path(&path), &good).unwrap();
        let value = load_verified(&path, &key, &identity).unwrap();
        assert_eq!(value.as_deref(), Some(&b"good"[..]));
        assert_eq!(std::fs::read(&path).unwrap(), good);
        assert!(!backup_path(&path).exists());
        let archived: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".corrupt-"))
            .collect();
        assert_eq!(archived.len(), 1, "坏文件应留档一份: {archived:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃点 4 的坏版本按「认证」判定：主文件用错误密钥加密时同样改用备份
    #[test]
    fn load_adopts_backup_when_main_key_mismatch() {
        use super::super::crypto::encrypt_payload;
        let dir = temp_dir("adopt-key-mismatch");
        let path = dir.join("vault.dat");
        let right = [5u8; 32];
        std::fs::write(&path, encrypt_payload(&[9u8; 32], b"other").unwrap()).unwrap();
        std::fs::write(
            backup_path(&path),
            encrypt_payload(&right, b"mine").unwrap(),
        )
        .unwrap();
        let value = load_verified(&path, &right, &identity).unwrap();
        assert_eq!(value.as_deref(), Some(&b"mine"[..]));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 崩溃点 5：两个文件都不可用 → 报错且两个文件原样保留（不落空表、不删备份）
    #[test]
    fn load_errors_keeps_both_files_when_unreadable() {
        let dir = temp_dir("both-bad");
        let path = dir.join("vault.dat");
        std::fs::write(&path, b"broken").unwrap();
        std::fs::write(backup_path(&path), b"also-broken").unwrap();
        let decode = |data: &[u8]| {
            if data == b"broken" || data == b"also-broken" {
                Err("坏数据".to_string())
            } else {
                Ok(data.to_vec())
            }
        };
        let key = [3u8; 32];
        let error = load_verified(&path, &key, &decode).unwrap_err();
        assert!(
            error.contains("保留原样"),
            "错误文案应说明文件保留: {error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        assert_eq!(std::fs::read(backup_path(&path)).unwrap(), b"also-broken");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空环境：主文件与备份都不存在 → None（只有这一种情况才能当空数据）
    #[test]
    fn load_returns_none_only_for_empty_environment() {
        let dir = temp_dir("empty");
        let path = dir.join("vault.dat");
        let key = [3u8; 32];
        assert!(load_verified(&path, &key, &identity).unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 主文件缺失但备份坏 → 报错（不把「读不出」当空表）且备份保留
    #[test]
    fn load_reports_broken_backup_instead_of_empty() {
        let dir = temp_dir("broken-backup");
        let path = dir.join("vault.dat");
        std::fs::write(backup_path(&path), b"broken").unwrap();
        let decode = |data: &[u8]| {
            if data == b"broken" {
                Err("坏数据".to_string())
            } else {
                Ok(data.to_vec())
            }
        };
        let key = [3u8; 32];
        assert!(load_verified(&path, &key, &decode).is_err());
        assert!(backup_path(&path).exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
