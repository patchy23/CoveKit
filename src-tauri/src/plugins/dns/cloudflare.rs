//! Cloudflare DNS API v4 Adapter
//! 使用推荐的 Bearer API Token；Zone 与 DNS Record 均通过官方 REST API 管理。

use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::models::{CloudflareConfig, DnsRecord, Domain, DomainList, RecordList};

const API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare DNS 客户端。
pub struct CloudflareDns {
    /// 复用连接池的 HTTP 客户端
    client: Client,
    /// Bearer API Token
    token: String,
}

impl CloudflareDns {
    /// 由 API Token 配置构造客户端。
    pub fn new(cfg: &CloudflareConfig) -> Result<Self, String> {
        let token = cfg.token.trim();
        if token.is_empty() {
            return Err("请先在「密钥设置」页配置 Cloudflare API Token".into());
        }
        Ok(Self {
            client: Client::new(),
            token: token.to_string(),
        })
    }

    /// 域名列表。Cloudflare Zone 列表分页拉取，避免静默漏掉超过 50 个 Zone 的账号。
    pub async fn get_domains(&self) -> Result<DomainList, String> {
        let mut page = 1_u32;
        let mut zones = Vec::new();
        let mut total = 0_u32;
        loop {
            let request = self
                .authorized(self.client.get(format!("{API_BASE}/zones")))
                .query(&[("page", page), ("per_page", 50)]);
            let (mut batch, info): (Vec<Zone>, _) = self.send(request).await?;
            total = info.as_ref().map_or(total, |value| value.total_count);
            zones.append(&mut batch);
            if page >= info.as_ref().map_or(page, |value| value.total_pages.max(1)) {
                break;
            }
            page += 1;
        }

        if total == 0 {
            total = zones.len() as u32;
        }
        Ok(DomainList {
            list: zones
                .into_iter()
                .map(|zone| Domain {
                    domain_id: zone.id,
                    domain_name: zone.name,
                    // Zone 列表不返回记录数；进入域名后由记录列表返回准确总数。
                    record_total: 0,
                    platform: super::models::PLATFORM_CLOUDFLARE.into(),
                    create_time: zone.created_on,
                })
                .collect(),
            total,
        })
    }

    /// 解析记录列表；Cloudflare 的 `search` 参数用于当前 UI 的人类模糊搜索场景。
    pub async fn get_records(
        &self,
        domain: &str,
        page: u32,
        size: u32,
        keyword: &str,
    ) -> Result<RecordList, String> {
        let zone_id = self.zone_id(domain).await?;
        let mut request = self
            .authorized(
                self.client
                    .get(format!("{API_BASE}/zones/{zone_id}/dns_records")),
            )
            .query(&[
                ("page", page.max(1).to_string()),
                ("per_page", size.to_string()),
            ]);
        if !keyword.trim().is_empty() {
            request = request.query(&[("search", keyword.trim())]);
        }
        let (records, info): (Vec<ApiRecord>, _) = self.send(request).await?;
        let total = info.map_or(records.len() as u32, |value| value.total_count);
        Ok(RecordList {
            list: records
                .into_iter()
                .map(|record| DnsRecord {
                    record_id: record.id,
                    rr: name_to_rr(&record.name, domain),
                    record_type: record.record_type,
                    ttl: record.ttl,
                    value: record.content,
                    line: if record.proxied.unwrap_or(false) {
                        "已代理".into()
                    } else {
                        "仅 DNS".into()
                    },
                })
                .collect(),
            total,
        })
    }

    /// 添加解析记录。
    pub async fn add_record(
        &self,
        domain: &str,
        rr: &str,
        rtype: &str,
        value: &str,
        ttl: u32,
    ) -> Result<(), String> {
        let zone_id = self.zone_id(domain).await?;
        let request = self
            .authorized(
                self.client
                    .post(format!("{API_BASE}/zones/{zone_id}/dns_records")),
            )
            .json(&serde_json::json!({
                "type": rtype,
                "name": rr_to_name(rr, domain),
                "content": value,
                "ttl": ttl,
                "proxied": false
            }));
        self.send::<serde_json::Value>(request).await?;
        Ok(())
    }

    /// 更新解析记录（PATCH 只更新当前表单中的通用字段）。
    pub async fn update_record(
        &self,
        domain: &str,
        record_id: &str,
        rr: &str,
        rtype: &str,
        value: &str,
        ttl: u32,
    ) -> Result<(), String> {
        let zone_id = self.zone_id(domain).await?;
        let request = self
            .authorized(self.client.patch(format!(
                "{API_BASE}/zones/{zone_id}/dns_records/{record_id}"
            )))
            .json(&serde_json::json!({
                "type": rtype,
                "name": rr_to_name(rr, domain),
                "content": value,
                "ttl": ttl
            }));
        self.send::<serde_json::Value>(request).await?;
        Ok(())
    }

    /// 删除解析记录。
    pub async fn delete_record(&self, domain: &str, record_id: &str) -> Result<(), String> {
        let zone_id = self.zone_id(domain).await?;
        let request = self.authorized(self.client.delete(format!(
            "{API_BASE}/zones/{zone_id}/dns_records/{record_id}"
        )));
        self.send::<serde_json::Value>(request).await?;
        Ok(())
    }

