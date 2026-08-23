/**
 * DNS 工具 · 纯函数单测
 */
import { describe, expect, it } from 'vitest'
import {
  CLOUD_RECORD_TYPES,
  DNS_SERVERS,
  isValidDomain,
  isValidServer,
  platformLabel,
  recordTypeBadgeClass,
  serverLabel,
} from './useDns'

describe('isValidDomain', () => {
  it('合法域名', () => {
    expect(isValidDomain('example.com')).toBe(true)
    expect(isValidDomain('www.example.com.cn')).toBe(true)
    expect(isValidDomain('a-b.example.com')).toBe(true)
  })
  it('非法输入', () => {
    expect(isValidDomain('')).toBe(false)
    expect(isValidDomain('example')).toBe(false)
    expect(isValidDomain('exa mple.com')).toBe(false)
    expect(isValidDomain('example..com')).toBe(false)
    expect(isValidDomain('http://example.com')).toBe(false)
  })
})

describe('isValidServer', () => {
  it('IP 与域名', () => {
    expect(isValidServer('223.5.5.5')).toBe(true)
    expect(isValidServer('8.8.8.8')).toBe(true)
    expect(isValidServer('1.1.1.1')).toBe(true)
    expect(isValidServer('dns.google')).toBe(true)
    expect(isValidServer('2400:3200::1')).toBe(true)
  })
  it('非法输入', () => {
    expect(isValidServer('')).toBe(false)
    expect(isValidServer('999.1.1.1')).toBe(false)
    expect(isValidServer('abc')).toBe(false)
  })
})

describe('serverLabel', () => {
  it('system 显示为系统默认', () => {
    expect(serverLabel('system')).toBe('系统默认')
    expect(serverLabel('8.8.8.8')).toBe('8.8.8.8')
  })
})

describe('recordTypeBadgeClass', () => {
  it('全部查询类型都有配色（非空）', () => {
    for (const t of ['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SOA', 'PTR', 'CAA', 'SRV', 'ANY']) {
      expect(recordTypeBadgeClass(t).length).toBeGreaterThan(0)
    }
  })
  it('未知类型回落中性色', () => {
    expect(recordTypeBadgeClass('ZZZ')).toContain('bg-neutral')
  })
})

describe('预设与常量', () => {
  it('预设服务器含系统默认', () => {
    expect(DNS_SERVERS.some((s) => s.addr === 'system')).toBe(true)
    expect(DNS_SERVERS.length).toBeGreaterThanOrEqual(5)
  })
  it('云解析类型都是合法记录类型', () => {
    for (const t of CLOUD_RECORD_TYPES) {
      expect(['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'CAA', 'SRV']).toContain(t)
    }
  })
  it('platformLabel', () => {
    expect(platformLabel('aliyun')).toBe('阿里云')
    expect(platformLabel('dnspod')).toBe('腾讯云 DNSPod')
    expect(platformLabel('cloudflare')).toBe('Cloudflare')
    expect(platformLabel('cf')).toBe('cf')
  })
})
