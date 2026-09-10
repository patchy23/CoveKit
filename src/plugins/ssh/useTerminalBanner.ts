/**
 * useTerminalBanner · 终端断线/重连提示文案（纯函数，便于单测）
 * 措辞对齐 OpenSSH 客户端：英文、无中文装饰、无黄色告警色；`host` 缺失时退化为 remote host
 * （断开事件会把 host 清空，因此调用方需缓存最近一次已知地址）。
 */

/** 转义前缀：暗灰（与交互输出区分，断开/重连属系统消息） */
const DIM = '\x1b[90m'

/** 终止样式 */
const RESET = '\x1b[0m'

/** 断开提示：`Connection to <host> closed.` + 暗灰引导行 */
export function disconnectBanner(host?: string): string {
  return `\r\nConnection to ${host || 'remote host'} closed.\r\n${DIM}Press Enter to reconnect.${RESET}\r\n`
}

/** 重连分隔线：标出缓冲里的断点位置与恢复时间 */
export function reconnectSeparator(host: string | undefined, time: string): string {
  return `\r\n${DIM}--- reconnected to ${host || 'remote host'} at ${time} ---${RESET}\r\n`
}

/** 当前时间（24 小时制，秒级）：重连分隔线用 */
export function bannerTime(date: Date = new Date()): string {
  return date.toLocaleTimeString('en-GB', { hour12: false })
}
