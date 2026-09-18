//! FRP 下载与安装流程；失败清理和进度由此统一负责。

use super::archive::extract;
use super::asset_name;
use super::bin_dir;
use super::checksum::verify_checksum;
use super::checksum::ChecksumStatus;
use super::detect::describe;
use super::exe_name;
use super::package_dir_name;
use super::release::asset_size;
use super::source_label;
use super::versioned_exe_name;
use super::DOWNLOAD_SOURCES;
use super::DOWNLOAD_TIMEOUT_SECS;
use super::EVENT_DOWNLOAD;
use super::REPO;
use crate::plugins::frp::models::FrpBinaryInfo;
use crate::plugins::frp::models::FrpBinarySource;
use crate::plugins::frp::models::FrpDownloadPayload;
use crate::plugins::frp::models::FrpDownloadPhase;
use std::path::Path;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

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
