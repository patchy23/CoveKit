import { Channel } from '@tauri-apps/api/core'
import type { DriverInstallProgress, DriverStatus } from '../contracts'
import { connectionIpc } from '../ipc'

export interface OracleDriverInstallCallbacks {
  onProgress: (progress: DriverInstallProgress) => void
  onError: (error: unknown) => void
}

export interface OracleDriverInstallScope {
  requestId: string
  promise: Promise<DriverStatus>
  cancel: () => void
}

function cancelledError(): Error {
  const error = new Error('Oracle 驱动准备已取消')
  error.name = 'AbortError'
  return error
}

/**
 * 为单个 Oracle 准备操作创建带独立 requestId 的安装 scope。
 * 后端以首条 waiting 进度作为请求已注册的确认，因此在确认前只记取消，
 * 避免取消命令早于安装请求而漏掉。
 */
export function startOracleDriverInstall(
  callbacks: OracleDriverInstallCallbacks
): OracleDriverInstallScope {
  const requestId = crypto.randomUUID()
  let registered = false
  let cancellationRequested = false
  let cancellationSent = false
  let settled = false

  const reportError = (error: unknown) => callbacks.onError(error)

  const sendCancellation = () => {
    if (!registered || !cancellationRequested || cancellationSent || settled) return
    cancellationSent = true
    void connectionIpc.cancelDriverInstall(requestId).catch(reportError)
  }

  const onProgress = new Channel<DriverInstallProgress>()
  onProgress.onmessage = (progress) => {
    if (!progress || progress.requestId !== requestId || progress.dbType !== 'oracle') return
    registered = true
    if (cancellationRequested) {
      sendCancellation()
      return
    }
    try {
      callbacks.onProgress(progress)
    } catch (error) {
      reportError(error)
    }
  }

  const promise = connectionIpc
    .installDriver('oracle', requestId, onProgress)
    .catch((error) => {
      if (cancellationRequested) throw cancelledError()
      throw error
    })
    .then((status) => {
      if (cancellationRequested) throw cancelledError()
      return status
    })
    .finally(() => {
      settled = true
      onProgress.onmessage = () => {}
    })

  return {
    requestId,
    promise,
    cancel() {
      if (settled || cancellationRequested) return
      cancellationRequested = true
      sendCancellation()
    },
  }
}

export function isOracleDriverInstallCancelled(error: unknown): boolean {
  return error instanceof Error && error.name === 'AbortError'
}
