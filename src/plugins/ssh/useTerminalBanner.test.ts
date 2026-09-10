import { describe, expect, it } from 'vitest'
import { bannerTime, disconnectBanner, reconnectSeparator } from './useTerminalBanner'

describe('disconnectBanner', () => {
  it('按 OpenSSH 措辞输出英文断线提示（含主机名）', () => {
    const text = disconnectBanner('82.157.102.178')
    expect(text).toContain('Connection to 82.157.102.178 closed.')
    expect(text).toContain('Press Enter to reconnect.')
  })

  it('不带中文文案，且提示行用暗灰而非黄色告警色', () => {
    const text = disconnectBanner('10.0.0.1')
    expect(text).not.toMatch(/[\u4e00-\u9fa5]/)
    expect(text).not.toContain('\x1b[33m')
    expect(text).toContain('\x1b[90m')
  })

  it('主机名缺失时退化为 remote host（断开事件会清空 host）', () => {
    expect(disconnectBanner(undefined)).toContain('Connection to remote host closed.')
    expect(disconnectBanner('')).toContain('Connection to remote host closed.')
  })

  it('每段以 CRLF 换行且以换行收尾（避免与提示符粘连）', () => {
    const text = disconnectBanner('h')
    expect(text.startsWith('\r\n')).toBe(true)
    expect(text.endsWith('\r\n')).toBe(true)
  })
})

describe('reconnectSeparator', () => {
  it('标出重连目标与时间', () => {
    const text = reconnectSeparator('82.157.102.178', '23:05:01')
    expect(text).toContain('--- reconnected to 82.157.102.178 at 23:05:01 ---')
    expect(text).not.toMatch(/[\u4e00-\u9fa5]/)
  })

  it('主机名缺失时退化为 remote host', () => {
    expect(reconnectSeparator('', '09:00:00')).toContain('reconnected to remote host')
  })
})

describe('bannerTime', () => {
  it('输出 24 小时制秒级时间（不出现 AM/PM）', () => {
    const text = bannerTime(new Date(2026, 8, 10, 23, 5, 1))
    expect(text).toBe('23:05:01')
  })
})
