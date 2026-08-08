//! DNSPod 云解析实现
//! 走官方 dnsapi.cn REST API（Token 认证，POST 表单），不依赖腾讯云 SDK；
//! 与参考项目的差异：参考项目用腾讯云 API 3.0（TC3 签名），此处用更轻量的官方 Token API。

use serde_json::Value;

use super::models::{DnsRecord, Domain, DomainList, ProviderConfig, RecordList};

/// dnsapi.cn API 基地址
const API_BASE: &str = "https://dnsapi.cn";

/// DNSPod 客户端（login_token 认证）
pub struct DnsPod {
    /// HTTP 客户端（表单 POST）
    client: reqwest::Client,
    /// Token ID
    token_id: String,
    /// Token 本体
    token: String,
}

impl DnsPod {
    /// 由密钥配置构造客户端；Token 未配置返回明确错误（引导去设置页）
    pub fn new(cfg: &ProviderConfig) -> Result<Self, String> {
        if cfg.id.is_empty() || cfg.key.is_empty() {
            return Err("请先在「设置」页配置腾讯云 API Token（ID / Token）".into());
        }
        Ok(Self {
            client: reqwest::Client::new(),
            token_id: cfg.id.clone(),
            token: cfg.key.clone(),
        })
    }

    /// 通用 POST form 调用：注入认证参数 → 请求 → 校验 status.code → 返回原始 JSON
    async fn call(&self, action: &str, extra: &[(&str, String)]) -> Result<Value, String> {
        // 认证参数（login_token=ID,Token）+ 业务参数合并为表单
        let mut params: Vec<(String, String)> = vec![
            (
                "login_token".into(),
                format!("{},{}", self.token_id, self.token),
            ),
            ("format".into(), "json".into()),
            ("lang".into(), "cn".into()),
        ];
        params.extend(extra.iter().map(|(k, v)| (k.to_string(), v.clone())));

        let resp = self
            .client
            .post(format!("{API_BASE}/{action}"))
            .header("User-Agent", "patchyBox-DNS/0.1")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("请求失败: {e}"))?;
        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("响应解析失败: {e}"))?;

        // dnsapi.cn 统一 status 结构：code != 1 即业务错误
        let code = json["status"]["code"].as_i64().unwrap_or(-1);
        if code != 1 {
            let msg = json["status"]["message"].as_str().unwrap_or("未知错误");
            return Err(format!("DNSPod 错误({code}): {msg}"));
        }
        Ok(json)
    }

    /// 域名列表（一次拉取前 100 条）
    pub async fn get_domains(&self) -> Result<DomainList, String> {
        let json = self
            .call(
                "Domain.List",
                &[("offset", "0".into()), ("length", "100".into())],
            )
            .await?;
        let list = json["domains"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| {
                        Some(Domain {
                            domain_id: d["id"].as_i64()?.to_string(),
                            domain_name: d["name"].as_str()?.to_string(),
                            record_total: d["records"].as_str()?.parse().unwrap_or(0),
                            create_time: d["created_on"].as_str().unwrap_or("").to_string(),
                            platform: super::models::PLATFORM_DNSPOD.into(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let total = json["info"]["domain_total"]
            .as_i64()
            .unwrap_or(list.len() as i64) as u32;
        Ok(DomainList { list, total })
    }

    /// 解析记录列表（分页）
    pub async fn get_records(
        &self,
        domain: &str,
        page: u32,
        size: u32,
    ) -> Result<RecordList, String> {
        // dnsapi.cn 用 offset 分页（从 0 开始）
        let offset = ((page.max(1) - 1) * size).to_string();
        let json = self
            .call(
                "Record.List",
                &[
                    ("domain", domain.into()),
                    ("offset", offset),
                    ("length", size.to_string()),
                ],
            )
            .await?;
        let list = json["records"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(DnsRecord {
                            record_id: r["id"].as_i64()?.to_string(),
                            rr: r["name"].as_str()?.to_string(),
                            record_type: r["type"].as_str()?.to_string(),
                            ttl: r["ttl"].as_str()?.parse().unwrap_or(600),
                            value: r["value"].as_str()?.to_string(),
                            line: r["line"].as_str().unwrap_or("默认").to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let total = json["info"]["record_total"]
            .as_i64()
            .unwrap_or(list.len() as i64) as u32;
        Ok(RecordList { list, total })
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
            "Record.Create",
            &[
                ("domain", domain.into()),
                ("sub_domain", rr.into()),
                ("record_type", rtype.into()),
                ("record_line", "默认".into()),
                ("value", value.into()),
                ("ttl", ttl.to_string()),
            ],
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
            "Record.Modify",
            &[
                ("domain", domain.into()),
                ("record_id", record_id.into()),
                ("sub_domain", rr.into()),
                ("record_type", rtype.into()),
                ("record_line", "默认".into()),
                ("value", value.into()),
                ("ttl", ttl.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// 删除解析记录
    pub async fn delete_record(&self, domain: &str, record_id: &str) -> Result<(), String> {
        self.call(
            "Record.Remove",
            &[("domain", domain.into()), ("record_id", record_id.into())],
        )
        .await?;
        Ok(())
    }
}
