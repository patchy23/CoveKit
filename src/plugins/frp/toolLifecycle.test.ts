/**
 * FRP 关闭策略的行为测试（T10-4）
 *
 * 关键口径：只有 running/starting 算「还在跑」；停止失败的档案必须逐条报出来，
 * 不允许「停了一半当成功」。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const status = vi.fn()
const stop = vi.fn()
const useToolLifecycle = vi.fn()

vi.mock('./ipc', () => ({
  ipc: {
    status: (...args: unknown[]) => status(...args),
    stop: (...args: unknown[]) => stop(...args),
  },
}))

vi.mock('@/core/lifecycle', () => ({
  useToolLifecycle: (...args: unknown[]) => useToolLifecycle(...args),
}))

const { isFrpLive, useFrpToolLifecycle } = await import('./toolLifecycle')

/** 取出登记进框架的声明 */
function spec(): {
  owner: string
  prepare: () => Promise<string | null>
  dispose: () => Promise<void>
} {
  useFrpToolLifecycle()
  return useToolLifecycle.mock.calls[useToolLifecycle.mock.calls.length - 1]?.[1]
}

describe('FRP 工具关闭策略', () => {
  beforeEach(() => {
    status.mockReset()
    stop.mockReset()
    useToolLifecycle.mockReset()
  })

  it('运行状态判定与界面运行标记同口径', () => {
    expect(isFrpLive('running')).toBe(true)
    expect(isFrpLive('starting')).toBe(true)
    expect(isFrpLive('stopped')).toBe(false)
    expect(isFrpLive('failed')).toBe(false)
  })

  it('没有进程在跑时不拦关闭', async () => {
    status.mockResolvedValue([{ fileName: 'a.toml', state: 'stopped' }])
    await expect(spec().prepare()).resolves.toBeNull()
  })

  it('有进程在跑时说明会被停止', async () => {
    status.mockResolvedValue([
      { fileName: 'a.toml', state: 'running' },
      { fileName: 'b.toml', state: 'starting' },
      { fileName: 'c.toml', state: 'stopped' },
    ])
    await expect(spec().prepare()).resolves.toContain('2 个 frpc 进程')
  })

  it('清理只停还在跑的档案', async () => {
    status.mockResolvedValue([
      { fileName: 'a.toml', state: 'running' },
      { fileName: 'c.toml', state: 'stopped' },
    ])
    stop.mockResolvedValue({ fileName: 'a.toml', state: 'stopped' })

    await spec().dispose()

    expect(stop).toHaveBeenCalledTimes(1)
    expect(stop).toHaveBeenCalledWith('a.toml')
  })

  it('停止失败要报出来', async () => {
    status.mockResolvedValue([{ fileName: 'a.toml', state: 'running' }])
    stop.mockRejectedValue(new Error('进程未响应'))

    await expect(spec().dispose()).rejects.toThrow(/a\.toml：进程未响应/)
  })
})
