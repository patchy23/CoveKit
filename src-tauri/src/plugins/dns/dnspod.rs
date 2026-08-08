//! 腾讯云 DNSPod 云解析实现（API 3.0：dnspod.tencentcloudapi.com）
//! 认证：CAM SecretId/SecretKey + TC3-HMAC-SHA256 签名（自实现，与官方「签名方法 v3」一致；
//! 参考项目用 tencentcloud-sdk-rs 0.1，此处自实现以去掉老 SDK 依赖并做友好错误归一）。
//! 背景：DNSPod 已被腾讯云收购，dnsapi.cn 老 API 降级为备选方案，故主用腾讯云 API 3.0。
//! 响应处理：先查 Response.Error 输出具体 Code/Message，再解析业务结构。

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::models::{DnsRecord, Domain, DomainList, ProviderConfig, RecordList, PLATFORM_DNSPOD};

/// 腾讯云 DNSPod 服务名（TC3 签名 scope 用）
const TC_SERVICE: &str = "dnspod";
/// 腾讯云 DNSPod 端点（参与签名与请求）
const TC_HOST: &str = "dnspod.tencentcloudapi.com";
/// DNSPod API 3.0 版本号
const TC_VERSION: &str = "2021-03-23";
/// 请求超时（避免界面无限转圈）
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// 响应体预览（诊断用：截断到 200 字符，换行折叠）
fn preview(text: &str) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    let cut: String = flat.chars().take(200).collect();
    if flat.chars().count() > 200 {
        format!("{cut}…")
    } else {
        cut
    }
}

/// SHA256 十六进制（TC3 签名两处使用）
fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// HMAC-SHA256（TC3 派生密钥链）
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC 接受任意长度密钥");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// 构造 CanonicalRequest（POST /，无 query，content-type + host 参与签名）
fn canonical_request(payload: &str, host: &str) -> String {
    format!(
        "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{host}\n\ncontent-type;host\n{}",
        sha256_hex(payload.as_bytes())
    )
}

/// TC3-HMAC-SHA256 签名，返回 Authorization 头值（官方格式：逗号后带空格）
fn tc3_authorization(
    secret_id: &str,
    secret_key: &str,
    service: &str,
    host: &str,
    payload: &str,
    timestamp: i64,
    date: &str,
) -> String {
    // 1. CanonicalRequest → 哈希
    let hashed_canonical = sha256_hex(canonical_request(payload, host).as_bytes());
    // 2. StringToSign（算法 + 时间戳 + 凭据范围 + 哈希）
    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign =
        format!("TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{hashed_canonical}");
    // 3. 三层派生密钥（TC3+SecretKey → SecretDate → SecretService → SecretSigning）→ 签名
    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256(&secret_date, service.as_bytes());
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
    let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes()));
    // 4. Authorization 头
    format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders=content-type;host, Signature={signature}"
    )
}

/// 腾讯云 DNSPod 客户端（API 3.0，TC3 签名）
pub struct TencentDns {
    /// HTTP 客户端
    client: reqwest::Client,
    /// CAM SecretId
    secret_id: String,
    /// CAM SecretKey
    secret_key: String,
}

impl TencentDns {
    /// 由密钥配置构造客户端；密钥未配置返回明确错误（引导去设置页）
    pub fn new(cfg: &ProviderConfig) -> Result<Self, String> {
        if cfg.id.is_empty() || cfg.key.is_empty() {
            return Err("请先在「设置」页配置腾讯云 DNSPod 密钥（SecretId / SecretKey）".into());
        }
        Ok(Self {
            client: reqwest::Client::new(),
            secret_id: cfg.id.clone(),
            secret_key: cfg.key.clone(),
        })
    }

    /// 通用调用：TC3 签名 → POST JSON → 错误归一（Response.Error 优先，再校验 HTTP 状态）
    async fn call(&self, action: &str, payload: Value) -> Result<Value, String> {
        let payload_str = payload.to_string();
        // 时间戳与 UTC 日期（签名与 X-TC-Timestamp 共用同一时间）
        let now = OffsetDateTime::now_utc();
        let timestamp = now.unix_timestamp();
        let date = now
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| format!("时间格式化失败: {e}"))?
            .chars()
            .take(10)
            .collect::<String>();
        let authorization = tc3_authorization(
            &self.secret_id,
            &self.secret_key,
            TC_SERVICE,
            TC_HOST,
            &payload_str,
            timestamp,
            &date,
        );

