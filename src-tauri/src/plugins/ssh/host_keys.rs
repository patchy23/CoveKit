//! SSH 插件 · 已知主机管理（CoveKit 私有 known_hosts 文件）
//! 存储沿用 russh 的 OpenSSH 行格式（[host]:port algorithm base64），零迁移成本；
//! 本模块在其上提供：条目解析（含 SHA256 指纹）、按指纹删除、按新密钥替换。
//! 文件为本应用私有（非用户 ~/.ssh/known_hosts），可放心整行增删。

use std::path::{Path, PathBuf};

use russh::keys::known_hosts::{known_host_keys_path, learn_known_hosts_path};
use russh::keys::{HashAlg, PublicKey};

use crate::plugins::ssh::models::KnownHostEntry;

/// 已知主机文件路径（数据分区下 ssh-known-hosts，经 `framework::paths` 解析并带旧布局回落）
pub(crate) fn known_hosts_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    crate::framework::paths::data_path(app, "ssh-known-hosts")
}

/// 公钥的 SHA256 指纹（Display 形如 SHA256:base64）
pub(crate) fn fingerprint_of(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// 读取全部已知主机条目（解析失败的单行跳过，不阻断列表）
pub(crate) fn list_entries(path: &Path) -> Result<Vec<KnownHostEntry>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("known_hosts 读取失败: {e}"))?;
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(marker), Some(algorithm), Some(base64)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        // 哈希主机名的条目（|1|salt|hash）无法还原明文，占位展示
        let (host, port) = parse_marker(marker);
        let key = match PublicKey::from_openssh(&format!("{algorithm} {base64}")) {
            Ok(key) => key,
            Err(_) => continue,
        };
        entries.push(KnownHostEntry {
            host,
            port,
            algorithm: key.algorithm().as_str().to_string(),
            fingerprint: fingerprint_of(&key),
        });
    }
    Ok(entries)
}

/// 解析 known_hosts 行首主机标记：`host`（22 端口）或 `[host]:port`；哈希条目返回占位。
fn parse_marker(marker: &str) -> (String, u16) {
    if let Some(rest) = marker.strip_prefix("|1|") {
        let _ = rest;
        return ("(hashed)".into(), 22);
    }
    if let Some(rest) = marker.strip_prefix('[') {
        if let Some((host, port)) = rest.split_once("]:") {
            return (host.to_string(), port.parse().unwrap_or(22));
        }
    }
    (marker.to_string(), 22)
}

/// 读取某主机:端口当前保存的全部条目
pub(crate) fn entries_for(
    path: &Path,
    host: &str,
    port: u16,
) -> Result<Vec<KnownHostEntry>, String> {
    Ok(known_host_keys_path(host, port, path)
        .map_err(|e| format!("known_hosts 解析失败: {e}"))?
        .into_iter()
        .map(|(_, key)| KnownHostEntry {
            host: host.to_string(),
            port,
            algorithm: key.algorithm().as_str().to_string(),
            fingerprint: fingerprint_of(&key),
        })
        .collect())
}

/// 保存（learn）服务器公钥；委托 russh 写入，保持 OpenSSH 行格式
pub(crate) fn learn(path: &Path, host: &str, port: u16, key: &PublicKey) -> Result<(), String> {
    learn_known_hosts_path(host, port, key, path).map_err(|e| format!("known_hosts 写入失败: {e}"))
}

/// 删除某主机:端口下与指定指纹匹配的条目；fingerprint 为空时删除该主机全部条目。返回删除行数。
pub(crate) fn delete_entries(
    path: &Path,
    host: &str,
    port: u16,
    fingerprint: &str,
) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("known_hosts 读取失败: {e}"))?;
    let mut removed = 0;
    let mut kept = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // 空行/注释/无法解析的行一律保留；主机+端口匹配且（指纹为空或指纹相等）时删除
        let drop_line = parse_line_host_port(trimmed)
            .map(|(entry_host, entry_port)| entry_host == host && entry_port == port)
            .unwrap_or(false)
            && (fingerprint.is_empty()
                || entries_for_line(trimmed)
                    .map(|e| e.fingerprint == fingerprint)
                    .unwrap_or(false));
        if drop_line {
            removed += 1;
        } else {
            kept.push(line.to_string());
        }
    }
    if removed > 0 {
        write_lines(path, &kept)?;
    }
    Ok(removed)
}

