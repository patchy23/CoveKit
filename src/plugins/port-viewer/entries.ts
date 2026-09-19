/** 端点筛选、标识与展示，纯函数不持有查询或窗口状态。 */
import type { PortEndpoint, PortEntry, Protocol } from './contracts'

export interface PortFilter {
  search: string
  by: 'port' | 'process' | 'pid'
  protocol: 'all' | Protocol
  view: 'listeners' | 'all'
}

export function filterError(filter: PortFilter): string {
  const value = filter.search.trim()
  if (!value || filter.by === 'process') return ''
  const max = filter.by === 'port' ? 65535 : 4294967295
  if (!/^\d+$/.test(value) || Number(value) < 1 || Number(value) > max) {
    return filter.by === 'port' ? '请输入 1–65535 的整数端口号' : '请输入有效的正整数 PID'
  }
  return ''
}

export function filterEntries(entries: PortEntry[], filter: PortFilter): PortEntry[] {
  if (filterError(filter)) return []
  const value = filter.search.trim().toLowerCase()
  return entries.filter((entry) => {
    if (filter.protocol !== 'all' && entry.protocol !== filter.protocol) return false
    if (filter.view === 'listeners' && entry.protocol === 'TCP' && entry.state !== 'LISTEN')
      return false
    if (!value) return true
    if (filter.by === 'port') return entry.localPort === Number(value)
    if (filter.by === 'pid') return entry.pid === Number(value)
    return entry.processName?.toLowerCase().includes(value) ?? false
  })
}

export function addressText(address: string, port: number): string {
  return address.includes(':') ? `[${address}]:${port}` : `${address}:${port}`
}

/** 只传端点身份，禁止把显示字段或旧状态混入关闭契约。 */
export function endpointOf(entry: PortEntry): PortEndpoint {
  const { protocol, family, localAddress, localPort, remoteAddress, remotePort, pid } = entry
  return { protocol, family, localAddress, localPort, remoteAddress, remotePort, pid }
}

export function entryKey(entry: PortEntry): string {
  return JSON.stringify([
    entry.protocol,
    entry.family,
    entry.localAddress,
    entry.localPort,
    entry.remoteAddress,
    entry.remotePort,
    entry.pid,
    entry.startedAt,
  ])
}

export function copyEntry(entry: PortEntry): string {
  return [
    `${entry.protocol} ${entry.family} ${addressText(entry.localAddress, entry.localPort)}`,
    `状态：${entry.state ?? '已绑定'}`,
    ...(entry.remoteAddress !== null && entry.remotePort !== null
      ? [`远端：${addressText(entry.remoteAddress, entry.remotePort)}`]
      : []),
    `进程：${entry.processName ?? '未读取'}，PID：${entry.pid}`,
    `程序路径：${entry.executablePath ?? '未读取'}`,
    ...(entry.detailError ? [`详情：${entry.detailError}`] : []),
  ].join('\n')
}
