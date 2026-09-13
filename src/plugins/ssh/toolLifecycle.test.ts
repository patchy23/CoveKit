/**
 * SSH 关闭策略的行为测试（T10-4）
 *
 * 盯策略本身：有连接才拦、拦的理由里有几个连接；清理要把每个失败都收集出来，
 * 不能「断到一半就算成功」。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const sshConnections = vi.fn()
const sshDisconnect = vi.fn()
const useToolLifecycle = vi.fn()

vi.mock('./ipc', () => ({
  ipc: {
    sshConnections: (...args: unknown[]) => sshConnections(...args),
    sshDisconnect: (...args: unknown[]) => sshDisconnect(...args),
  },
}))

vi.mock('@/core/lifecycle', () => ({
  useToolLifecycle: (...args: unknown[]) => useToolLifecycle(...args),
}))

const { useSshToolLifecycle } = await import('./toolLifecycle')

/** 取出登记进框架的声明（测试直接调用 prepare/dispose，无需挂载组件） */
function spec(): {
  owner: string
  prepare: () => Promise<string | null>
  dispose: () => Promise<void>
} {
  useSshToolLifecycle()
  expect(useToolLifecycle).toHaveBeenCalledWith('ssh', expect.any(Object))
  return useToolLifecycle.mock.calls[useToolLifecycle.mock.calls.length - 1]?.[1]
}

describe('SSH 工具关闭策略', () => {
  beforeEach(() => {
    sshConnections.mockReset()
    sshDisconnect.mockReset()
    useToolLifecycle.mockReset()
  })

  it('没有活动连接时不拦关闭', async () => {
    sshConnections.mockResolvedValue([
      { sessionId: 's1', status: 'disconnected', host: 'a' },
      { sessionId: 's2', status: 'error', host: 'b' },
    ])
    await expect(spec().prepare()).resolves.toBeNull()
  })

  it('连接与连接中都算活动会话，理由里带上数量', async () => {
    sshConnections.mockResolvedValue([
      { sessionId: 's1', status: 'connected', host: 'a' },
      { sessionId: 's2', status: 'reconnecting', host: 'b' },
      { sessionId: 's3', status: 'disconnected', host: 'c' },
    ])
    await expect(spec().prepare()).resolves.toContain('2 个连接')
  })

  it('清理时逐个断开会话', async () => {
    sshConnections.mockResolvedValue([
      { sessionId: 's1', status: 'connected', host: 'a' },
      { sessionId: 's2', status: 'connected', host: 'b' },
    ])
    sshDisconnect.mockResolvedValue(undefined)

    await spec().dispose()

    expect(sshDisconnect.mock.calls.map((call) => call[0])).toEqual(['s1', 's2'])
  })

  it('断开失败要报出来，并且继续处理其余会话', async () => {
    sshConnections.mockResolvedValue([
      { sessionId: 's1', status: 'connected', host: 'a.example' },
      { sessionId: 's2', status: 'connected', host: 'b.example' },
    ])
    sshDisconnect.mockImplementation((sessionId: string) =>
      sessionId === 's1' ? Promise.reject(new Error('连接已失效')) : Promise.resolve(undefined)
    )

    await expect(spec().dispose()).rejects.toThrow(/a\.example：连接已失效/)
    expect(sshDisconnect).toHaveBeenCalledTimes(2)
  })
})
