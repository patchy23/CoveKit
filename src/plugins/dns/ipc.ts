/**
 * DNS 插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { DnsConfig, DomainList, RecordList, ServerQueryResult } from './contracts'

export const ipc = {
  dnsReadPrepare: (): Promise<string> => invokeCommand('dns_read_prepare', {}),
  dnsReadCancel: (requestId: string): Promise<void> =>
    invokeCommand('dns_read_cancel', { requestId }),
  dnsQuery: (
    domain: string,
    rtype: string,
    servers: string[],
    requestId?: string
  ): Promise<ServerQueryResult[]> =>
    invokeCommand('dns_query', { domain, rtype, servers, requestId }),
  dnsDomains: (platform: string, requestId?: string): Promise<DomainList> =>
    invokeCommand('dns_domains', { platform, requestId }),
  dnsRecords: (
    platform: string,
    domain: string,
    page: number,
    size: number,
    keyword: string,
    requestId?: string
  ): Promise<RecordList> =>
    invokeCommand('dns_records', { platform, domain, page, size, keyword, requestId }),
  dnsAddRecord: (
    platform: string,
    domain: string,
    rr: string,
    rtype: string,
    value: string,
    ttl: number
  ): Promise<void> =>
    invokeCommand('dns_add_record', {
      payload: { platform, domain, rr, rtype, value, ttl },
    }),
  dnsUpdateRecord: (
    platform: string,
    domain: string,
    recordId: string,
    rr: string,
    rtype: string,
    value: string,
    ttl: number
  ): Promise<void> =>
    invokeCommand('dns_update_record', {
      payload: { platform, domain, recordId, rr, rtype, value, ttl },
    }),
  dnsDeleteRecord: (platform: string, domain: string, recordId: string): Promise<void> =>
    invokeCommand('dns_delete_record', { platform, domain, recordId }),
  dnsConfigGet: (): Promise<DnsConfig> => invokeCommand('dns_config_get', {}),
  dnsConfigSet: (config: DnsConfig): Promise<void> => invokeCommand('dns_config_set', { config }),
}
