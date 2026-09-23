/** 仅接受来自当前主机的绝对 file URI，不允许目录报告执行命令或切换到另一台主机。 */
export function parseTerminalDirectory(report: string, host: string): string | undefined {
  try {
    const uri = new URL(report)
    if (uri.protocol !== 'file:' || uri.search || uri.hash || uri.username || uri.password) return
    if (uri.hostname && uri.hostname.toLowerCase() !== host.toLowerCase()) return
    const path = decodeURIComponent(uri.pathname)
    if (!path.startsWith('/') || [...path].some((char) => char.charCodeAt(0) < 32)) return
    return path
  } catch {
    return
  }
}
