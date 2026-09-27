//! 阿里云云解析（alidns）实现
//! 使用共享传输调用 alidns.cn-hangzhou，保持既有 RPC 操作与响应契约。

use serde::{Deserialize, Serialize};

use super::aliyun_rpc::AliyunRpc;
use super::models::{DnsRecord, Domain, DomainList, ProviderConfig, RecordList};

/// 阿里云云解析客户端（RPC 签名与传输由 owner 内适配器处理）
pub struct AliyunDns {
    /// RPC 客户端（AccessKey 签名 + alidns 端点）
    client: AliyunRpc,
}

impl AliyunDns {
    /// 由密钥配置构造客户端；密钥未配置返回明确错误（引导去设置页）
    pub fn new(cfg: &ProviderConfig) -> Result<Self, String> {
        if cfg.id.is_empty() || cfg.key.is_empty() {
            return Err("请先在「设置」页配置阿里云 AccessKey（AccessKey ID / Secret）".into());
        }
        Ok(Self {
            client: AliyunRpc::new(cfg),
        })
    }

    /// 域名列表（一次拉取前 100 条）
    pub async fn get_domains(&self) -> Result<DomainList, String> {
        let resp = self
            .client
            .call::<DescribeDomains>(
                "DescribeDomains",
                &[("PageNumber", "1"), ("PageSize", "100")],
            )
            .await?;
        let list = resp
            .domain_list
            .domain
            .into_iter()
            .map(|d| Domain {
                domain_id: d.domain_id,
                domain_name: d.domain_name,
                record_total: d.record_count,
                create_time: d.create_time,
                platform: super::models::PLATFORM_ALIYUN.into(),
            })
            .collect();
        Ok(DomainList {
            list,
            total: resp.total_count,
        })
    }

    /// 解析记录列表（分页；keyword 非空时按主机记录/记录值模糊搜索，服务端过滤）
    pub async fn get_records(
        &self,
        domain: &str,
        page: u32,
        size: u32,
        keyword: &str,
    ) -> Result<RecordList, String> {
        // 阿里云用 RRKeyWord（主机记录）+ ValueKeyWord（记录值）两个独立关键字参数
        let mut params = vec![
            ("DomainName".to_string(), domain.to_string()),
            ("PageNumber".to_string(), page.max(1).to_string()),
            ("PageSize".to_string(), size.clamp(1, 200).to_string()),
        ];
        if !keyword.trim().is_empty() {
            params.push(("RRKeyWord".to_string(), keyword.trim().to_string()));
            params.push(("ValueKeyWord".to_string(), keyword.trim().to_string()));
        }
        let resp = self
            .client
            .call::<DescribeDomainRecords>(
                "DescribeDomainRecords",
                &params
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect::<Vec<_>>(),
            )
            .await?;
        let list = resp
            .domain_records
            .record
            .into_iter()
            .map(|r| DnsRecord {
                record_id: r.record_id,
                rr: r.rr,
                record_type: r.type_field,
                ttl: r.ttl,
                value: r.value,
                line: r.line,
            })
            .collect();
        Ok(RecordList {
            list,
            total: resp.total_count,
        })
    }

    /// 添加解析记录
    pub async fn add_record(
        &self,
        domain: &str,
        rr: &str,
        rtype: &str,
        value: &str,
        ttl: u32,
    ) -> Result<(), String> {
        self.client
            .call::<ActionDomainRecord>(
                "AddDomainRecord",
                &[
                    ("DomainName", domain),
                    ("RR", rr),
                    ("Type", rtype),
                    ("Value", value),
                    ("TTL", &ttl.to_string()),
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
        self.client
            .call::<ActionDomainRecord>(
                "UpdateDomainRecord",
                &[
                    ("DomainName", domain),
                    ("RecordId", record_id),
                    ("RR", rr),
                    ("Type", rtype),
                    ("Value", value),
                    ("TTL", &ttl.to_string()),
                ],
            )
            .await?;
        Ok(())
    }

    /// 删除解析记录
    pub async fn delete_record(&self, record_id: &str) -> Result<(), String> {
        self.client
            .call::<ActionDomainRecord>("DeleteDomainRecord", &[("RecordId", record_id)])
            .await?;
        Ok(())
    }
}

/* ── 阿里云 alidns 响应结构（与官方 OpenAPI 字段对齐） ── */

/// DescribeDomains 响应
#[derive(Serialize, Deserialize, Debug)]
struct DescribeDomains {
    /// 域名列表容器
    #[serde(rename = "Domains")]
    domain_list: AliDomainList,
    /// 域名总数
    #[serde(rename = "TotalCount")]
    total_count: u32,
}

/// 域名列表容器
#[derive(Serialize, Deserialize, Debug)]
struct AliDomainList {
    /// 域名数组
    #[serde(rename = "Domain")]
    domain: Vec<AliDomain>,
}

/// 单个域名
#[derive(Serialize, Deserialize, Debug)]
struct AliDomain {
    /// 域名 ID
    #[serde(rename = "DomainId")]
    domain_id: String,
    /// 域名名称
    #[serde(rename = "DomainName")]
    domain_name: String,
    /// 解析记录数量
    #[serde(rename = "RecordCount")]
    record_count: u32,
    /// 创建时间
    #[serde(rename = "CreateTime")]
    create_time: String,
}

/// DescribeDomainRecords 响应
#[derive(Serialize, Deserialize, Debug)]
struct DescribeDomainRecords {
    /// 记录总数
    #[serde(rename = "TotalCount")]
    total_count: u32,
    /// 记录列表容器
    #[serde(rename = "DomainRecords")]
    domain_records: DomainRecords,
}

/// 记录列表容器
#[derive(Serialize, Deserialize, Debug)]
struct DomainRecords {
    /// 记录数组
    #[serde(rename = "Record")]
    record: Vec<AliRecord>,
}

/// 单条解析记录
#[derive(Serialize, Deserialize, Debug)]
struct AliRecord {
    /// 主机记录
    #[serde(rename = "RR")]
    rr: String,
    /// 记录类型
    #[serde(rename = "Type")]
    type_field: String,
    /// 解析线路
    #[serde(rename = "Line")]
    line: String,
    /// 记录值
    #[serde(rename = "Value")]
    value: String,
    /// 记录 ID
    #[serde(rename = "RecordId")]
    record_id: String,
    /// TTL（秒）
    #[serde(rename = "TTL")]
    ttl: u32,
}

/// 增删改动作响应（仅需 RequestId 校验成功）
#[derive(Serialize, Deserialize, Debug)]
struct ActionDomainRecord {
    /// 请求 ID
    #[serde(rename = "RequestId")]
    request_id: String,
}