    /// 用精确 Zone 名称换取 Zone ID。
    async fn zone_id(&self, domain: &str) -> Result<String, String> {
        let request = self
            .authorized(self.client.get(format!("{API_BASE}/zones")))
            .query(&[("name", domain.trim()), ("per_page", "1")]);
        let (zones, _): (Vec<Zone>, _) = self.send(request).await?;
        zones
            .into_iter()
            .find(|zone| zone.name.eq_ignore_ascii_case(domain.trim()))
            .map(|zone| zone.id)
            .ok_or_else(|| format!("Cloudflare 未找到 Zone: {}", domain.trim()))
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        request.bearer_auth(&self.token)
    }

    /// 统一解析 Cloudflare envelope，让 HTTP 与业务错误都带回明确消息。
    async fn send<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<(T, Option<ResultInfo>), String> {
        let response = request
            .send()
            .await
            .map_err(|error| format!("Cloudflare 请求失败: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("Cloudflare 响应读取失败: {error}"))?;
        let envelope: ApiEnvelope<T> = serde_json::from_str(&body)
            .map_err(|error| format!("Cloudflare 响应解析失败（HTTP {status}）: {error}"))?;
        if !status.is_success() || !envelope.success {
            let detail = envelope
                .errors
                .iter()
                .map(|item| format!("{}: {}", item.code, item.message))
                .collect::<Vec<_>>()
                .join("；");
            return Err(if detail.is_empty() {
                format!("Cloudflare 请求失败（HTTP {status}）")
            } else {
                format!("Cloudflare 请求失败: {detail}")
            });
        }
        envelope
            .result
            .map(|result| (result, envelope.result_info))
            .ok_or_else(|| "Cloudflare 成功响应缺少 result".into())
    }
}

#[derive(Deserialize)]
/// Cloudflare API v4 统一响应包。
struct ApiEnvelope<T> {
    /// 业务请求是否成功
    success: bool,
    /// 成功时的业务结果
    result: Option<T>,
    /// 失败时的错误列表
    #[serde(default)]
    errors: Vec<ApiError>,
    /// 列表接口分页信息
    result_info: Option<ResultInfo>,
}

#[derive(Deserialize)]
/// Cloudflare 业务错误。
struct ApiError {
    /// 平台错误码
    code: u32,
    /// 平台错误消息
    message: String,
}

#[derive(Deserialize)]
/// 列表接口分页信息。
struct ResultInfo {
    /// 匹配结果总数
    #[serde(default)]
    total_count: u32,
    /// 总页数
    #[serde(default)]
    total_pages: u32,
}

#[derive(Deserialize)]
/// Cloudflare Zone 摘要。
struct Zone {
    /// Zone ID
    id: String,
    /// Zone 域名
    name: String,
    /// 创建时间
    #[serde(default)]
    created_on: String,
}

#[derive(Deserialize)]
/// Cloudflare DNS Record 通用字段。
struct ApiRecord {
    /// 记录 ID
    id: String,
    /// 完整记录名
    name: String,
    /// DNS 记录类型
    #[serde(rename = "type")]
    record_type: String,
    /// TTL 秒数；1 表示自动
    ttl: u32,
    /// 记录值
    content: String,
    /// 是否启用 Cloudflare 代理
    proxied: Option<bool>,
}

/// 将 UI 主机记录转换为 Cloudflare 所需的完整记录名。
fn rr_to_name(rr: &str, domain: &str) -> String {
    let rr = rr.trim().trim_end_matches('.');
    let domain = domain.trim().trim_end_matches('.');
    if rr.is_empty() || rr == "@" || rr.eq_ignore_ascii_case(domain) {
        domain.to_string()
    } else if rr
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", domain.to_ascii_lowercase()))
    {
        rr.to_string()
    } else {
        format!("{rr}.{domain}")
    }
}

/// 将 Cloudflare 完整记录名转换为 UI 主机记录，根域名显示为 @。
fn name_to_rr(name: &str, domain: &str) -> String {
    let name = name.trim_end_matches('.');
    let domain = domain.trim().trim_end_matches('.');
    if name.eq_ignore_ascii_case(domain) {
        return "@".into();
    }
    let suffix = format!(".{}", domain.to_ascii_lowercase());
    if name.to_ascii_lowercase().ends_with(&suffix) {
        return name[..name.len() - suffix.len()].to_string();
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::{name_to_rr, rr_to_name, ApiEnvelope};

    #[test]
    fn record_names_roundtrip_root_and_subdomain() {
        assert_eq!(rr_to_name("@", "example.com"), "example.com");
        assert_eq!(rr_to_name("www", "example.com"), "www.example.com");
        assert_eq!(name_to_rr("example.com", "example.com"), "@");
        assert_eq!(name_to_rr("WWW.Example.com", "example.com"), "WWW");
    }

    #[test]
    fn error_envelope_can_be_parsed_without_result() {
        let parsed: ApiEnvelope<serde_json::Value> = serde_json::from_str(
            r#"{"success":false,"errors":[{"code":9109,"message":"Invalid access token"}],"result":null}"#,
        )
        .unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.errors[0].code, 9109);
    }
}
