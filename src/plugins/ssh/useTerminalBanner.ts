/**
 * useTerminalBanner · 终端断线/重连提示文案（纯函数，便于单测）
 * 措辞对齐 OpenSSH 客户端：英文、无中文装饰、无黄色告警色；`host` 缺失时退化为 remote host
 * （断开事件会把 host 清空，因此调用方需缓存最近一次已知地址）。
 */

/** 断线提示主色：品牌橙（真彩色，与窗口交互色 tertiary 同值） */
const CLOSED_FG = '\x1b[38;2;240;86;44m'

/** 次级说明与分隔线：中灰（比 ANSI 90 亮一档；90 在暗底仅 2.6:1，几乎读不出） */
const MUTED_FG = '\x1b[38;2;139;148;158m'

/** 终止样式 */
const RESET = '\x1b[0m'

/** 断开提示：品牌橙的 `Connection to <host> closed.` + 中灰引导行 */
export function disconnectBanner(host?: string): string {
  return `\r\n${CLOSED_FG}Connection to ${host || 'remote host'} closed.${RESET}\r\n${MUTED_FG}Press Enter to reconnect.${RESET}\r\n`
}

/** 重连分隔线：标出缓冲里的断点位置与恢复时间 */
export function reconnectSeparator(host: string | undefined, time: string): string {
  return `\r\n${MUTED_FG}--- reconnected to ${host || 'remote host'} at ${time} ---${RESET}\r\n`
}

/** 当前时间（24 小时制，秒级）：重连分隔线用 */
export function bannerTime(date: Date = new Date()): string {
  return date.toLocaleTimeString('en-GB', { hour12: false })
}
