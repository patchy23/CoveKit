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

interface SizeRequest {
  id: string
  cols: number
  rows: number
}

/** 可见挂载区不含内边距；测量按帧合并，PTY 请求串行且只同步最新尺寸。 */
export function createTerminalResizeController(options: TerminalResizeOptions) {
  let fitFrame: number | null = null
  let disposed = false
  let syncing = false
  let pending: SizeRequest | null = null
  let synced: string | null = null

  async function flush() {
    if (syncing || disposed) return
    syncing = true
    try {
      while (pending && !disposed) {
        const request: SizeRequest = pending
        pending = null
        if (request.id !== options.getTerminalId()) continue
        const key = JSON.stringify(request)
        if (key === synced) continue
        try {
          await options.resize(request.id, request.cols, request.rows)
          if (!disposed) synced = key
        } catch (error) {
          if (!disposed && request.id === options.getTerminalId())
            options.onError(request.id, error)
        }
      }
    } finally {
      syncing = false
    }
  }

  function scheduleFitAndSync() {
    if (disposed || fitFrame !== null) return
    fitFrame = requestAnimationFrame(() => {
      fitFrame = null
      const terminal = options.getTerminal()
      const addon = options.getFitAddon()
      const host = options.getHost()
      if (!terminal || !addon || !host || host.clientWidth <= 0 || host.clientHeight <= 0) return
      const size = addon.proposeDimensions()
      if (
        !size ||
        !Number.isFinite(size.cols) ||
        !Number.isFinite(size.rows) ||
        size.cols < 2 ||
        size.rows < 1
      )
        return
      if (terminal.cols !== size.cols || terminal.rows !== size.rows)
        terminal.resize(size.cols, size.rows)
      const id = options.getTerminalId()
      if (!id) {
        synced = null
        return
      }
      pending = { id, cols: size.cols, rows: size.rows }
      void flush()
    })
  }

  function cancelScheduledFit() {
    disposed = true
    pending = null
    if (fitFrame !== null) cancelAnimationFrame(fitFrame)
    fitFrame = null
  }

  return { scheduleFitAndSync, cancelScheduledFit }
}
