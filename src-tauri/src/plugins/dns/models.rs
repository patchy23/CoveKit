//! DNS 插件 · serde 数据结构（与前端 plugins/dns/contracts.ts 逐字段同步）
//! 命名规范：serde 统一 camelCase（rename_all），错误统一 ok:false + error 结构。

use serde::{Deserialize, Serialize};

/// 云平台标识：阿里云
pub const PLATFORM_ALIYUN: &str = "aliyun";

/// 云平台标识：DNSPod
pub const PLATFORM_DNSPOD: &str = "dnspod";

/// 单平台密钥配置（阿里云 AccessKey / DNSPod Token）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// 密钥 ID（AccessKeyId / Token ID）
    pub id: String,
    /// 密钥（AccessKeySecret / Token）
    pub key: String,
}

/// DNS 插件全局配置（两平台密钥）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsConfig {
    /// 阿里云 AccessKey 配置
    pub aliyun: ProviderConfig,
    /// DNSPod Token 配置
    pub dnspod: ProviderConfig,
}

/// 域名条目（云解析侧）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    /// 域名在平台侧的 ID
    pub domain_id: String,
    /// 域名（如 example.com）
    pub domain_name: String,
    /// 解析记录数量
    pub record_total: u32,
    /// 所属平台（aliyun / dnspod）
    pub platform: String,
    /// 创建时间（平台格式）
    pub create_time: String,
}

/// 域名列表结果
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DomainList {
    /// 域名列表
    pub list: Vec<Domain>,
    /// 总数（分页用）
    pub total: u32,
}

/// 解析记录条目（云解析侧）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    /// 记录 ID（平台侧）
    pub record_id: String,
    /// 主机记录（@ 表示根域名）
    pub rr: String,
    /// 记录类型（A/AAAA/CNAME/MX/TXT/NS/...）
    pub record_type: String,
    /// TTL（秒）
    pub ttl: u32,
    /// 记录值
    pub value: String,
    /// 解析线路（阿里云/DNSPod 默认「默认」）
    pub line: String,
}

/// 解析记录列表结果
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecordList {
    /// 记录列表
    pub list: Vec<DnsRecord>,
    /// 总数（分页用）
    pub total: u32,
}

/// 单条 DNS 查询应答记录（dig 风格）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsAnswer {
    /// 记录完整名称（如 www.example.com.）
    pub name: String,
    /// 记录类型
    pub record_type: String,
    /// TTL（秒）
    pub ttl: u32,
    /// 记录值
    pub value: String,
}

/// 单台 DNS 服务器查询结果
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerQueryResult {
    /// 服务器地址（或 system 表示系统默认）
    pub server: String,
    /// 是否成功
    pub ok: bool,
    /// 失败原因（ok=false 时）
    pub error: Option<String>,
    /// 查询耗时（毫秒）
    pub elapsed_ms: u64,
    /// 应答记录列表
    pub records: Vec<DnsAnswer>,
}

/// 添加解析记录入参（打包为结构体，规避 clippy too_many_arguments）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddRecordPayload {
    /// 云平台（aliyun / dnspod）
    pub platform: String,
    /// 域名
    pub domain: String,
    /// 主机记录（@ 表示根域名）
    pub rr: String,
    /// 记录类型
    pub rtype: String,
    /// 记录值
    pub value: String,
    /// TTL（秒）
    pub ttl: u32,
}

/// 更新解析记录入参（打包为结构体，规避 clippy too_many_arguments）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRecordPayload {
    /// 云平台（aliyun / dnspod）
    pub platform: String,
    /// 域名
    pub domain: String,
    /// 记录 ID（平台侧）
    pub record_id: String,
    /// 主机记录（@ 表示根域名）
    pub rr: String,
    /// 记录类型
    pub rtype: String,
    /// 记录值
    pub value: String,
    /// TTL（秒）
    pub ttl: u32,
}
