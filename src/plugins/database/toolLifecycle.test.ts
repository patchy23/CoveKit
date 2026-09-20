/**
 * 数据库工具关闭策略的行为测试（T10-4）
 *
 * 策略：先保护事务、运行中查询和草稿，再断开全部连接。
 * 这里锁住两点：全部连接都被断开；断开失败要报出来，不能静默留连接。
 */
import { ref } from 'vue'
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
function spec(): {
  owner: string
  prepare: () => Promise<string | null>
  dispose: () => Promise<void>
} {
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

  it('空闲工具允许关闭', async () => {
    expect(await spec().prepare()).toBeNull()
  })
  it('事务和查询阻止关闭，草稿保存失败也可见', async () => {
    const queryStates = ref({ q: { status: 'success', transactionActive: true } })
    const flushDrafts = vi.fn().mockResolvedValue(undefined)
    useDatabaseToolLifecycle({ queryStates, flushDrafts })
    const prepare = useToolLifecycle.mock.calls.at(-1)![1].prepare
    expect(await prepare()).toContain('未提交事务')
    queryStates.value.q = { status: 'running', transactionActive: false }
    expect(await prepare()).toContain('仍在执行')
    queryStates.value.q.status = 'success'
    flushDrafts.mockRejectedValue(new Error('写入失败'))
    expect(await prepare()).toContain('写入失败')
    flushDrafts.mockResolvedValue(undefined)
    expect(await prepare()).toBeNull()
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
