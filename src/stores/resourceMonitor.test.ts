import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, disposePinia, setActivePinia, type Pinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import type { ResourceSnapshot } from '@/core/ipc/contracts'
import ResourceMonitorSettings from '@/features/settings/ResourceMonitorSettings.vue'
import ResourceMonitor from '@/features/sidebar/ResourceMonitor.vue'
import { useSettingsStore } from './settings'
import { useResourceMonitorStore } from './resourceMonitor'

const api = vi.hoisted(() => ({
  resourceMonitorSnapshot: vi.fn(),
  frameworkCommandsList: vi.fn(),
  settingsPatch: vi.fn(),
}))
vi.mock('@/core/ipc/ipc', () => ({ ipc: api }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@/core/registry/toolRegistry', () => ({
  getTools: () => [
    { id: 'ssh', name: 'SSH' },
    { id: 'other', name: '未接入工具' },
  ],
}))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ openTabs: [], openSettings: vi.fn() }) }))
let pinia: Pinia
const value: ResourceSnapshot = {
  memoryMetric: 'privateWorkingSet',
  partial: false,
  coverage: '进程树',
  missingProcesses: 0,
  logicalCpus: 4,
  processes: [
    {
      pid: 1,
      identity: 'start',
      kind: 'main',
      cpuSeconds: 1,
      residentBytes: 3 * 1024 ** 2,
      privateResidentBytes: 1024 ** 2,
      privateBytes: null,
      handles: null,
      threads: 2,
    },
  ],
}
beforeEach(() => {
  vi.useFakeTimers()
  pinia = createPinia()
  setActivePinia(pinia)
  vi.clearAllMocks()
  Object.assign(window, { __TAURI_INTERNALS__: {} })
  api.settingsPatch.mockResolvedValue(1)
  api.frameworkCommandsList.mockResolvedValue([{ name: 'ssh_list', doc: '', toolId: 'ssh' }])
  api.resourceMonitorSnapshot.mockResolvedValue(value)
})
afterEach(() => {
  disposePinia(pinia)
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
  vi.useRealTimers()
})

it('默认不采样，设置只列支持工具，关闭保留选择且停止采样', async () => {
  const monitor = useResourceMonitorStore()
  const settings = useSettingsStore()
  const wrapper = mount(ResourceMonitorSettings, { global: { plugins: [pinia] } })
  await flushPromises()
  expect(api.resourceMonitorSnapshot).not.toHaveBeenCalled()
  expect(wrapper.text()).toContain('SSH')
  expect(wrapper.text()).not.toContain('未接入工具')
  await wrapper.find('[role="switch"]').trigger('click')
  await flushPromises()
  expect(monitor.totals?.memory).toBe(1024 ** 2)
  expect(monitor.peakMemory).toBe(1024 ** 2)
  await wrapper.find('[role="checkbox"]').trigger('click')
  await flushPromises()
  expect(settings.settings.resourceMonitorTools).toEqual(['ssh'])
  const sidebar = mount(ResourceMonitor, { global: { plugins: [pinia] } })
  expect(sidebar.text()).toContain('1 MiB')
  await wrapper.find('[role="switch"]').trigger('click')
  await flushPromises()
  const count = api.resourceMonitorSnapshot.mock.calls.length
  await vi.advanceTimersByTimeAsync(4000)
  expect(api.resourceMonitorSnapshot).toHaveBeenCalledTimes(count)
  expect(settings.settings.resourceMonitorTools).toEqual(['ssh'])
  expect(sidebar.find('button').exists()).toBe(false)
  wrapper.unmount()
  sidebar.unmount()
})

it('采样失败保留旧值并标记中断；关闭后清空会话', async () => {
  const settings = useSettingsStore()
  const monitor = useResourceMonitorStore()
  await settings.set('resourceMonitorEnabled', true)
  await flushPromises()
  api.resourceMonitorSnapshot.mockRejectedValueOnce(new Error('进程不可读'))
  await vi.advanceTimersByTimeAsync(1000)
  expect(monitor.error).toContain('进程不可读')
  expect(monitor.totals?.memory).toBe(1024 ** 2)
  expect(monitor.updatedAt).toBeDefined()
  await settings.set('resourceMonitorEnabled', false)
  await flushPromises()
  expect(monitor.totals).toBeUndefined()
  expect(monitor.peakMemory).toBeNull()
})

it('保存失败回滚开关并只在分区显示错误', async () => {
  const settings = useSettingsStore()
  const wrapper = mount(ResourceMonitorSettings, { global: { plugins: [pinia] } })
  await flushPromises()
  api.settingsPatch.mockRejectedValueOnce(new Error('磁盘只读'))
  await wrapper.find('[role="switch"]').trigger('click')
  await flushPromises()
  expect(settings.settings.resourceMonitorEnabled).toBe(false)
  expect(wrapper.find('[role="alert"]').text()).toContain('磁盘只读')
  expect(settings.saveError).toBeNull()
  wrapper.unmount()
})

it('私有工作集不可用时不混用完整工作集或抬高峰值', async () => {
  const settings = useSettingsStore()
  const monitor = useResourceMonitorStore()
  await settings.set('resourceMonitorEnabled', true)
  await flushPromises()
  api.resourceMonitorSnapshot.mockResolvedValue({
    ...value,
    processes: value.processes.map((p) => ({ ...p, privateResidentBytes: null })),
  })
  await vi.advanceTimersByTimeAsync(1000)
  expect(monitor.totals?.memory).toBeNull()
  expect(monitor.peakMemory).toBe(1024 ** 2)
  const wrapper = mount(ResourceMonitor, { global: { plugins: [pinia] } })
  expect(wrapper.text()).toContain('不可用')
  expect(wrapper.text()).not.toContain('3 MiB')
  wrapper.unmount()
})

it('关闭且无活动的工具收紧为一行，在途请求与残留资源仍展示明细', async () => {
  const settings = useSettingsStore()
  const monitor = useResourceMonitorStore()
  await settings.set('resourceMonitorEnabled', true)
  await settings.set('resourceMonitorTools', ['idle', 'pending', 'retained'])
  await flushPromises()
  const empty = {
    requests: 10,
    failures: 0,
    completed: 10,
    totalMs: 100,
    inFlight: 0,
    scopes: 0,
    listeners: 0,
    timers: 0,
  }
  monitor.details = [
    { id: 'idle', ...empty },
    { id: 'pending', ...empty, inFlight: 1 },
    { id: 'retained', ...empty, listeners: 1 },
  ]
  const wrapper = mount(ResourceMonitor, { attachTo: document.body, global: { plugins: [pinia] } })
  await wrapper.find('button').trigger('click')
  await flushPromises()
  expect(document.querySelector('[data-tool="idle"]')?.textContent).not.toContain('请求')
  expect(document.querySelector('[data-tool="pending"]')?.textContent).toContain('在途 1')
  expect(document.querySelector('[data-tool="retained"]')?.textContent).toContain('订阅 1')
  const toggle = document.querySelector<HTMLElement>(
    '[role="dialog"] [role="button"][aria-controls]'
  )!
  const extra = document.getElementById(toggle.getAttribute('aria-controls')!)!
  expect(extra.style.display).toBe('none')
  toggle.click()
  await flushPromises()
  expect(extra.style.display).not.toBe('none')
  expect(extra.textContent).toContain('私有提交合计')
  wrapper.unmount()
})
