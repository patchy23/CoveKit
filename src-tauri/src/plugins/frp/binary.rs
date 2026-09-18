//! frp 插件 · frpc 可执行文件（定位 / 版本查询 / 一键下载安装）
//!
//! 定位优先级：工具设置 `frpcPath` → 本工具下载目录 → PATH → 用户目录常见位置。
//! 下载走 GitHub Release（直连优先、内置镜像自动回退），并用 release 附带的
//! `frp_sha256_checksums.txt` 做 SHA256 强校验；
//! 解压复用系统工具（Windows `Expand-Archive`、其它平台 `tar`），不为此引入压缩库依赖——
//! 代价是必须绕开系统解压工具的扩展名与退出码坑，详见 `extract` 注释。
//! 下载中任一步失败都不改动已配置路径（调用方只在成功时记录新路径）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
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

/// 工具内下载的 frpc 文件名（按版本号分文件，本地可并存多个版本以便档案各自绑定）
fn versioned_exe_name(version: &str) -> String {
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

/// checksums 文件名（上游与版本无关，随 release 固定提供这一个）
///
/// 上游实际资产名是 `frp_sha256_checksums.txt`，文件内每行形如 `<sha256>  <资产名>`。
fn checksums_name() -> &'static str {
    "frp_sha256_checksums.txt"
}

/// 工具设置读取（settings.json 的 `app.tools.frp.<key>`）
///
/// 2026-09-14 起只剩 `frpcPath` 一个读取方，且它是历史兼容读取：
/// 设置页已不再提供该配置项，新入口是客户端管理弹窗「引用外部文件」。
fn setting(app: &AppHandle, key: &str) -> Option<String> {
    // 经框架读路径（合并设备层与空间层）：插件不得直读设置文件，否则换空间后读到错的一层
    crate::framework::settings::tool_setting(app, "frp", key)
}

/// 下载源前缀：直连 GitHub 官方优先，失败后依次回退到内置镜像。
///
/// 2026-09-14 起不再提供「下载镜像」设置项——能否访问 GitHub 取决于网络环境，
/// 属于应用该自己处理的问题（自动回退），而不是让用户去填前缀（填错只会更难排查）。
/// 三个镜像按本机实测可达性挑选（真实 release 资产 206 响应）。
const DOWNLOAD_SOURCES: [&str; 4] = [
    "",
    "https://ghfast.top/",
    "https://ghproxy.net/",
    "https://gh.llkk.cc/",
];

/// 下载源可读名（错误信息用；空前缀表示直连官方）
fn source_label(prefix: &str) -> &str {
    if prefix.is_empty() {
        "GitHub 官方"
    } else {
        prefix
    }
}

