/** 存储维护必须在无阻断且清理成功后切换，不能强制丢弃内容或重启进程。 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useUiStore } from '@/stores/ui'
import { useStorageMaintenance } from './useStorageMaintenance'

const mocks = vi.hoisted(() => ({
  collect: vi.fn(),
  request: vi.fn(),
  dispose: vi.fn(),
}))

vi.mock('@/core/lifecycle/toolContext', () => ({
  collectAllToolBlockers: mocks.collect,
  disposeAllTools: mocks.dispose,
}))
vi.mock('@/core/lifecycle/closeBridge', () => ({ requestBackendClose: mocks.request }))
vi.mock('@/core/lifecycle', () => ({ negotiateToolClose: vi.fn(), publishGlobalHidden: vi.fn() }))

beforeEach(() => {
  vi.clearAllMocks()
  setActivePinia(createPinia())
  mocks.collect.mockResolvedValue([])
  mocks.request.mockResolvedValue({ proceed: true, forced: false, blockers: [], failures: [] })
  mocks.dispose.mockResolvedValue({ failures: [], timedOut: false })
  const ui = useUiStore()
  ui.openTool('ssh')
  ui.openTool('database')
  ui.openSettings()
})

describe('存储维护关闭协商', () => {
  it('未保存内容不清理工具、不调用迁移、不刷新', async () => {
    const blockers = [{ owner: 'editor', message: '未保存' }]
    mocks.collect.mockResolvedValue(blockers)
    mocks.request.mockResolvedValue({ proceed: false, blockers: ['editor: 未保存'], failures: [] })
    const reload = vi.fn()
    const operation = vi.fn()
    const flow = useStorageMaintenance((key) => key, reload)
    expect(await flow.run(operation)).toBe(false)
    expect(mocks.request).toHaveBeenCalledWith('restart', null, blockers, false)
    expect(mocks.dispose).not.toHaveBeenCalled()
    expect(operation).not.toHaveBeenCalled()
    expect(reload).not.toHaveBeenCalled()
    expect(useUiStore().openTabs).toHaveLength(2)
    expect(flow.error.value).toContain('未保存')
  })

  it.each([
    { failures: [{ owner: 'ssh', message: '释放失败' }], timedOut: false },
    { failures: [], timedOut: true },
  ])('清理失败或超时均停止，不移除页签', async (cleanup) => {
    mocks.dispose.mockResolvedValue(cleanup)
    const reload = vi.fn()
    const operation = vi.fn()
    const flow = useStorageMaintenance((key) => key, reload)
    expect(await flow.run(operation)).toBe(false)
    expect(operation).not.toHaveBeenCalled()
    expect(reload).not.toHaveBeenCalled()
    expect(useUiStore().openTabs).toHaveLength(2)
    expect(flow.error.value).toContain('storageCleanupFailed')
  })

  it('清理成功才移除工具页签并执行迁移，完成后仅刷新界面', async () => {
    const reload = vi.fn()
    const ui = useUiStore()
    const operation = vi.fn(async () => {
      expect(mocks.dispose).toHaveBeenCalledWith('restart')
      expect(ui.openTabs).toEqual([])
      expect(ui.activeTab).toBeNull()
      expect(ui.settingsOpen).toBe(true)
      expect(reload).not.toHaveBeenCalled()
    })
    const flow = useStorageMaintenance((key) => key, reload)
    expect(await flow.run(operation)).toBe(true)
    expect(operation).toHaveBeenCalledOnce()
    expect(reload).toHaveBeenCalledOnce()
    expect(flow.busy.value).toBe(false)
  })

  it('迁移失败显示原因且不刷新，可重试', async () => {
    const reload = vi.fn()
    const operation = vi.fn().mockRejectedValue(new Error('目标不可写'))
    const flow = useStorageMaintenance((key) => key, reload)
    expect(await flow.run(operation)).toBe(false)
    expect(reload).not.toHaveBeenCalled()
    expect(flow.error.value).toBe('目标不可写')
    expect(flow.busy.value).toBe(false)
    operation.mockResolvedValue({ root: 'new' })
    expect(await flow.run(operation)).toBe(true)
  })

  it('后台拒绝或关闭协商失败均不清理', async () => {
    mocks.request.mockRejectedValue(new Error('后台不可用'))
    const flow = useStorageMaintenance((key) => key, vi.fn())
    const operation = vi.fn()
    expect(await flow.run(operation)).toBe(false)
    expect(mocks.dispose).not.toHaveBeenCalled()
    expect(operation).not.toHaveBeenCalled()
    expect(flow.error.value).toBe('后台不可用')
  })
})
