/**
 * SSH 工具 · 系统信息与磁盘明细的展示逻辑（纯函数，便于单测）
 * 排序、使用率阈值、空值占位都在这里决定，组件只负责渲染。
 */
import type { SshDiskEntry } from '../contracts'

/** 使用率语义色阶（normal 用默认色，不额外着色） */
export type DiskUsageTone = 'normal' | 'warning' | 'danger'

/** 磁盘表网格行（键与 UiDataGrid 列 key 一一对应；type 别名以便赋给 Record<string, unknown>[]） */
export type DiskRow = {
  /** 行标识（同一挂载点只对应一个文件系统） */
  id: string
  /** 挂载点 */
  mountPoint: string
  /** 文件系统类型（ext4/xfs/tmpfs） */
  fsType: string
  /** 文件系统名（/dev/vda1、overlay） */
  filesystem: string
  /** 已用 / 总量（合并列文本，保留 df -h 原文） */
  usageText: string
  /** 使用率百分比 0-100 */
  usePercent: number
}

/** 使用率阈值：>95% 危险、>85% 警告，其余常规（与容量告警一致） */
export function diskUsageTone(percent: number): DiskUsageTone {
  if (!Number.isFinite(percent)) return 'normal'
  if (percent > 95) return 'danger'
  if (percent > 85) return 'warning'
  return 'normal'
}

/** 百分比文本：整数不带小数（87%），非整数保留一位（88.5%） */
export function formatPercent(percent: number): string {
  if (!Number.isFinite(percent)) return '-'
  return `${Number.isInteger(percent) ? percent : percent.toFixed(1)}%`
}

/** 空值占位：空串/未采集统一显示 `-`（字段缺失要可见，不能显示空白） */
export function textOrDash(value: string | null | undefined): string {
  const text = (value ?? '').trim()
  return text === '' ? '-' : text
}

/** 平均负载文本：`0.08 / 0.03 / 0.00`（1/5/15 分钟） */
export function formatLoadAvg(loadAvg: [number, number, number]): string {
  return loadAvg.map((value) => (Number.isFinite(value) ? value.toFixed(2) : '-')).join(' / ')
}

/** 磁盘按使用率降序（同使用率按挂载点升序，保证刷新时行序稳定） */
export function sortDisksByUsage(disks: SshDiskEntry[]): SshDiskEntry[] {
  return [...disks].sort(
    (left, right) =>
      right.usePercent - left.usePercent || left.mountPoint.localeCompare(right.mountPoint)
  )
}

/** 磁盘明细 → 网格行（排序 + 合并「已用 / 总量」列） */
export function toDiskRows(disks: SshDiskEntry[]): DiskRow[] {
  return sortDisksByUsage(disks).map((disk) => ({
    id: `${disk.filesystem}@${disk.mountPoint}`,
    mountPoint: disk.mountPoint,
    fsType: disk.fsType,
    filesystem: disk.filesystem,
    usageText: `${disk.usedText} / ${disk.sizeText}`,
    usePercent: disk.usePercent,
  }))
}