/// 按「优先源 → 其余源」的顺序排列下载源，用于同源兜底取 checksums
fn sources_in_order(preferred: &str) -> Vec<&'static str> {
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
fn version_key(version: &str) -> Vec<u32> {
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
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// 读取版本号：`frpc -v` 输出形如 `frpc version 0.71.0`；失败返回 None（不影响可用性判定）
pub(crate) async fn probe_version(exe: &Path) -> Option<String> {
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

/// 查询上游可用版本（GitHub Releases API；响应结构只取前端需要的字段）
pub(crate) async fn versions(limit: u32) -> Result<Vec<FrpReleaseInfo>, String> {
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
    // 版本列表只能走官方 API：几个常用镜像前缀都拒绝代理 api.github.com（实测 403），
    // 所以这里不做镜像回退，只在失败时把原因说清楚
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page={per_page}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;
    let response = client
        .get(&url)
        .header("User-Agent", "CoveKit")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("请求上游版本失败（请检查网络能否访问 api.github.com）：{e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "上游返回 {}（api.github.com 不可达或已触发限流）",
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
async fn asset_size(version: &str) -> Option<u64> {
    let asset = asset_name(version);
    let releases = versions(20).await.ok()?;
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

/// 强校验结论
enum ChecksumStatus {
    /// 取到期望值且与本地压缩包一致
    Verified,
    /// 取到期望值但与本地不一致：调用方必须丢弃下载文件
    Mismatch { expected: String, actual: String },
    /// 拿不到期望值，附可直接展示给用户的原因（与「不一致」严格区分）
    Unavailable(String),
}

/// 强校验结果：结论 + 本地压缩包实测 SHA256
///
/// 无论结论如何都带上实测值：拿不到期望值时它就是用户与上游
/// `frp_sha256_checksums.txt` 对照的唯一依据——上游列的是压缩包哈希，
/// 拿解压后可执行文件的哈希去比对没有意义。
struct VerifyOutcome {
    status: ChecksumStatus,
    archive_hash: String,
}

/// 从 checksums 文本里取指定资产的期望 SHA256
///
/// 上游每行是 `<sha256>  <资产名>`；这里按任意空白切分以兼容 CRLF 与多余空格，
/// 并只认 64 位十六进制的哈希，避免把格式异常的行当成有效期望值。
fn parse_expected_hash(text: &str, asset: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        if parts.next()? != asset {
            return None;
        }
        let valid = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        valid.then(|| hash.to_lowercase())
    })
}

/// 用 checksums 文本判定结论（纯函数，便于覆盖「一致 / 不一致 / 未列出」三种结果）
fn judge_checksum(text: &str, asset: &str, archive_hash: &str) -> ChecksumStatus {
    match parse_expected_hash(text, asset) {
        Some(expected) if expected == archive_hash => ChecksumStatus::Verified,
        Some(expected) => ChecksumStatus::Mismatch {
            expected,
            actual: archive_hash.to_string(),
        },
        None => ChecksumStatus::Unavailable(format!(
            "上游 checksums 未列出 {asset}（上游资产命名可能已变），未强校验"
        )),
    }
}

/// 强校验：取上游 checksums 并与本地压缩包比对
///
/// 取 checksums 与下载共用同一组下载源（下载成功的那个排最前）。三种结果分开报——
/// 通过、不一致（上层丢弃文件并报错）、拿不到期望值；后者还要区分「上游没有这个文件」
/// 与「网络取不到」，不能笼统写成「未强校验」。只有读取本地压缩包失败才返回 Err。
async fn verify_checksum(
    version: &str,
    archive: &Path,
    preferred: &str,
) -> Result<VerifyOutcome, String> {
    let asset = asset_name(version);
    let archive_hash = sha256_of(archive)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())?;
    let mut text: Option<String> = None;
    // 有源明确回 404：该版本确实没有这个文件，与「网络不通」是两回事
    let mut missing_upstream = false;
    for prefix in sources_in_order(preferred) {
        let url = format!(
            "{prefix}https://github.com/{REPO}/releases/download/v{version}/{}",
            checksums_name()
        );
        let Ok(response) = client
            .get(&url)
            .header("User-Agent", "CoveKit")
            .send()
            .await
        else {
            continue;
        };
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            missing_upstream = true;
            continue;
        }
        if !response.status().is_success() {
            continue;
        }
        if let Ok(body) = response.text().await {
            text = Some(body);
            break;
        }
    }
    let status = match text {
        Some(text) => judge_checksum(&text, &asset, &archive_hash),
        None if missing_upstream => ChecksumStatus::Unavailable(format!(
            "上游未提供 {}（版本 {version} 可能较旧），未强校验",
            checksums_name()
        )),
        None => ChecksumStatus::Unavailable(format!(
            "{} 取不到（网络不通或上游拒绝），未强校验",
            checksums_name()
        )),
    };
    Ok(VerifyOutcome {
        status,
        archive_hash,
    })
}

/// 从单个下载源抓取压缩包到 `archive`：成功返回（已下载字节，总大小）
///
/// `budget` 是本次尝试可用的时间额度（调用方按已耗时递减），超时即中断本次尝试；
/// 失败时保留已写入的半截文件，由调用方清理——只有调用方知道后面还有没有别的源要试。
async fn fetch_archive(
    app: &AppHandle,
    url: &str,
    archive: &Path,
    version: &str,
    budget: Duration,
) -> Result<(u64, Option<u64>), String> {
    let client = reqwest::Client::builder()
        .timeout(budget)
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;
    let mut response = client
        .get(url)
        .header("User-Agent", "CoveKit")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "上游返回 HTTP {}（版本 {version} 可能没有 {} 资产）",
            response.status().as_u16(),
            asset_name(version)
        ));
    }
    // 优先用响应头的 Content-Length；上游重定向后缺失时取 release 资产大小，
    // 保证进度条与「已下载 / 总大小」始终有分母（否则只能显示总大小未知）
    let total = match response.content_length() {
        Some(size) if size > 0 => Some(size),
        _ => asset_size(version).await,
    };
    // 首个进度事件要等响应头到达后再发：此时才知道总大小，否则会先闪一次「总大小未知」
    emit(
        app,
        version,
        FrpDownloadPhase::Download,
        Some(0),
        total,
        None,
    );
    let mut file = tokio::fs::File::create(archive)
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
    Ok((received, total))
}

