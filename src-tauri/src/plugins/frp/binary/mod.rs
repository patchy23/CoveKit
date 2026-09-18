//! FRP 客户端获取入口与平台、下载源约定。

mod archive;
mod checksum;
mod detect;
mod download;
mod release;
#[cfg(test)]
mod tests;
use std::path::PathBuf;
use tauri::AppHandle;

/// 上游仓库（版本查询与下载的事实源）
pub(super) const REPO: &str = "fatedier/frp";

/// 查询版本超时（秒）
pub(super) const QUERY_TIMEOUT_SECS: u64 = 20;

/// 下载超时（秒）：整包约 12MB，慢链路给足余量
pub(super) const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

/// 下载进度事件（前端据此显示已下载字节）
pub(super) const EVENT_DOWNLOAD: &str = "frp://download";

/// 平台可执行文件名
pub(super) fn exe_name() -> &'static str {
    if cfg!(windows) {
        "frpc.exe"
    } else {
        "frpc"
    }
}

/// 当前平台标识（与上游 release 资产命名一致：windows / darwin / linux）
pub(super) fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        _ => "linux",
    }
}

/// 当前架构标识（与上游 release 资产命名一致：amd64 / arm64）
pub(super) fn arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// 工具内下载的 frpc 文件名（按版本号分文件，本地可并存多个版本以便档案各自绑定）
pub(super) fn versioned_exe_name(version: &str) -> String {
    // 版本号来自网络响应，过滤成安全字符，避免拼出路径分隔符
    let safe: String = version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    let safe = if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    };
    format!("frpc-{safe}{}", if cfg!(windows) { ".exe" } else { "" })
}

/// 压缩包内目录名（`tar` 解压后的一级目录）
pub(super) fn package_dir_name(version: &str) -> String {
    format!("frp_{version}_{}_{}", platform(), arch())
}

/// 上游资产名（Windows 为 zip，其它平台为 tar.gz）
pub(super) fn asset_name(version: &str) -> String {
    let base = package_dir_name(version);
    if cfg!(windows) {
        format!("{base}.zip")
    } else {
        format!("{base}.tar.gz")
    }
}

/// checksums 文件名（上游与版本无关，随 release 固定提供这一个）
///
/// 上游实际资产名是 `frp_sha256_checksums.txt`，文件内每行形如 `<sha256>  <资产名>`。
pub(super) fn checksums_name() -> &'static str {
    "frp_sha256_checksums.txt"
}

/// 工具设置读取（settings.json 的 `app.tools.frp.<key>`）
///
/// 2026-09-14 起只剩 `frpcPath` 一个读取方，且它是历史兼容读取：
/// 设置页已不再提供该配置项，新入口是客户端管理弹窗「引用外部文件」。
pub(super) fn setting(app: &AppHandle, key: &str) -> Option<String> {
    // 经框架读路径（合并设备层与空间层）：插件不得直读设置文件，否则换空间后读到错的一层
    crate::framework::settings::tool_setting(app, "frp", key)
}

/// 下载源前缀：直连 GitHub 官方优先，失败后依次回退到内置镜像。
///
/// 2026-09-14 起不再提供「下载镜像」设置项——能否访问 GitHub 取决于网络环境，
/// 属于应用该自己处理的问题（自动回退），而不是让用户去填前缀（填错只会更难排查）。
/// 三个镜像按本机实测可达性挑选（真实 release 资产 206 响应）。
pub(super) const DOWNLOAD_SOURCES: [&str; 4] = [
    "",
    "https://ghfast.top/",
    "https://ghproxy.net/",
    "https://gh.llkk.cc/",
];

/// 下载源可读名（错误信息用；空前缀表示直连官方）
pub(super) fn source_label(prefix: &str) -> &str {
    if prefix.is_empty() {
        "GitHub 官方"
    } else {
        prefix
    }
}

/// 按「优先源 → 其余源」的顺序排列下载源，用于同源兜底取 checksums
pub(super) fn sources_in_order(preferred: &str) -> Vec<&'static str> {
    let mut ordered: Vec<&'static str> = DOWNLOAD_SOURCES.to_vec();
    if let Some(index) = ordered.iter().position(|prefix| *prefix == preferred) {
        ordered.swap(0, index);
    }
    ordered
}

/// 本工具的 frpc 下载目录（`<存储根>/data/frp/bin`）
pub(crate) fn bin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = crate::framework::paths::data_dir(app)?
        .join("frp")
        .join("bin");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 frpc 目录失败（{}）：{e}", dir.display()))?;
    Ok(dir)
}

pub(crate) use detect::{detect, detect_with, probe_version};
pub(crate) use download::download;
pub(crate) use release::versions;