/// 替换主机全部旧密钥为新密钥（指纹变更时用户确认后调用）
pub(crate) fn replace_entries(
    path: &Path,
    host: &str,
    port: u16,
    key: &PublicKey,
) -> Result<(), String> {
    delete_entries(path, host, port, "")?;
    learn(path, host, port, key)
}

/// 解析一行的主机标记；注释/空行返回 None
fn parse_line_host_port(line: &str) -> Option<(String, u16)> {
    let marker = line.split_whitespace().next()?;
    Some(parse_marker(marker))
}

/// 解析单行条目为 KnownHostEntry（无法解析返回 None）
fn entries_for_line(line: &str) -> Option<KnownHostEntry> {
    let mut parts = line.split_whitespace();
    let (marker, algorithm, base64) = (parts.next()?, parts.next()?, parts.next()?);
    let key = PublicKey::from_openssh(&format!("{algorithm} {base64}")).ok()?;
    let (host, port) = parse_marker(marker);
    Some(KnownHostEntry {
        host,
        port,
        algorithm: key.algorithm().as_str().to_string(),
        fingerprint: fingerprint_of(&key),
    })
}

/// 原子写回（临时文件 + 替换），失败时保留原文件
fn write_lines(path: &Path, lines: &[String]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    std::fs::write(&tmp, content).map_err(|e| format!("known_hosts 写入失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("known_hosts 替换失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// ed25519 测试公钥的 OpenSSH 单行表示
    const ED25519_KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB0Qz8YnP5jelia/Hi6g3ew6vqNKSnZdYmFOERgnpt05";

    fn tmp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("pb-hostkey-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn learn_then_list_roundtrip() {
        let path = tmp_path("roundtrip");
        let key = PublicKey::from_openssh(ED25519_KEY).unwrap();
        learn(&path, "example.com", 22, &key).unwrap();
        let entries = list_entries(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].host, "example.com");
        assert_eq!(entries[0].port, 22);
        assert_eq!(entries[0].algorithm, "ssh-ed25519");
        assert!(entries[0].fingerprint.starts_with("SHA256:"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn entries_for_matches_port_marker() {
        let path = tmp_path("port");
        let key = PublicKey::from_openssh(ED25519_KEY).unwrap();
        learn(&path, "10.0.0.5", 2222, &key).unwrap();
        // russh 对非 22 端口写 [host]:port 标记
        assert_eq!(entries_for(&path, "10.0.0.5", 2222).unwrap().len(), 1);
        assert_eq!(entries_for(&path, "10.0.0.5", 22).unwrap().len(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn delete_by_fingerprint_keeps_other_host() {
        let path = tmp_path("delete");
        let key = PublicKey::from_openssh(ED25519_KEY).unwrap();
        learn(&path, "a.example.com", 22, &key).unwrap();
        learn(&path, "b.example.com", 22, &key).unwrap();
        let fp = entries_for(&path, "a.example.com", 22).unwrap()[0]
            .fingerprint
            .clone();
        let removed = delete_entries(&path, "a.example.com", 22, &fp).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(entries_for(&path, "a.example.com", 22).unwrap().len(), 0);
        assert_eq!(entries_for(&path, "b.example.com", 22).unwrap().len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn replace_swaps_saved_key() {
        let path = tmp_path("replace");
        let old = PublicKey::from_openssh(ED25519_KEY).unwrap();
        // 另一把 ed25519 测试钥（不同指纹）
        let new = PublicKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOWkwrJ/Og5dqpPrZQYmXNnP7F0T0dxHpgVMbM6Id9UG",
        )
        .unwrap();
        learn(&path, "host.example", 22, &old).unwrap();
        replace_entries(&path, "host.example", 22, &new).unwrap();
        let entries = entries_for(&path, "host.example", 22).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fingerprint, fingerprint_of(&new));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_marker_handles_bracket_port() {
        assert_eq!(parse_marker("example.com"), ("example.com".into(), 22));
        assert_eq!(parse_marker("[10.0.0.5]:2222"), ("10.0.0.5".into(), 2222));
    }
}