/// 下载并安装指定版本的 frpc（失败不修改任何已配置路径）
pub(crate) async fn download(app: &AppHandle, version: &str) -> Result<FrpBinaryInfo, String> {
    let version = version.trim().trim_start_matches('v');
    if version.is_empty() {
        return Err("版本号为空".to_string());
    }
    let asset = asset_name(version);
    let bin = bin_dir(app)?;
    // 下载与解压共用一个临时目录：压缩包必须保持原始文件名（含 .zip/.tar.gz 扩展名），
    // 见 `extract` 注释——在文件名后追加 .tmp 会让两个系统解压器都拒绝处理
    let staging = bin.join("staging");
    let _ = tokio::fs::remove_dir_all(&staging).await;
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("创建下载临时目录失败（{}）：{e}", staging.display()))?;
    let archive = staging.join(&asset);

    // ── 下载（流式写盘，按块推送进度）──
    // 逐个下载源尝试：直连失败自动走内置镜像。每次失败先删掉半截文件，
    // 否则下一个源会接着往残包里写，最后报「校验失败」而不是「下载失败」，把原因指偏。
    // 换源共享同一份总时间额度（默认 5 分钟）：源越多越不能各自跑满一份超时，
    // 否则网络黑洞下用户要等 20 分钟才看到失败。
    let mut budget = Duration::from_secs(DOWNLOAD_TIMEOUT_SECS);
    let mut picked: Option<(&'static str, u64, Option<u64>)> = None;
    let mut failures: Vec<String> = Vec::new();
    for prefix in DOWNLOAD_SOURCES {
        if budget.is_zero() {
            failures.push("已用完本次下载的时间额度".to_string());
            break;
        }
        let url = format!("{prefix}https://github.com/{REPO}/releases/download/v{version}/{asset}");
        let started = std::time::Instant::now();
        match fetch_archive(app, &url, &archive, version, budget).await {
            Ok((received, total)) => {
                picked = Some((prefix, received, total));
                break;
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&archive).await;
                failures.push(format!("{}：{error}", source_label(prefix)));
            }
        }
        budget = budget.saturating_sub(started.elapsed());
    }
    let Some((source, received, total)) = picked else {
        return Err(format!(
            "下载失败，已尝试 {} 个源（{}）",
            failures.len(),
            failures.join("；")
        ));
    };

    // ── 校验（上游提供 checksums 时强校验）──
    emit(
        app,
        version,
        FrpDownloadPhase::Verify,
        Some(received),
        total,
        None,
    );
    let outcome = verify_checksum(version, &archive, source).await?;
    if let ChecksumStatus::Mismatch { expected, actual } = &outcome.status {
        let _ = tokio::fs::remove_file(&archive).await;
        return Err(format!(
            "SHA256 校验不通过，已丢弃下载文件（来源 {}）：期望 {expected}，实际 {actual}；可重试或稍后再试",
            source_label(source)
        ));
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
    let target = bin.join(versioned_exe_name(version));
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
    let mut info = describe(target.clone(), FrpBinarySource::Downloaded, None).await;
    if let ChecksumStatus::Unavailable(reason) = &outcome.status {
        info.error = Some(format!(
            "{reason}；仅校验了解压完整性。压缩包 SHA256：{}",
            outcome.archive_hash
        ));
    }
    // 登记到客户端清单：下载完即可被档案绑定。登记失败不阻断下载结果
    // （文件已就位，用户也可以在客户端管理里手动引用该路径）。
    let _ = crate::plugins::frp::clients::add_downloaded(app, &target).await;
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
        let dir = std::env::temp_dir().join("covekit-frp-extract-ok");
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
        let dir = std::env::temp_dir().join("covekit-frp-extract-broken");
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
    fn version_key_orders_numerically() {
        // 字符串序会判错：`0.9.0` 按字符串大于 `0.71.0`，按数值则相反
        assert!(version_key("0.71.0") > version_key("0.9.0"));
        assert!(version_key("0.71.0") > version_key("0.70.9"));
        assert_eq!(version_key("0.71.0"), vec![0, 71, 0]);
        // 带预发布后缀时非数字段按 0 处理，不 panic（rc1 解析失败记为 0）
        assert_eq!(version_key("0.71.0-rc1"), vec![0, 71, 0, 0]);
    }

    #[test]
    fn download_sources_try_direct_first_and_hoist_the_working_one() {
        // 直连排首位：能直连就不该绕镜像
        assert_eq!(DOWNLOAD_SOURCES[0], "");
        assert_eq!(source_label(""), "GitHub 官方");
        // 已成功的源提到最前（同源取 checksums），其余顺序不变
        let ordered = sources_in_order("https://ghproxy.net/");
        assert_eq!(ordered[0], "https://ghproxy.net/");
        assert_eq!(ordered.len(), DOWNLOAD_SOURCES.len());
        // 未知来源不改动顺序、不 panic
        assert_eq!(sources_in_order("https://example.invalid/")[0], "");
    }

    #[test]
    fn which_finds_existing_file_on_path() {
        // 用系统上必然存在的可执行文件探测（windows: cmd.exe，其它: sh）
        let probe = which(if cfg!(windows) { "cmd.exe" } else { "sh" });
        assert!(probe.is_some());
    }

    /// checksums 资产名固定，不能按版本拼
    ///
    /// 回归「强校验从未生效」：代码曾请求 `frp_{version}_checksums.txt`，而上游实际
    /// 提供的是固定的 `frp_sha256_checksums.txt`（v0.44 起，更早版本没有这个文件），
    /// 于是每次下载都拿到 404、静默退化成「仅校验解压完整性」。
    #[test]
    fn checksums_name_matches_upstream_asset() {
        assert_eq!(checksums_name(), "frp_sha256_checksums.txt");
    }

    /// 解析上游 checksums：CRLF、Tab 分隔、大写哈希、注释行与格式异常行都要处理对
    #[test]
    fn parse_expected_hash_reads_upstream_format() {
        let text = "a872a46b08ff971462f311dce3d9b3c538f3c130ed7cdee3ea75b6728b9f5d3c  frp_0.70.1_android_arm64.tar.gz\r\n\
                    CBF69CF26E5553E914E97D37F5D4367FA30F5F531D073A889465AF4719281E25\tfrp_0.70.1_darwin_amd64.tar.gz\n";
        let asset = "frp_0.70.1_darwin_amd64.tar.gz";
        assert_eq!(
            parse_expected_hash(text, asset).as_deref(),
            Some("cbf69cf26e5553e914e97d37f5d4367fa30f5f531d073a889465af4719281e25")
        );
        // 未列出的资产、注释行、长度不足的哈希都不认作期望值
        assert_eq!(
            parse_expected_hash(text, "frp_0.70.1_windows_amd64.zip"),
            None
        );
        assert_eq!(parse_expected_hash("# 说明行 frp_x.zip", "frp_x.zip"), None);
        assert_eq!(
            parse_expected_hash("deadbeef  frp_x.zip", "frp_x.zip"),
            None
        );
    }

    /// 「不一致」必须与「拿不到期望值」分开，且原因要能指明具体资产
    #[test]
    fn judge_checksum_separates_mismatch_from_unavailable() {
        let hash = "cbf69cf26e5553e914e97d37f5d4367fa30f5f531d073a889465af4719281e25";
        let other = "0".repeat(64);
        let asset = "frp_0.70.1_darwin_amd64.tar.gz";
        let text = format!("{hash}  {asset}\n");

        assert!(matches!(
            judge_checksum(&text, asset, hash),
            ChecksumStatus::Verified
        ));

        match judge_checksum(&text, asset, &other) {
            ChecksumStatus::Mismatch { expected, actual } => {
                assert_eq!(expected, hash);
                assert_eq!(actual, other);
            }
            _ => panic!("哈希不符必须判 Mismatch"),
        }

        // 资产未列在 checksums 里：是「拿不到期望值」，不是「校验失败」
        match judge_checksum(&text, "frp_0.70.1_windows_amd64.zip", hash) {
            ChecksumStatus::Unavailable(reason) => {
                assert!(
                    reason.contains("frp_0.70.1_windows_amd64.zip"),
                    "原因要指明具体资产：{reason}"
                );
            }
            _ => panic!("未列出应判 Unavailable"),
        }
    }
}
