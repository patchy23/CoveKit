import { enableAutoUnmount, flushPromises, shallowMount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { UiButton } from '@/core/ui'
import MonitorTab from './MonitorTab.vue'
import MonitorSystemPanel from './MonitorSystemPanel.vue'
import MonitorTrend from './MonitorTrend.vue'
import { monitorTrendPath } from './monitorTrend'

const mock = vi.hoisted(() => ({ sshMonitorGet: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: mock }))
vi.mock('@/core/lifecycle', () => ({
  useToolScope: () => ({
    scope: { timeout: vi.fn() },
    visibility: { value: { active: true } },
    onResume: vi.fn(),
  }),
  throttledInterval: (visible: number) => visible,
}))
enableAutoUnmount(afterEach)
afterEach(() => vi.resetAllMocks())

const sample = {
  cpuPercent: 20,
  memoryPercent: 30,
  memoryUsed: 300,
  memoryTotal: 1000,
  diskPercent: 40,
  diskUsed: 400,
  diskTotal: 1000,
  netDownloadBps: 200,
  netUploadBps: 100,
  timestamp: 180000,
}
const create = () =>
  shallowMount(MonitorTab, {
    props: { connection: { profileId: 'server', sessionId: 'session', status: 'connected' } },
    global: { renderStubDefaultSlot: true },
  })

it('首次采样期间显示加载，系统与磁盘独立挂载；失败后可重试', async () => {
  let reject!: (error: Error) => void
  mock.sshMonitorGet.mockImplementationOnce(
    () =>
      new Promise((_, fail) => {
        reject = fail
      })
  )
  const wrapper = create()
  await flushPromises()
  expect(wrapper.get('[role="status"]').text()).toContain('正在采集')
  expect(wrapper.findComponent(MonitorSystemPanel).exists()).toBe(true)
  reject(new Error('timeout'))
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toContain('timeout')
  mock.sshMonitorGet.mockResolvedValueOnce(sample)
  wrapper.getComponent(UiButton).vm.$emit('click')
  await flushPromises()
  expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  expect(wrapper.findAllComponents(MonitorTrend)).toHaveLength(3)
  expect(wrapper.getComponent(MonitorSystemPanel).props('metrics')).toEqual(sample)
})

it('刷新失败保留旧曲线并明确提示，后续采样淘汰三分钟前的数据', async () => {
  mock.sshMonitorGet.mockResolvedValueOnce(sample)
  const wrapper = create()
  await flushPromises()
  mock.sshMonitorGet.mockRejectedValueOnce(new Error('offline'))
  wrapper.getComponent(UiButton).vm.$emit('click')
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toContain('保留上次采样数据')
  expect(wrapper.findAllComponents(MonitorTrend)[0].props('data')).toEqual(sample)
  const latest = { ...sample, timestamp: sample.timestamp + 181000 }
  mock.sshMonitorGet.mockResolvedValueOnce(latest)
  wrapper.getComponent(UiButton).vm.$emit('click')
  await flushPromises()
  expect(wrapper.findAllComponents(MonitorTrend)[0].props('history')).toEqual([latest])
})

it('曲线按采样时间落点，不把刚采到的两个点拉伸为三分钟历史', () => {
  expect(
    monitorTrendPath(
      [
        { timestamp: 177000, value: 25 },
        { timestamp: 180000, value: 50 },
      ],
      180000,
      100
    )
  ).toBe('M590.0,75.0 L600.0,50.0')
  expect(
    monitorTrendPath(
      [
        { timestamp: -1, value: 90 },
        { timestamp: 0, value: -5 },
        { timestamp: 90000, value: Number.NaN },
        { timestamp: 180000, value: 150 },
      ],
      180000,
      100
    )
  ).toBe('M0.0,100.0 M600.0,0.0')
})
