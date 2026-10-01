import type { Channel } from '@tauri-apps/api/core'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DriverInstallProgress, DriverStatus } from '../contracts'

const env = vi.hoisted(() => ({
  install: vi.fn(),
  cancel: vi.fn(),
}))

vi.mock('../ipc', () => ({
  connectionIpc: {
    installDriver: env.install,
    cancelDriverInstall: env.cancel,
  },
}))

const { isOracleDriverInstallCancelled, startOracleDriverInstall } =
  await import('./useOracleDriver')

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined
  let reject: (error?: unknown) => void = () => undefined
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function readyStatus(): DriverStatus {
  return { ready: true, kind: 'agent', version: '2.0' }
}

function progress(
  requestId: string,
  over: Partial<DriverInstallProgress> = {}
): DriverInstallProgress {
  return {
    requestId,
    dbType: 'oracle',
    phase: 'downloading',
    downloadedBytes: 20,
    totalBytes: 100,
    ...over,
  }
}

beforeEach(() => {
  let nextCallbackId = 0
  vi.stubGlobal('__TAURI_INTERNALS__', {
    transformCallback: () => ++nextCallbackId,
    unregisterCallback: vi.fn(),
  })
  env.install.mockReset()
  env.cancel.mockReset().mockResolvedValue(undefined)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('Oracle 驱动安装请求', () => {
  it('只接受当前 Oracle requestId 的进度，并等待安装状态结果', async () => {
    const pending = deferred<DriverStatus>()
    env.install.mockReturnValue(pending.promise)
    const onProgress = vi.fn()
    const onError = vi.fn()
    const scope = startOracleDriverInstall({ onProgress, onError })
    const channel = env.install.mock.calls[0][2] as Channel<DriverInstallProgress>

    channel.onmessage(progress('another-request'))
    channel.onmessage(progress(scope.requestId, { dbType: 'kingbase' }))
    expect(onProgress).not.toHaveBeenCalled()
    channel.onmessage(progress(scope.requestId))
    expect(onProgress).toHaveBeenCalledWith(progress(scope.requestId))
    expect(env.install).toHaveBeenCalledWith('oracle', scope.requestId, channel)

    pending.resolve(readyStatus())
    await expect(scope.promise).resolves.toEqual(readyStatus())
    expect(onError).not.toHaveBeenCalled()
  })

  it('首条 waiting 确认请求注册前只记取消，安装迟到失败按取消结束', async () => {
    const pending = deferred<DriverStatus>()
    env.install.mockReturnValue(pending.promise)
    const onProgress = vi.fn()
    const onError = vi.fn()
    const scope = startOracleDriverInstall({ onProgress, onError })
    const channel = env.install.mock.calls[0][2] as Channel<DriverInstallProgress>

    scope.cancel()
    expect(env.cancel).not.toHaveBeenCalled()
    channel.onmessage(progress(scope.requestId, { phase: 'waiting' }))
    expect(env.cancel).toHaveBeenCalledWith(scope.requestId)
    expect(onProgress).not.toHaveBeenCalled()

    pending.reject(new Error('后端已取消'))
    await expect(scope.promise).rejects.toMatchObject({ name: 'AbortError' })
    expect(isOracleDriverInstallCancelled(new Error('普通失败'))).toBe(false)
    expect(onError).not.toHaveBeenCalled()
  })

  it('取消 IPC 失败通过当前操作反馈，进度回调失败也不会静默', async () => {
    const pending = deferred<DriverStatus>()
    env.install.mockReturnValue(pending.promise)
    env.cancel.mockRejectedValue(new Error('取消请求失败'))
    const onError = vi.fn()
    const scope = startOracleDriverInstall({
      onProgress: () => {
        throw new Error('进度处理失败')
      },
      onError,
    })
    const channel = env.install.mock.calls[0][2] as Channel<DriverInstallProgress>

    channel.onmessage(progress(scope.requestId))
    scope.cancel()
    await Promise.resolve()
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: '进度处理失败' }))
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: '取消请求失败' }))

    pending.reject(new Error('后端已取消'))
    await expect(scope.promise).rejects.toMatchObject({ name: 'AbortError' })
  })

  it('未取消时安装失败保留为真实失败', async () => {
    env.install.mockRejectedValue(new Error('下载失败'))
    const scope = startOracleDriverInstall({ onProgress: vi.fn(), onError: vi.fn() })

    await expect(scope.promise).rejects.toThrow('下载失败')
  })
})
