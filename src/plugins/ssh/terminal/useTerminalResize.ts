import type { FitAddon } from '@xterm/addon-fit'
import type { Terminal } from 'xterm'

interface TerminalResizeOptions {
  getTerminal: () => Terminal | null
  getFitAddon: () => FitAddon | null
  getHost: () => HTMLElement | null
  getTerminalId: () => string | null
  resize: (terminalId: string, cols: number, rows: number) => Promise<unknown>
  onError: (terminalId: string, error: unknown) => void
}

/** 管理 xterm 布局测量与远端 PTY 尺寸同步，避免隐藏页签期间测量到 0 尺寸。 */
export function createTerminalResizeController(options: TerminalResizeOptions) {
  let fitFrame: number | null = null

  function fitTerminal(): boolean {
    const terminal = options.getTerminal()
    const fitAddon = options.getFitAddon()
    const host = options.getHost()
    if (!terminal || !fitAddon || !host || host.clientWidth <= 0 || host.clientHeight <= 0) {
      return false
    }
    fitAddon.fit()
    const screen = terminal.element?.querySelector<HTMLElement>('.xterm-screen')
    const style = getComputedStyle(host)
    const availableHeight =
      host.clientHeight -
      parseFloat(style.paddingTop || '0') -
      parseFloat(style.paddingBottom || '0')
    if (screen && screen.scrollHeight > availableHeight + 1) {
      terminal.resize(terminal.cols, Math.max(1, terminal.rows - 1))
    }
    return terminal.cols > 0 && terminal.rows > 0
  }

  function scheduleFitAndSync() {
    if (fitFrame !== null) return
    fitFrame = requestAnimationFrame(() => {
      fitFrame = null
      const terminal = options.getTerminal()
      const terminalId = options.getTerminalId()
      if (!fitTerminal() || !terminal || !terminalId) return
      const { cols, rows } = terminal
      if (cols <= 0 || rows <= 0) return
      void options.resize(terminalId, cols, rows).catch((error) => {
        options.onError(terminalId, error)
      })
    })
  }

  function cancelScheduledFit() {
    if (fitFrame === null) return
    cancelAnimationFrame(fitFrame)
    fitFrame = null
  }

  return { scheduleFitAndSync, cancelScheduledFit }
}
