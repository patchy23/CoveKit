//! frp 插件 · frpc 可执行文件（定位 / 版本查询 / 一键下载安装）
//!
//! 定位优先级：工具设置 `frpcPath` → 本工具下载目录 → PATH → 用户目录常见位置。
//! 下载走 GitHub Release（可配镜像前缀），优先用 release 附带的 checksums 文件做 SHA256 校验；
//! 解压复用系统工具（Windows `Expand-Archive`、其它平台 `tar`），不为此引入压缩库依赖——
//! 代价是必须绕开系统解压工具的扩展名与退出码坑，详见 `extract` 注释。
//! 下载中任一步失败都不改动已配置路径（调用方只在成功时记录新路径）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::plugins::frp::models::{
    FrpBinaryInfo, FrpBinarySource, FrpDownloadPayload, FrpDownloadPhase, FrpReleaseAsset,
    FrpReleaseInfo,
};

/// 上游仓库（版本查询与下载的事实源）
const REPO: &str = "fatedier/frp";
/// 查询版本超时（秒）
const QUERY_TIMEOUT_SECS: u64 = 20;
/// 下载超时（秒）：整包约 12MB，慢链路给足余量
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
/// 下载进度事件（前端据此显示已下载字节）
const EVENT_DOWNLOAD: &str = "frp://download";

/// 平台可执行文件名
fn exe_name() -> &'static str {
    if cfg!(windows) {
        "frpc.exe"
    } else {
        "frpc"
    }
}

/// 当前平台标识（与上游 release 资产命名一致：windows / darwin / linux）
fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        _ => "linux",
    }
}

/// 当前架构标识（与上游 release 资产命名一致：amd64 / arm64）
fn arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// 压缩包内目录名（`tar` 解压后的一级目录）
fn package_dir_name(version: &str) -> String {
    format!("frp_{version}_{}_{}", platform(), arch())
}

/// 上游资产名（Windows 为 zip，其它平台为 tar.gz）
fn asset_name(version: &str) -> String {
    let base = package_dir_name(version);
    if cfg!(windows) {
        format!("{base}.zip")
    } else {
        format!("{base}.tar.gz")
    }
}

/// checksums 文件名（上游随 release 提供时用于 SHA256 强校验）
fn checksums_name(version: &str) -> String {
    format!("frp_{version}_checksums.txt")
}

/// 工具设置读取（settings.json 的 `app.tools.frp.<key>`）
fn setting(app: &AppHandle, key: &str) -> Option<String> {
    let store = app.store("settings.json").ok()?;
    let app_config = store.get("app")?;
    let value = app_config.get("tools")?.get("frp")?.get(key)?;
    value.as_str().map(String::from)
}

