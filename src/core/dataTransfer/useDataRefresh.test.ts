import { effectScope } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useDataRefresh } from './useDataRefresh'

const mock = vi.hoisted(() => ({
  handler: undefined as ((datasets: string[]) => void) | undefined,
  dispose: vi.fn(),
  toast: vi.fn(),
}))
vi.mock('@/core/ipc/spaceEvents', () => ({
  onSpaceDataChanged(handler: (datasets: string[]) => void) {
    mock.handler = handler
    return mock.dispose
  },
}))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mock.toast }) }))

describe('导入刷新订阅', () => {
  beforeEach(() => vi.clearAllMocks())

  it('只刷新相关数据集并随作用域释放', async () => {
    const refresh = vi.fn()
    const scope = effectScope()
    scope.run(() => useDataRefresh('database.', refresh))
    mock.handler?.(['dns.providers'])
    await Promise.resolve()
    expect(refresh).not.toHaveBeenCalled()
    mock.handler?.(['database.history'])
    await Promise.resolve()
    expect(refresh).toHaveBeenCalledTimes(1)
    mock.handler?.(['*'])
    await Promise.resolve()
    expect(refresh).toHaveBeenCalledTimes(2)
    scope.stop()
    expect(mock.dispose).toHaveBeenCalledOnce()
  })

  it('刷新失败可感知且不会形成未处理拒绝', async () => {
    const scope = effectScope()
    scope.run(() => useDataRefresh('dns.', () => Promise.reject(new Error('读取失败'))))
    mock.handler?.(['dns.providers'])
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(mock.toast).toHaveBeenCalledWith(expect.stringContaining('读取失败'))
    scope.stop()
  })
})
