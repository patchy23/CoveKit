/**
 * 接口调试工具关闭策略的行为测试（T10-4）
 *
 * 策略：关闭页签关闭全部**已打开**的 WS 会话；HTTP 请求后端一次性执行、无可取消句柄，
 * 不做假清理。这里锁住：只关打开的、后端回报失败要抛出来。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const wsSessions = vi.fn()
const wsClose = vi.fn()
const useToolLifecycle = vi.fn()

vi.mock('./ipc', () => ({
  ipc: {
    wsSessions: (...args: unknown[]) => wsSessions(...args),
    wsClose: (...args: unknown[]) => wsClose(...args),
  },
}))

vi.mock('@/core/lifecycle', () => ({
  useToolLifecycle: (...args: unknown[]) => useToolLifecycle(...args),
}))

const { useHttpWsToolLifecycle } = await import('./toolLifecycle')

/** 取出登记进框架的声明 */
function spec(): { owner: string; dispose: () => Promise<void> } {
  useHttpWsToolLifecycle()
  expect(useToolLifecycle).toHaveBeenCalledWith('http-ws', expect.any(Object))
  return useToolLifecycle.mock.calls[useToolLifecycle.mock.calls.length - 1]?.[1]
}

describe('接口调试工具关闭策略', () => {
  beforeEach(() => {
    wsSessions.mockReset()
    wsClose.mockReset()
    useToolLifecycle.mockReset()
  })

  it('没有打开的 WS 会话时不做任何调用', async () => {
    wsSessions.mockResolvedValue([{ id: 'w1', url: 'wss://a', open: false }])

    await spec().dispose()

    expect(wsClose).not.toHaveBeenCalled()
  })

  it('关闭全部已打开的会话', async () => {
    wsSessions.mockResolvedValue([
      { id: 'w1', url: 'wss://a', open: true },
      { id: 'w2', url: 'wss://b', open: false },
      { id: 'w3', url: 'wss://c', open: true },
    ])
    wsClose.mockResolvedValue({ ok: true })

    await spec().dispose()

    expect(wsClose.mock.calls.map((call) => call[0])).toEqual(['w1', 'w3'])
  })

  it('后端回报关闭失败要报出来（不把 ok:false 当成功）', async () => {
    wsSessions.mockResolvedValue([{ id: 'w1', url: 'wss://a', open: true }])
    wsClose.mockResolvedValue({ ok: false, message: '对端未响应' })

    await expect(spec().dispose()).rejects.toThrow(/wss:\/\/a：对端未响应/)
  })
})