/// 镜像前缀（可配置；非空时统一补一个 `/`，用于拼在原始 URL 前）
fn mirror(app: &AppHandle) -> String {
    let raw = setting(app, "downloadMirror").unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
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

/// 在 PATH 中查找可执行文件（逐目录探测，不调用外部 which）
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// 读取版本号：`frpc -v` 输出形如 `frpc version 0.71.0`；失败返回 None（不影响可用性判定）
async fn probe_version(exe: &Path) -> Option<String> {
    let output = Command::new(exe).arg("-v").output().await.ok()?;
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
async fn describe(
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

    // 2) 本工具下载目录
    if let Ok(dir) = bin_dir(app) {
        let candidate = dir.join(exe_name());
        if candidate.is_file() {
            return describe(candidate, FrpBinarySource::Downloaded, warning).await;
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

/// 查询上游可用版本（GitHub Releases API；响应结构只取前端需要的字段）
pub(crate) async fn versions(app: &AppHandle, limit: u32) -> Result<Vec<FrpReleaseInfo>, String> {
    /// Releases API 响应的最小字段集
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
        published_at: Option<String>,
        #[serde(default)]
        assets: Vec<Asset>,
    }
    /// 资产的最小字段集
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        size: Option<u64>,
    }

    let per_page = limit.clamp(1, 50);
    let url = format!(
        "{}https://api.github.com/repos/{REPO}/releases?per_page={per_page}",
        mirror(app)
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;
    let response = client
        .get(&url)
        .header("User-Agent", "patchyBox")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("请求上游版本失败（可尝试配置下载镜像）：{e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "上游返回 {}（可尝试配置下载镜像）",
            response.status().as_u16()
        ));
    }
    let releases: Vec<Release> = response
        .json()
        .await
        .map_err(|e| format!("解析版本列表失败：{e}"))?;
    Ok(releases
        .into_iter()
        .map(|release| FrpReleaseInfo {
            version: release.tag_name.trim_start_matches('v').to_string(),
            published_at: release.published_at.unwrap_or_default(),
            assets: release
                .assets
                .into_iter()
                .map(|asset| FrpReleaseAsset {
                    name: asset.name,
                    size: asset.size.unwrap_or(0),
                })
                .collect(),
        })
        .collect())
}

/// 查询上游某版本的资产字节数
///
/// GitHub 的下载地址会 302 到 objects.githubusercontent.com，重定向后的响应是分块传输、
/// **不带 `Content-Length`**（实测 `response.content_length()` 为 None），此时前端只能显示
/// 「总大小未知」。release API 的 `assets[].size` 是该资产的权威大小，用它兜底。
async fn asset_size(app: &AppHandle, version: &str) -> Option<u64> {
    let asset = asset_name(version);
    let releases = versions(app, 20).await.ok()?;
    releases
        .into_iter()
        .find(|release| release.version == version)
        .and_then(|release| release.assets.into_iter().find(|item| item.name == asset))
        .map(|item| item.size)
        .filter(|size| *size > 0)
}

/// 推送下载进度
fn emit(
    app: &AppHandle,
    version: &str,
    phase: FrpDownloadPhase,
    received: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
) {
    let payload = FrpDownloadPayload {
        version: version.to_string(),
        phase,
        received,
        total,
        error,
    };
    let _ = app.emit(EVENT_DOWNLOAD, payload);
}

/// 命令行路径参数（按原样传参，交给 OS 处理；仅非 Windows 的 tar 分支使用）
#[cfg(not(windows))]
fn arg_str(path: &Path) -> String {
    path.display().to_string()
}

/// PowerShell 单引号字面量（字符串内的单引号需转义成两个，否则路径含引号时脚本被截断）
#[cfg(windows)]
fn ps_literal(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

/// 目标目录是否已产出内容（系统解压工具退出码不可信时的兜底判据）
async fn has_entries(dir: &Path) -> bool {
    match tokio::fs::read_dir(dir).await {
        Ok(mut entries) => entries.next_entry().await.ok().flatten().is_some(),
        Err(_) => false,
    }
}

/// 从命令输出里取一段可读原因（stderr 优先；中文系统上 PowerShell 的中文报错会因 GBK
/// 编码显示为乱码，但其中的英文错误标识仍可辨认，比笼统的「解压失败」有用）
fn output_tail(stderr: &[u8], stdout: &[u8]) -> String {
    let pick = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let from_stderr = pick(stderr);
    if from_stderr.is_empty() {
        pick(stdout)
    } else {
        from_stderr
    }
}

/// 解压压缩包到目标目录（复用系统工具，避免为一次性操作引入压缩库依赖）
///
/// 三个坑都是实测踩到才发现的，改动本函数前先读（Windows 分支）：
/// 1) **压缩包必须保留 `.zip` 扩展名**：PowerShell 5.1 的 `Expand-Archive` 对 `.zip.tmp`
///    这类后缀直接报 NotSupportedArchiveFileExtension，内容都不看就拒绝；
/// 2) **不要按名字调用 `tar` 解 zip**：PATH 上的 `tar` 可能是 MSYS / GNU tar（本机实测
///    GNU tar 1.35），GNU tar 不支持 zip，还会把 `C:\` 当远程主机（`Cannot connect to C:`）；
///    只有 Windows 自带的 bsdtar 支持 zip，按名字调用等于把成败押在用户机器的 PATH 顺序上；
/// 3) `Expand-Archive` 遇到坏包时**写 stderr 但退出码仍为 0**，必须用 `try/catch + exit 1`
///    才能拿到非零退出码，并且额外校验目录确实产出了内容，否则会把「什么都没解出来」当成功。
#[cfg(windows)]
async fn extract(archive: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建解压目录失败：{e}"))?;
    let script = format!(
        "try {{ Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force -ErrorAction Stop }} \
         catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}",
        ps_literal(archive),
        ps_literal(dir)
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await
        .map_err(|e| format!("调用系统解压工具失败：{e}"))?;
    if output.status.success() && has_entries(dir).await {
        return Ok(());
    }
    let detail = output_tail(&output.stderr, &output.stdout);
    if detail.is_empty() {
        Err("解压失败：系统解压工具未能解开该压缩包".to_string())
    } else {
        Err(format!("解压失败：{detail}"))
    }
}

/// 解压压缩包到目标目录（其它平台：系统 tar 原生支持 tar.gz）
#[cfg(not(windows))]
async fn extract(archive: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建解压目录失败：{e}"))?;
    let output = Command::new("tar")
        .args(["-xzf", &arg_str(archive), "-C", &arg_str(dir)])
        .output()
        .await
        .map_err(|e| format!("调用系统解压工具失败：{e}"))?;
    if output.status.success() && has_entries(dir).await {
        return Ok(());
    }
    let detail = output_tail(&output.stderr, &output.stdout);
    if detail.is_empty() {
        Err("解压失败：系统 tar 无法解开该压缩包".to_string())
    } else {
        Err(format!("解压失败：{detail}"))
    }
}

/// 计算文件 SHA256（16 进制小写）
fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| format!("读取下载文件失败：{e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// 尝试强校验：从上游 checksums 文件取出该资产的期望值并比对；上游未提供时返回 None（调用方提示未强校验）
async fn verify_checksum(
    app: &AppHandle,
    version: &str,
    archive: &Path,
) -> Result<Option<bool>, String> {
    let asset = asset_name(version);
    let url = format!(
        "{}https://github.com/{REPO}/releases/download/v{version}/{}",
        mirror(app),
        checksums_name(version)
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())?;
    let response = match client
        .get(&url)
        .header("User-Agent", "patchyBox")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let text = response.text().await.map_err(|e| e.to_string())?;
    let expected = text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?;
        if name == asset {
            Some(hash.to_lowercase())
        } else {
            None
        }
    });
    match expected {
        Some(hash) => Ok(Some(hash == sha256_of(archive)?)),
        None => Ok(None),
    }
}

/// 下载并安装指定版本的 frpc（失败不修改任何已配置路径）
pub(crate) async fn download(app: &AppHandle, version: &str) -> Result<FrpBinaryInfo, String> {
    let version = version.trim().trim_start_matches('v');
    if version.is_empty() {
        return Err("版本号为空".to_string());
    }
    let asset = asset_name(version);
    let url = format!(
        "{}https://github.com/{REPO}/releases/download/v{version}/{asset}",
        mirror(app)
    );
    let bin = bin_dir(app)?;
    // 下载与解压共用一个临时目录：压缩包必须保持原始文件名（含 .zip/.tar.gz 扩展名），
    // 见 `extract` 注释——在文件名后追加 .tmp 会让两个系统解压器都拒绝处理
    let staging = bin.join("staging");
    let _ = tokio::fs::remove_dir_all(&staging).await;
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("创建下载临时目录失败（{}）：{e}", staging.display()))?;
    let archive = staging.join(&asset);

    // ── 下载（流式写盘，按块推送进度）──
    // 首个进度事件要等响应头到达后再发：此时才知道总大小，否则会先闪一次「总大小未知」
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;
    let mut response = client
        .get(&url)
        .header("User-Agent", "patchyBox")
        .send()
        .await
        .map_err(|e| format!("下载失败（可尝试配置下载镜像）：{e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "下载失败：上游返回 {}（版本 {version} 可能没有 {} 资产）",
            response.status().as_u16(),
            asset
        ));
    }
    // 优先用响应头的 Content-Length；上游重定向后缺失时取 release 资产大小，
    // 保证进度条与「已下载 / 总大小」始终有分母（否则只能显示总大小未知）
    let total = match response.content_length() {
        Some(size) if size > 0 => Some(size),
        _ => asset_size(app, version).await,
    };
    emit(
        app,
        version,
        FrpDownloadPhase::Download,
        Some(0),
        total,
        None,
    );
    let mut file = tokio::fs::File::create(&archive)
        .await
        .map_err(|e| format!("创建临时文件失败（{}）：{e}", archive.display()))?;
    let mut received: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("读取下载数据失败：{e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入临时文件失败：{e}"))?;
        received += u64::try_from(chunk.len()).unwrap_or(0);
        emit(
            app,
            version,
            FrpDownloadPhase::Download,
            Some(received),
            total,
            None,
        );
    }
    file.flush()
        .await
        .map_err(|e| format!("写入临时文件失败：{e}"))?;
    drop(file);

    // ── 校验（上游提供 checksums 时强校验）──
    emit(
        app,
        version,
        FrpDownloadPhase::Verify,
        Some(received),
        total,
        None,
    );
    let checked = verify_checksum(app, version, &archive).await?;
    if checked == Some(false) {
        let _ = tokio::fs::remove_file(&archive).await;
        return Err("SHA256 校验失败，已丢弃下载文件（可换镜像重试）".to_string());
    }

    // ── 解压并取出 frpc ──
    emit(
        app,
        version,
        FrpDownloadPhase::Extract,
        Some(received),
        total,
        None,
    );
    // 解压目标必须是压缩包所在的子目录：若直接解到 staging，压缩包自身就会让
    // `has_entries` 成立，退出码不可信的问题又绕回来了
    let unpack = staging.join("unpack");
    if let Err(error) = extract(&archive, &unpack).await {
        // 失败即清理：整包约 14MB，留着既占空间又会让用户误以为已经装好
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    let from = unpack.join(package_dir_name(version)).join(exe_name());
    if !from.is_file() {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(format!(
            "解压后未找到 {}（压缩包结构可能已变，请手动解压后指定路径）",
            exe_name()
        ));
    }
    let target = bin.join(exe_name());
    tokio::fs::rename(&from, &target)
        .await
        .or_else(|_| std::fs::copy(&from, &target).map(|_| ()))
        .map_err(|e| format!("安装 frpc 失败（{}）：{e}", target.display()))?;
    // 非 Windows 平台补可执行位（下载通常不带 +x）
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&target) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target, perms);
        }
    }
    let _ = tokio::fs::remove_file(&archive).await;
    let _ = tokio::fs::remove_dir_all(&staging).await;

    emit(
        app,
        version,
        FrpDownloadPhase::Done,
        Some(received),
        total,
        None,
    );
    let mut info = describe(target, FrpBinarySource::Downloaded, None).await;
    if checked.is_none() {
        info.error = Some(format!(
            "上游未提供 checksums，仅校验了解压完整性；本地 SHA256：{}",
            sha256_of(&bin.join(exe_name())).unwrap_or_else(|_| "计算失败".to_string())
        ));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_follows_upstream_convention() {
        let name = asset_name("0.71.0");
        assert!(name.starts_with("frp_0.71.0_"));
        assert!(name.ends_with(if cfg!(windows) { ".zip" } else { ".tar.gz" }));
    }

    #[test]
    fn platform_and_arch_map_to_upstream_tokens() {
        assert!(["windows", "darwin", "linux"].contains(&platform()));
        assert!(["amd64", "arm64"].contains(&arch()));
    }

    #[test]
    fn package_dir_matches_asset_stem() {
        let dir = package_dir_name("0.71.0");
        let asset = asset_name("0.71.0");
        assert!(asset.starts_with(&dir));
    }

    /// 解压必须能真正解开 zip
    ///
    /// 回归「下载完成却报解压失败」：下载文件曾命名为 `.zip.tmp`，PowerShell 的
    /// `Expand-Archive` 只看后缀就拒绝（NotSupportedArchiveFileExtension），而下载本身
    /// 是完整的——测试同时守住「后缀合法」与「确实解出文件」两点。
    #[cfg(windows)]
    #[tokio::test]
    async fn extract_unpacks_real_zip() {
        let dir = std::env::temp_dir().join("patchybox-frp-extract-ok");
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::create_dir_all(&dir).await.expect("建临时目录");
        let source = dir.join("payload");
        tokio::fs::create_dir_all(&source).await.expect("建源目录");
        tokio::fs::write(source.join("frpc.txt"), b"frpc")
            .await
            .expect("写样例文件");
        let archive = dir.join("sample.zip");
        let script = format!(
            "Compress-Archive -Path '{}' -DestinationPath '{}' -Force",
            ps_literal(&source.join("*")),
            ps_literal(&archive)
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .await
            .expect("调用 PowerShell 造 zip");
        assert!(status.success(), "造测试压缩包失败");

        let out = dir.join("out");
        extract(&archive, &out).await.expect("解压应成功");
        assert!(out.join("frpc.txt").is_file(), "解压后应产出文件");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    /// 坏包不能假成功
    ///
    /// 回归第二重坑：`Expand-Archive` 遇到坏包时只写 stderr、**退出码仍是 0**，
    /// 只看 `status.success()` 会把「什么都没解出来」当成安装成功。
    #[cfg(windows)]
    #[tokio::test]
    async fn extract_rejects_broken_archive() {
        let dir = std::env::temp_dir().join("patchybox-frp-extract-broken");
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::create_dir_all(&dir).await.expect("建临时目录");
        let archive = dir.join("broken.zip");
        tokio::fs::write(&archive, b"this is definitely not a zip archive")
            .await
            .expect("写坏包");

        let out = dir.join("out");
        let result = extract(&archive, &out).await;
        assert!(result.is_err(), "坏包必须报错，不能静默成功");
        assert!(!has_entries(&out).await, "坏包不应产出任何文件");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn which_finds_existing_file_on_path() {
        // 用系统上必然存在的可执行文件探测（windows: cmd.exe，其它: sh）
        let probe = which(if cfg!(windows) { "cmd.exe" } else { "sh" });
        assert!(probe.is_some());
    }
}
