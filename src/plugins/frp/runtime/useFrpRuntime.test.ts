import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, onUnmounted, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createScope, scopeStats } from '@/core/lifecycle/scope'
import { useFrpRuntime } from './useFrpRuntime'
import type { FrpLogPayload, FrpRuntimeState } from '../contracts'

const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  start: vi.fn(),
  listen: vi.fn(),
  toast: vi.fn(),
}))
vi.mock('../ipc', () => ({ ipc: { status: mocks.status, start: mocks.start } }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mocks.toast }) }))
vi.mock('@/core/lifecycle', () => ({
  throttledInterval: (normal: number, hidden: number, inactive: boolean) =>
    inactive ? hidden : normal,
  useToolLifecycle: () => {
    const scope = createScope('frp')
    onUnmounted(() => void scope.dispose())
    return { scope, visibility, running: ref(false) }
  },
}))
const visibility = ref({ active: true, covered: false, hidden: false })

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => (resolve = done))
  return { promise, resolve }
}

function setup() {
  let runtime!: ReturnType<typeof useFrpRuntime>
  const wrapper = mount(
    defineComponent({
      setup() {
        runtime = useFrpRuntime()
        return () => null
      },
    })
  )
  return { wrapper, runtime }
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.status.mockResolvedValue([])
  mocks.listen.mockImplementation(async () => vi.fn())
  visibility.value = { active: true, covered: false, hidden: false }
})
afterEach(() => vi.restoreAllMocks())

it('单次日志事件同步状态和完整错误，重复行不丢弃，恢复立即清错误，兼容旧事件', async () => {
  const { wrapper, runtime } = setup()
  await flushPromises()
  const receive = mocks.listen.mock.calls.find(([name]) => name === 'frp://log')![1] as (event: {
    payload: FrpLogPayload
  }) => void
  const line = 'start error: ' + 'x'.repeat(70000)
  try {
    const payload: FrpLogPayload = {
      fileName: 'test.toml',
      line,
      stream: 'stderr',
      ts: 1,
      state: { fileName: 'test.toml', state: 'error', pid: 7 },
      lastErrorFromLine: true,
    }
    receive({ payload })
    receive({ payload })
    expect(runtime.logsOf('test.toml')).toHaveLength(2)
    expect(runtime.logsOf('test.toml')[0].line).toBe(line)
    expect(runtime.stateOf('test.toml')).toMatchObject({
      state: 'error',
      pid: 7,
      lastLine: line,
      lastError: line,
    })
    receive({
      payload: {
        fileName: 'test.toml',
        line: 'login to server success',
        stream: 'stdout',
        ts: 2,
        state: { fileName: 'test.toml', state: 'running', pid: 7 },
      },
    })
    expect(runtime.stateOf('test.toml')).toMatchObject({ state: 'running', lastError: undefined })
    receive({ payload: { fileName: 'test.toml', line: 'legacy', stream: 'stdout', ts: 3 } })
    expect(runtime.logsOf('test.toml')).toHaveLength(4)
  } finally {
    wrapper.unmount()
  }
  receive({ payload: { fileName: 'test.toml', line, stream: 'stderr', ts: 4 } })
  expect(runtime.logsOf('test.toml')).toEqual([])
})

it('首个订阅未完成就关闭时立即解绑迟到订阅，不启动查询和后续监听', async () => {
  const baseline = scopeStats()
  const pending = deferred<() => void>()
  const unlisten = vi.fn()
  mocks.listen.mockReturnValueOnce(pending.promise)
  const { wrapper } = setup()
  wrapper.unmount()
  pending.resolve(unlisten)
  await flushPromises()
  expect(unlisten).toHaveBeenCalledOnce()
  expect(mocks.listen).toHaveBeenCalledTimes(1)
  expect(mocks.status).not.toHaveBeenCalled()
  expect(scopeStats()).toMatchObject({
    live: baseline.live,
    listeners: baseline.listeners,
    timers: baseline.timers,
  })
})

it('查询晚到时不恢复已关闭状态；恢复可见也不再唤醒旧实例', async () => {
  const baseline = scopeStats()
  const pending = deferred<FrpRuntimeState[]>()
  mocks.status.mockReturnValueOnce(pending.promise)
  const { wrapper, runtime } = setup()
  await flushPromises()
  expect(mocks.status).toHaveBeenCalledOnce()
  wrapper.unmount()
  pending.resolve([{ fileName: 'test.toml', state: 'running', pid: 123 }])
  await flushPromises()
  visibility.value = { active: false, covered: false, hidden: false }
  await flushPromises()
  visibility.value = { active: true, covered: false, hidden: false }
  await flushPromises()
  expect(runtime.states.value).toEqual({})
  expect(mocks.status).toHaveBeenCalledOnce()
  expect(scopeStats()).toMatchObject({
    live: baseline.live,
    listeners: baseline.listeners,
    timers: baseline.timers,
  })
})

it('正常挂载后关闭释放轮询与可见性监听，重复启停只提交一次', async () => {
  const baseline = scopeStats()
  const pending = deferred<FrpRuntimeState>()
  mocks.start.mockReturnValueOnce(pending.promise)
  const { wrapper, runtime } = setup()
  await flushPromises()
  expect(scopeStats().timers).toBe(baseline.timers + 1)
  const first = runtime.start('test.toml')
  await runtime.start('test.toml')
  expect(mocks.start).toHaveBeenCalledOnce()
  wrapper.unmount()
  pending.resolve({ fileName: 'test.toml', state: 'running' })
  await first
  visibility.value = { active: false, covered: false, hidden: false }
  await flushPromises()
  visibility.value = { active: true, covered: false, hidden: false }
  await flushPromises()
  expect(runtime.busy.value).toEqual({})
  expect(mocks.toast).not.toHaveBeenCalled()
  expect(mocks.status).toHaveBeenCalledOnce()
  expect(scopeStats()).toMatchObject({
    live: baseline.live,
    listeners: baseline.listeners,
    timers: baseline.timers,
  })
})
