/**
 * DNS 插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/plugins/dns/models.rs 的 serde 结构同步（camelCase）。
 */

/** 单平台密钥配置（可选公共 Vault；未选择时使用手工 AccessKey） */
export interface ProviderConfig {
  /** 密钥 ID（AccessKeyId / Token ID） */
  id: string
  /** 密钥（AccessKeySecret / Token） */
  key: string
  /** 公共 Vault 的 AccessKey 对凭证 id */
  credentialRef?: string
}

/** Cloudflare API Token 配置（可选公共 Vault；未选择时使用手工 Token） */
export interface CloudflareConfig {
  token: string
  /** 公共 Vault 的 API Token 凭证 id */
  credentialRef?: string
}

/** DNS 插件全局配置（三平台密钥） */
export interface DnsConfig {
  aliyun: ProviderConfig
  dnspod: ProviderConfig
  cloudflare: CloudflareConfig
}

/** 云解析平台标识 */
export type DnsPlatform = 'aliyun' | 'dnspod' | 'cloudflare'

/** 域名条目（云解析侧） */
export interface CloudDomain {
  domainId: string
  domainName: string
  recordTotal: number
  platform: string
  createTime: string
}

/** 域名列表 */
export interface DomainList {
  list: CloudDomain[]
  total: number
}

/** 解析记录条目（云解析侧） */
export interface CloudRecord {
  recordId: string
  rr: string
  recordType: string
  ttl: number
  value: string
  line: string
}

/** 解析记录列表 */
export interface RecordList {
  list: CloudRecord[]
  total: number
}

/** 单条 DNS 查询应答（dig 风格） */
export interface DnsAnswer {
  name: string
  recordType: string
  ttl: number
  value: string
}

/** 单台 DNS 服务器查询结果 */
export interface ServerQueryResult {
  server: string
  ok: boolean
  error?: string
  elapsedMs: number
  records: DnsAnswer[]
}

/** 命令清单（本插件命令的唯一出处） */
export const commands = {
  dnsQuery: 'dns_query',
  dnsDomains: 'dns_domains',
  dnsRecords: 'dns_records',
  dnsAddRecord: 'dns_add_record',
  dnsUpdateRecord: 'dns_update_record',
  dnsDeleteRecord: 'dns_delete_record',
  dnsConfigGet: 'dns_config_get',
  dnsConfigSet: 'dns_config_set',
} as const

/** 命令入参 */
export type Payloads = {
  dns_query: { domain: string; rtype: string; servers: string[] }
  dns_domains: { platform: string }
  dns_records: { platform: string; domain: string; page: number; size: number; keyword: string }
  dns_add_record: {
    payload: {
      platform: string
      domain: string
      rr: string
      rtype: string
      value: string
      ttl: number
    }
  }
  dns_update_record: {
    payload: {
      platform: string
      domain: string
      recordId: string
      rr: string
      rtype: string
      value: string
      ttl: number
    }
  }
  dns_delete_record: { platform: string; domain: string; recordId: string }
  dns_config_get: Record<string, never>
  dns_config_set: { config: DnsConfig }
}

/** 命令返回 */
export type Results = {
  dns_query: ServerQueryResult[]
  dns_domains: DomainList
  dns_records: RecordList
  dns_add_record: void
  dns_update_record: void
  dns_delete_record: void
  dns_config_get: DnsConfig
  dns_config_set: void
}
