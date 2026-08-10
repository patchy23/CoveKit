/**
 * DNS 工具 · 纯函数（记录类型/服务器预设/校验/徽标配色）
 */

/** 查询记录类型（与 Rust query.rs parse_record_type 保持一致） */
export const RECORD_TYPES = [
  'A',
  'AAAA',
  'CNAME',
  'MX',
  'TXT',
  'NS',
  'SOA',
  'PTR',
  'CAA',
  'SRV',
  'ANY',
] as const

export type RecordType = (typeof RECORD_TYPES)[number]

/** 云解析可添加的记录类型（A/CNAME 外常见类型） */
export const CLOUD_RECORD_TYPES = ['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'CAA', 'SRV'] as const

/** 预设公共 DNS 服务器（label + 地址；system = 系统默认） */
export interface DnsServerPreset {
  label: string
  addr: string
}

export const DNS_SERVERS: DnsServerPreset[] = [
  { label: '系统默认', addr: 'system' },
  { label: '阿里 DNS', addr: '223.5.5.5' },
  { label: 'DNSPod', addr: '119.29.29.29' },
  { label: '114 DNS', addr: '114.114.114.114' },
  { label: 'Cloudflare', addr: '1.1.1.1' },
  { label: 'Google', addr: '8.8.8.8' },
]

/** TTL 预设（秒） */
export const TTL_PRESETS = [60, 300, 600, 1800, 3600, 86400]

/** 域名合法性（宽松校验：字母数字/连字符/点，至少一个点） */
export function isValidDomain(domain: string): boolean {
  const d = domain.trim()
  if (!d || d.length > 253) return false
  if (!d.includes('.')) return false
  return /^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*$/.test(
    d
  )
}

/** IP 或域名（查询服务器输入：IPv4/IPv6/域名均可） */
export function isValidServer(s: string): boolean {
  const v = s.trim()
  if (!v) return false
  // IPv4（四段 0-255）
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(v)) {
    return v.split('.').every((p) => Number(p) <= 255)
  }
  // IPv6（含 :: 缩写）或域名
  return isValidDomain(v) || v.includes(':')
}

/** 服务器地址显示名（system → 系统默认；其余原样） */
export function serverLabel(addr: string): string {
  if (addr === 'system') return '系统默认'
  return addr
}

/** 记录类型徽标配色（A 绿 / AAAA 蓝 / CNAME 紫 / MX 橙 / TXT 青 / NS 灰 / SOA 灰 / PTR 蓝 / CAA 黄 / SRV 紫 / ANY 灰） */
export function recordTypeBadgeClass(rtype: string): string {
  switch (rtype.toUpperCase()) {
    case 'A':
      return 'bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark'
    case 'AAAA':
      return 'bg-info-soft text-info-strong dark:bg-info-soft-dark dark:text-info-dark'
    case 'CNAME':
    case 'SRV':
      return 'bg-purple-soft text-purple-strong dark:bg-purple-soft-dark dark:text-purple-dark'
    case 'MX':
      return 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
    case 'TXT':
      return 'bg-cyan-soft text-cyan-strong dark:bg-cyan-soft-dark dark:text-cyan-dark'
    case 'PTR':
      return 'bg-info-soft text-info-strong dark:bg-info-soft-dark dark:text-info-dark'
    case 'CAA':
      return 'bg-warning-soft text-warning-strong dark:bg-warning-soft-dark dark:text-warning-dark'
    default:
      return 'bg-neutral text-secondary dark:bg-neutral-dark dark:text-secondary-dark'
  }
}

/** 平台显示名 */
export function platformLabel(platform: string): string {
  return platform === 'aliyun' ? '阿里云' : platform === 'dnspod' ? '腾讯云 DNSPod' : platform
}
