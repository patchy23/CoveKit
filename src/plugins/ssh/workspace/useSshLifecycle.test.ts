import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { useSshLifecycle, type SshLifecyclePorts } from './useSshLifecycle'

const mock = vi.hoisted(() => ({
  onConnectionStatus: vi.fn(),
  onTerminalData: vi.fn(),
  onTransferProgress: vi.fn(),
  onConnectStage: vi.fn(),
  onHostKeyVerify: vi.fn(),
  toast: vi.fn(),
}))
vi.mock('../ipc', () => mock)
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mock.toast }) }))
vi.mock('@/stores/settings', () => ({
  useSettingsStore: () => ({
    getToolSetting: (_owner: string, key: string) => (key === 'idleDisconnectV2' ? true : '0'),
  }),
}))

const subscriptions = [
  mock.onConnectionStatus,
  mock.onTerminalData,
  mock.onTransferProgress,
  mock.onConnectStage,
  mock.onHostKeyVerify,
]
let stops: ReturnType<typeof vi.fn<() => void>>[]
let wrapper: VueWrapper | undefined
beforeEach(() => {
  vi.resetAllMocks()
  // Vue 首次创建 renderer 会等待 DevTools 注入；提供空 hook，计数仅观察业务定时器。
  vi.stubGlobal('__VUE_DEVTOOLS_GLOBAL_HOOK__', { emit: vi.fn() })
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-09-26T00:00:00Z'))
  stops = subscriptions.map((subscribe) => {
    const stop = vi.fn()
    subscribe.mockResolvedValue(stop)
    return stop
  })
})
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function setup() {
  const row = {
    id: 'workspace',
    connection: { profileId: 'profile', sessionId: 'session', status: 'connected' as const },
    lastActivityAt: Date.now(),
  }
  const rows = ref<ReturnType<SshLifecyclePorts['listWorkspaces']>>([row])
  const ports: SshLifecyclePorts = {
    listWorkspaces: vi.fn(() => rows.value),
    onConnectionStatusEvent: vi.fn(),
    onConnectStageEvent: vi.fn(),
    onHostKeyEnqueue: vi.fn(),
    onActivity: vi.fn(),
    onIdleExpired: vi.fn(),
    onDispose: vi.fn(),
  }
  wrapper = mount(
    defineComponent({
      setup() {
        useSshLifecycle(ports)
        return () => h('div')
      },
    })
  )
  return { rows, row, ports }
}

it('第二条订阅失败仍保有第一条的释放责任，其他事件继续可用', async () => {
  mock.onTransferProgress.mockRejectedValue(new Error('传输订阅失败'))
  mock.onHostKeyVerify.mockRejectedValue(new Error('密钥订阅失败'))
  const { ports } = setup()
  await flushPromises()
  mock.onConnectStage.mock.calls[0][0]({ profileId: 'profile' })
  expect(ports.onConnectStageEvent).toHaveBeenCalledTimes(1)
  expect(mock.toast).toHaveBeenCalledTimes(2)
  wrapper!.unmount()
  wrapper = undefined
  for (const index of [0, 1, 3]) expect(stops[index]).toHaveBeenCalledTimes(1)
  expect(ports.onDispose).toHaveBeenCalledTimes(1)
  expect(vi.getTimerCount()).toBe(0)
})

it('订阅尚未完成时关闭立即释放已有监听，迟到句柄只释放一次且不继续初始化', async () => {
  let resolve!: (stop: () => void) => void
  mock.onTransferProgress.mockImplementationOnce(
    () =>
      new Promise((yes) => {
        resolve = yes
      })
  )
  const { ports } = setup()
  await flushPromises()
  wrapper!.unmount()
  wrapper = undefined
  expect(stops[1]).toHaveBeenCalledTimes(1)
  mock.onTerminalData.mock.calls[0][0]({ connectionId: 'session' })
  mock.onConnectionStatus.mock.calls[0][0]({ sessionId: 'session' })
  expect(ports.onActivity).not.toHaveBeenCalled()
  expect(ports.onConnectionStatusEvent).not.toHaveBeenCalled()
  resolve(stops[2])
  await flushPromises()
  expect(stops[2]).toHaveBeenCalledTimes(1)
  expect(mock.onConnectStage).not.toHaveBeenCalled()
  expect(vi.getTimerCount()).toBe(0)
})

it('关闭会话清除活动节流记录，晚到事件不重新持有旧会话且正常活动节流不变', async () => {
  const { rows, row, ports } = setup()
  await flushPromises()
  const output = mock.onTerminalData.mock.calls[0][0]
  output({ connectionId: 'session' })
  output({ connectionId: 'session' })
  expect(ports.onActivity).toHaveBeenCalledTimes(1)
  rows.value[0].lastActivityAt++
  expect(ports.listWorkspaces).toHaveBeenCalledTimes(1)
  rows.value = []
  output({ connectionId: 'session' })
  expect(ports.onActivity).toHaveBeenCalledTimes(1)
  rows.value = [row]
  output({ connectionId: 'session' })
  expect(ports.onActivity).toHaveBeenCalledTimes(2)
  vi.advanceTimersByTime(5000)
  output({ connectionId: 'session' })
  expect(ports.onActivity).toHaveBeenCalledTimes(3)
})

it('某个退订失败仍清理剩余订阅和定时器，并报告失败', async () => {
  stops[1].mockImplementation(() => {
    throw new Error('退订失败')
  })
  const { ports } = setup()
  await flushPromises()
  wrapper!.unmount()
  wrapper = undefined
  for (const stop of stops) expect(stop).toHaveBeenCalledTimes(1)
  expect(mock.toast).toHaveBeenCalledWith(expect.stringContaining('退订失败'))
  expect(ports.onDispose).toHaveBeenCalledTimes(1)
  expect(vi.getTimerCount()).toBe(0)
})

it('连续挂载关闭五十轮后订阅全部释放且没有活动定时器', async () => {
  for (let round = 0; round < 50; round++) {
    setup()
    await flushPromises()
    wrapper!.unmount()
    wrapper = undefined
  }
  for (const stop of stops) expect(stop).toHaveBeenCalledTimes(50)
  expect(vi.getTimerCount()).toBe(0)
})
