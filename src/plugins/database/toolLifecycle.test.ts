/**
 * 数据库工具关闭策略的行为测试（T10-4）
 *
 * 策略：关闭页签即断开全部连接（不弹询问，因为断开连接没有数据丢失风险）。
 * 这里锁住两点：全部连接都被断开；断开失败要报出来，不能静默留连接。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const list = vi.fn()
const disconnect = vi.fn()
const useToolLifecycle = vi.fn()

vi.mock('./ipc', () => ({
  connectionIpc: {
    list: (...args: unknown[]) => list(...args),
    disconnect: (...args: unknown[]) => disconnect(...args),
  },
}))

vi.mock('@/core/lifecycle', () => ({
  useToolLifecycle: (...args: unknown[]) => useToolLifecycle(...args),
}))

const { useDatabaseToolLifecycle } = await import('./toolLifecycle')

/** 取出登记进框架的声明 */
function spec(): { owner: string; prepare?: unknown; dispose: () => Promise<void> } {
  useDatabaseToolLifecycle()
  expect(useToolLifecycle).toHaveBeenCalledWith('database', expect.any(Object))
  return useToolLifecycle.mock.calls[useToolLifecycle.mock.calls.length - 1]?.[1]
}

describe('数据库工具关闭策略', () => {
  beforeEach(() => {
    list.mockReset()
    disconnect.mockReset()
    useToolLifecycle.mockReset()
  })

  it('不拦关闭：断开连接不需要用户确认', () => {
    expect(spec().prepare).toBeUndefined()
  })

  it('关闭页签断开全部连接', async () => {
    list.mockResolvedValue([
      { id: 'c1', label: '本地 MySQL' },
      { id: 'c2', label: '测试 PG' },
    ])
    disconnect.mockResolvedValue(undefined)

    await spec().dispose()

    expect(disconnect.mock.calls.map((call) => call[0])).toEqual(['c1', 'c2'])
  })

  it('断开失败要报出来', async () => {
    list.mockResolvedValue([{ id: 'c1', label: '本地 MySQL' }])
    disconnect.mockRejectedValue(new Error('连接已被服务端关闭'))

    await expect(spec().dispose()).rejects.toThrow(/本地 MySQL：连接已被服务端关闭/)
  })
})