        let resp = self
            .client
            .post(format!("https://{TC_HOST}/"))
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", TC_VERSION)
            .timeout(REQUEST_TIMEOUT)
            .body(payload_str)
            .send()
            .await
            .map_err(|e| format!("请求失败: {e}"))?;

        // 状态码 + 文本：非 2xx 或 JSON 解析失败时错误信息带响应体预览，便于定位
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;
        if !status.is_success() {
            return Err(format!("腾讯云 HTTP 错误({status}): {}", preview(&text)));
        }
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| format!("响应解析失败({status}): {e} — 响应内容: {}", preview(&text)))?;

        // 业务错误：腾讯云统一包在 Response.Error 里
        if let Some(err) = json.get("Response").and_then(|r| r.get("Error")) {
            let code = err
                .get("Code")
                .and_then(|c| c.as_str())
                .unwrap_or("Unknown");
            let msg = err
                .get("Message")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误");
            return Err(format!("腾讯云 DNSPod 错误({code}): {msg}"));
        }
        Ok(json)
    }

    /// 域名列表（一次拉取前 100 条）
    pub async fn get_domains(&self) -> Result<DomainList, String> {
        let json = self.call("DescribeDomainList", json!({})).await?;
        let resp: DescribeDomainListResp =
            serde_json::from_value(json).map_err(|e| format!("响应解析失败: {e}"))?;
        let list = resp
            .domain_list
            .into_iter()
            .map(|d| Domain {
                domain_id: d.domain_id.to_string(),
                domain_name: d.name,
                record_total: d.record_count,
                create_time: d.created_on,
                platform: PLATFORM_DNSPOD.into(),
            })
            .collect();
        Ok(DomainList {
            list,
            total: resp.domain_count_info.domain_total,
        })
    }

    /// 解析记录列表（分页；offset 从 0 开始）
    pub async fn get_records(
        &self,
        domain: &str,
        page: u32,
        size: u32,
    ) -> Result<RecordList, String> {
        let offset = (page.max(1) - 1) * size;
        let json = self
            .call(
                "DescribeRecordList",
                json!({
                    "Domain": domain,
                    "Offset": offset,
                    "Limit": size,
                }),
            )
            .await?;
        let resp: DescribeRecordListResp =
            serde_json::from_value(json).map_err(|e| format!("响应解析失败: {e}"))?;
        let list = resp
            .record_list
            .into_iter()
            .map(|r| DnsRecord {
                record_id: r.record_id.to_string(),
                rr: r.name,
                record_type: r.type_field,
                ttl: r.ttl,
                value: r.value,
                line: r.line,
            })
            .collect();
        Ok(RecordList {
            list,
            total: resp.record_count_info.total_count,
        })
    }

    /// 添加解析记录（线路固定「默认」）
    pub async fn add_record(
        &self,
        domain: &str,
        rr: &str,
        rtype: &str,
        value: &str,
        ttl: u32,
    ) -> Result<(), String> {
        self.call(
            "CreateRecord",
            json!({
                "Domain": domain,
                "SubDomain": rr,
                "RecordType": rtype,
                "RecordLine": "默认",
                "Value": value,
                "TTL": ttl,
            }),
        )
        .await?;
        Ok(())
    }

    /// 更新解析记录
    pub async fn update_record(
        &self,
        domain: &str,
        record_id: &str,
        rr: &str,
        rtype: &str,
        value: &str,
        ttl: u32,
    ) -> Result<(), String> {
        self.call(
            "ModifyRecord",
            json!({
                "Domain": domain,
                "RecordId": record_id.parse::<i64>().map_err(|_| "记录 ID 无效".to_string())?,
                "SubDomain": rr,
                "RecordType": rtype,
                "RecordLine": "默认",
                "Value": value,
                "TTL": ttl,
            }),
        )
        .await?;
        Ok(())
    }

    /// 删除解析记录
    pub async fn delete_record(&self, domain: &str, record_id: &str) -> Result<(), String> {
        self.call(
            "DeleteRecord",
            json!({
                "Domain": domain,
                "RecordId": record_id.parse::<i64>().map_err(|_| "记录 ID 无效".to_string())?,
            }),
        )
        .await?;
        Ok(())
    }
}

/* ── 腾讯云 DNSPod API 3.0 响应结构（PascalCase 字段） ── */

/// DescribeDomainList 响应
#[derive(Deserialize, Debug)]
struct DescribeDomainListResp {
    /// 域名列表
    #[serde(rename = "DomainList")]
    domain_list: Vec<TencentDomain>,
    /// 域名统计
    #[serde(rename = "DomainCountInfo")]
    domain_count_info: TencentDomainCountInfo,
}

