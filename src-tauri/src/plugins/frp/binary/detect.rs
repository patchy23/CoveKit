//! FRP 客户端路径定位和版本探测。

use super::bin_dir;
use super::exe_name;
use super::setting;
use crate::plugins::frp::models::FrpBinaryInfo;
use crate::plugins::frp::models::FrpBinarySource;
use std::path::Path;
use std::path::PathBuf;
use tauri::AppHandle;
use tokio::process::Command;

/// 用户目录下的常见落点（手工解压后的典型位置，只做只读探测）
fn common_candidates() -> Vec<PathBuf> {
    let Some(home) = std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
    else {
        return Vec::new();
    };
    let home = PathBuf::from(home);
    let names = [
        "frp/frpc.exe",
        "frp/frpc",
        "frp/bin/frpc.exe",
        "Downloads/frp/frpc.exe",
        "Downloads/frp/frpc",
        "bin/frpc.exe",
        "bin/frpc",
    ];
    names.iter().map(|rel| home.join(rel)).collect()
}

/// 版本号 → 可比较的数值段（`0.71.0` 大于 `0.9.0`；非数字段按 0 处理）
pub(super) fn version_key(version: &str) -> Vec<u32> {
    version
        .split(['.', '-', '_'])
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

/// 下载目录里按版本号命名的 frpc 中最新的一支（`frpc-<版本><后缀>`）
///
/// 用途：既有用户可能在引入「客户端清单」之前就用旧版一键下载装过 frpc，
/// 这些文件不在 clients 表里，需要靠扫描目录补登记，否则界面会显示「没有客户端」。
async fn latest_versioned_exe(dir: &Path) -> Option<PathBuf> {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("frpc-") || !name.ends_with(suffix) {
            continue;
        }
        let version = name.trim_start_matches("frpc-").trim_end_matches(suffix);
        let key = version_key(version);
        let better = match best.as_ref() {
            Some((current, _)) => key > *current,
            None => true,
        };
        if better {
            best = Some((key, path));
        }
    }
    best.map(|(_, path)| path)
}

/// 在 PATH 中查找可执行文件（逐目录探测，不调用外部 which）
pub(super) fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// 读取版本号：`frpc -v` 输出形如 `frpc version 0.71.0`；失败返回 None（不影响可用性判定）
pub(crate) async fn probe_version(exe: &Path) -> Option<String> {
    let mut command = Command::new(exe);
    // 后台版本探测不创建控制台窗口，避免添加或刷新客户端时闪黑窗。
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = command.arg("-v").output().await.ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.split_whitespace()
        .find(|token| {
            token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
        })
        .map(String::from)
}

/// 组装探测结果（顺带带上此前的降级原因，便于前端提示"为什么没用你填的路径"）
pub(super) async fn describe(
    path: PathBuf,
    source: FrpBinarySource,
    warning: Option<String>,
) -> FrpBinaryInfo {
    let version = probe_version(&path).await;
    FrpBinaryInfo {
        ok: true,
        path: Some(path.display().to_string()),
        version,
        source: Some(source),
        error: warning,
    }
}

/// 探测 frpc：显式路径（设置项或命令入参）→ 下载目录 → PATH → 常见位置
pub(crate) async fn detect_with(app: &AppHandle, explicit: Option<&str>) -> FrpBinaryInfo {
    let mut warning: Option<String> = None;

    // 1) 显式路径（命令入参优先于设置项）
    let configured = explicit
        .map(String::from)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| setting(app, "frpcPath").filter(|value| !value.trim().is_empty()));
    if let Some(raw) = configured {
        let candidate = PathBuf::from(raw.trim());
        if candidate.is_file() {
            return describe(candidate, FrpBinarySource::Settings, None).await;
        }
        warning = Some(format!("配置的 frpc 路径不存在：{}", candidate.display()));
    }

    // 2) 本工具下载目录：优先无版本号的 frpc.exe（旧版遗留），
    //    其次按版本号命名的多版本里最新的一支
    if let Ok(dir) = bin_dir(app) {
        let legacy = dir.join(exe_name());
        if legacy.is_file() {
            return describe(legacy, FrpBinarySource::Downloaded, warning).await;
        }
        if let Some(versioned) = latest_versioned_exe(&dir).await {
            return describe(versioned, FrpBinarySource::Downloaded, warning).await;
        }
    }

    // 3) PATH
    if let Some(found) = which(exe_name()) {
        return describe(found, FrpBinarySource::Path, warning).await;
    }

    // 4) 常见位置
    if let Some(found) = common_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file())
    {
        return describe(found, FrpBinarySource::Common, warning).await;
    }

    FrpBinaryInfo {
        ok: false,
        path: None,
        version: None,
        source: None,
        error: Some(
            warning
                .unwrap_or_else(|| "未找到 frpc：请在设置里指定路径，或使用一键下载".to_string()),
        ),
    }
}

/// 探测 frpc（走设置项与自动查找）
pub(crate) async fn detect(app: &AppHandle) -> FrpBinaryInfo {
    detect_with(app, None).await
}
