//! FRP 客户端下载包的 SHA256 校验。

use super::asset_name;
use super::checksums_name;
use super::sources_in_order;
use super::QUERY_TIMEOUT_SECS;
use super::REPO;
use std::path::Path;
use std::time::Duration;

/// 计算文件 SHA256（16 进制小写）
fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| format!("读取下载文件失败：{e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// 强校验结论
pub(super) enum ChecksumStatus {
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
pub(super) struct VerifyOutcome {
    /// 校验结论，交由下载流程决定保留或丢弃文件。
    pub(super) status: ChecksumStatus,
    /// 本地压缩包哈希，供失败提示与人工核验使用。
    pub(super) archive_hash: String,
}

/// 从 checksums 文本里取指定资产的期望 SHA256
///
/// 上游每行是 `<sha256>  <资产名>`；这里按任意空白切分以兼容 CRLF 与多余空格，
/// 并只认 64 位十六进制的哈希，避免把格式异常的行当成有效期望值。
pub(super) fn parse_expected_hash(text: &str, asset: &str) -> Option<String> {
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
pub(super) fn judge_checksum(text: &str, asset: &str, archive_hash: &str) -> ChecksumStatus {
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
pub(super) async fn verify_checksum(
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
