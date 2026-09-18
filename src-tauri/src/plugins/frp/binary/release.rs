//! FRP 上游版本与发行资产查询。

use super::asset_name;
use super::QUERY_TIMEOUT_SECS;
use super::REPO;
use crate::plugins::frp::models::FrpReleaseAsset;
use crate::plugins::frp::models::FrpReleaseInfo;
use std::time::Duration;

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
pub(super) async fn asset_size(version: &str) -> Option<u64> {
    let asset = asset_name(version);
    let releases = versions(20).await.ok()?;
    releases
        .into_iter()
        .find(|release| release.version == version)
        .and_then(|release| release.assets.into_iter().find(|item| item.name == asset))
        .map(|item| item.size)
        .filter(|size| *size > 0)
}