/// 单个域名
#[derive(Deserialize, Debug)]
struct TencentDomain {
    /// 域名 ID
    #[serde(rename = "DomainId")]
    domain_id: i64,
    /// 域名名称
    #[serde(rename = "Name")]
    name: String,
    /// 解析记录数量
    #[serde(rename = "RecordCount")]
    record_count: u32,
    /// 创建时间
    #[serde(rename = "CreatedOn")]
    created_on: String,
}

/// 域名统计
#[derive(Deserialize, Debug)]
struct TencentDomainCountInfo {
    /// 域名总数
    #[serde(rename = "DomainTotal")]
    domain_total: u32,
}

/// DescribeRecordList 响应
#[derive(Deserialize, Debug)]
struct DescribeRecordListResp {
    /// 记录统计
    #[serde(rename = "RecordCountInfo")]
    record_count_info: TencentRecordCountInfo,
    /// 记录列表
    #[serde(rename = "RecordList")]
    record_list: Vec<TencentRecord>,
}

/// 记录统计
#[derive(Deserialize, Debug)]
struct TencentRecordCountInfo {
    /// 记录总数
    #[serde(rename = "TotalCount")]
    total_count: u32,
}

/// 单条解析记录
#[derive(Deserialize, Debug)]
struct TencentRecord {
    /// 记录 ID
    #[serde(rename = "RecordId")]
    record_id: i64,
    /// 主机记录
    #[serde(rename = "Name")]
    name: String,
    /// 记录类型
    #[serde(rename = "Type")]
    type_field: String,
    /// 记录值
    #[serde(rename = "Value")]
    value: String,
    /// TTL（秒）
    #[serde(rename = "TTL")]
    ttl: u32,
    /// 解析线路
    #[serde(rename = "Line")]
    line: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与官方 Python SDK（tencentcloud-sdk-python sign_tc3）交叉验证：
    /// 固定密钥/时间戳/请求体，期望值由 SDK 生成（2026-08-08 实测）
    #[test]
    fn tc3_matches_official_sdk() {
        let payload = r#"{"Domain":"example.com"}"#;
        let host = "dnspod.tencentcloudapi.com";
        // CanonicalRequest 哈希（SDK 期望值）
        let canonical = canonical_request(payload, host);
        assert_eq!(
            sha256_hex(canonical.as_bytes()),
            "7fa1d689f10362e6f66fb34010903078e4801fe8bde8c40ad5ac1e9acc028830",
            "CanonicalRequest 构造与官方 SDK 不一致"
        );
        // 完整 Authorization 头（SDK 期望值）
        let auth = tc3_authorization(
            "AKIDtest123",
            "testsecret456",
            "dnspod",
            host,
            payload,
            1551113065,
            "2019-02-25",
        );
        assert_eq!(
            auth,
            "TC3-HMAC-SHA256 Credential=AKIDtest123/2019-02-25/dnspod/tc3_request, \
             SignedHeaders=content-type;host, \
             Signature=80affd0dd1d45b4e15d4c39ff58a4b2a28d770e4e176d7de0c1d00d1205ff71c"
        );
    }

    /// Authorization 头格式（官方格式：Credential/scope + SignedHeaders + Signature=64hex）
    #[test]
    fn tc3_authorization_format() {
        let auth = tc3_authorization(
            "AKIDtest",
            "secretkey",
            "dnspod",
            "dnspod.tencentcloudapi.com",
            "{}",
            1551113065,
            "2019-02-25",
        );
        assert!(auth.starts_with(
            "TC3-HMAC-SHA256 Credential=AKIDtest/2019-02-25/dnspod/tc3_request, SignedHeaders=content-type;host, Signature="
        ));
        let sig = auth.rsplit("Signature=").next().unwrap();
        assert_eq!(sig.len(), 64, "签名应为 64 位十六进制");
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 真实网络：无效 SecretKey 应返回签名校验错误（AuthFailure.SignatureFailure）
    /// 验证签名流程被服务端接受（手动运行：cargo test -- --ignored dns::dnspod）
    #[tokio::test]
    #[ignore]
    async fn call_rejects_invalid_secret() {
        let dns = TencentDns::new(&ProviderConfig {
            id: "AKIDinvalid".into(),
            key: "invalidsecret".into(),
        })
        .unwrap();
        let err = dns.get_domains().await.unwrap_err();
        assert!(
            err.contains("AuthFailure"),
            "无效密钥应返回签名校验错误: {err}"
        );
    }
}
