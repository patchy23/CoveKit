import { describe, expect, it } from 'vitest'
import type { SshDiskEntry } from './contracts'
import {
  diskUsageTone,
  formatLoadAvg,
  formatPercent,
  sortDisksByUsage,
  textOrDash,
  toDiskRows,
} from './sshSystemInfo'

function disk(partial: Partial<SshDiskEntry> & { mountPoint: string }): SshDiskEntry {
  return {
    filesystem: '/dev/vda1',
    fsType: 'ext4',
    sizeText: '99G',
    usedText: '12G',
    availText: '87G',
    usePercent: 0,
    ...partial,
  }
}

describe('sshSystemInfo 使用率阈值与格式化', () => {
  it('diskUsageTone 分档：>95 危险、>85 警告、其余常规', () => {
    expect(diskUsageTone(0)).toBe('normal')
    expect(diskUsageTone(85)).toBe('normal')
    expect(diskUsageTone(85.1)).toBe('warning')
    expect(diskUsageTone(95)).toBe('warning')
    expect(diskUsageTone(95.1)).toBe('danger')
    expect(diskUsageTone(Number.NaN)).toBe('normal')
  })

  it('formatPercent 整数不带小数、非整数保留一位', () => {
    expect(formatPercent(87)).toBe('87%')
    expect(formatPercent(88.5)).toBe('88.5%')
    expect(formatPercent(Number.NaN)).toBe('-')
  })

  it('textOrDash 空值与空串显示 `-`', () => {
    expect(textOrDash('prod-01')).toBe('prod-01')
    expect(textOrDash('')).toBe('-')
    expect(textOrDash('   ')).toBe('-')
    expect(textOrDash(undefined)).toBe('-')
    expect(textOrDash(null)).toBe('-')
  })

  it('formatLoadAvg 固定两位小数', () => {
    expect(formatLoadAvg([0.08, 0.03, 0])).toBe('0.08 / 0.03 / 0.00')
    expect(formatLoadAvg([1.5, Number.NaN, 12.345])).toBe('1.50 / - / 12.35')
  })
})

describe('sshSystemInfo 磁盘排序与行映射', () => {
  it('sortDisksByUsage 按使用率降序、同值按挂载点升序', () => {
    const disks = [
      disk({ mountPoint: '/', usePercent: 13 }),
      disk({ mountPoint: '/data', usePercent: 97 }),
      disk({ mountPoint: '/var', usePercent: 13 }),
    ]
    expect(sortDisksByUsage(disks).map((item) => item.mountPoint)).toEqual(['/data', '/', '/var'])
  })

  it('sortDisksByUsage 不修改入参', () => {
    const disks = [
      disk({ mountPoint: '/a', usePercent: 1 }),
      disk({ mountPoint: '/b', usePercent: 2 }),
    ]
    sortDisksByUsage(disks)
    expect(disks.map((item) => item.mountPoint)).toEqual(['/a', '/b'])
  })

  it('toDiskRows 合并「已用 / 总量」并生成稳定行标识', () => {
    const rows = toDiskRows([
      disk({
        mountPoint: '/data',
        filesystem: '/dev/vdb1',
        usedText: '460G',
        sizeText: '500G',
        usePercent: 97,
      }),
      disk({ mountPoint: '/', usePercent: 13 }),
    ])
    expect(rows.map((row) => row.mountPoint)).toEqual(['/data', '/'])
    expect(rows[0]).toEqual({
      id: '/dev/vdb1@/data',
      mountPoint: '/data',
      fsType: 'ext4',
      filesystem: '/dev/vdb1',
      usageText: '460G / 500G',
      usePercent: 97,
    })
  })

  it('toDiskRows 空输入返回空数组', () => {
    expect(toDiskRows([])).toEqual([])
  })
})
