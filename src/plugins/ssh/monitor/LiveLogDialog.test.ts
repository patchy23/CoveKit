import { enableAutoUnmount, flushPromises, mount, shallowMount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { UiLogViewer } from '@/core/ui'
import LiveLogDialog from './LiveLogDialog.vue'
import { ipc } from '../ipc'

vi.mock('../ipc', () => ({ ipc: { sshDockerLogs: vi.fn(), sshServiceLogs: vi.fn() } }))
vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(ipc.sshDockerLogs).mockReset().mockResolvedValue({ ok: true, logs: 'docker' })
  vi.mocked(ipc.sshServiceLogs).mockReset().mockResolvedValue({ ok: true, logs: 'service' })
})
afterEach(() => vi.useRealTimers())

function mountLog(kind: 'docker' | 'service' = 'docker') {
  return shallowMount(LiveLogDialog, {
    props: { title: '日志', connectionId: 'connection', targetId: 'target', kind },
    global: { renderStubDefaultSlot: true },
  })
}

it.each(['docker', 'service'] as const)(
  '%s 日志请求使用所选上限，定时刷新沿用选择',
  async (kind) => {
    const command = kind === 'docker' ? ipc.sshDockerLogs : ipc.sshServiceLogs
    vi.mocked(command).mockResolvedValue({
      ok: true,
      logs: Array.from({ length: 350 }, (_, i) => `line-${i}`).join('\n') + '\n',
    })
    const wrapper = mountLog(kind)
    await flushPromises()
    expect(command).toHaveBeenLastCalledWith(expect.objectContaining({ lines: 300 }))
    expect(wrapper.getComponent(UiLogViewer).props('content')).toContain('line-349')
    wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 100)
    await flushPromises()
    expect(command).toHaveBeenLastCalledWith(expect.objectContaining({ lines: 100 }))
    // 显示上限由公共组件测试；此处验证请求档位和轮询职责。
    expect(wrapper.getComponent(UiLogViewer).props('content')).toContain('line-349')
    await vi.advanceTimersByTimeAsync(2000)
    expect(command).toHaveBeenCalledTimes(3)
    expect(command).toHaveBeenLastCalledWith(expect.objectContaining({ lines: 100 }))
  }
)

it('暂停显示不停止后台轮询；关闭窗口后停止轮询', async () => {
  const wrapper = mount(LiveLogDialog, {
    props: { title: '日志', connectionId: 'connection', targetId: 'target', kind: 'docker' },
    global: { stubs: { UiFloatingWindow: { template: '<div><slot /></div>' } } },
  })
  await flushPromises()
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '暂停显示')!
    .trigger('click')
  vi.mocked(ipc.sshDockerLogs).mockResolvedValue({ ok: true, logs: 'latest' })
  await vi.advanceTimersByTimeAsync(4000)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(3)
  expect(wrapper.get('pre').text()).toBe('docker')
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '继续显示')!
    .trigger('click')
  expect(wrapper.get('pre').text()).toBe('latest')
  wrapper.unmount()
  await vi.advanceTimersByTimeAsync(4000)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(3)
})

it('切换时串行补拉最新档位，关闭后不再补发请求', async () => {
  let resolve!: (value: { ok: boolean; logs: string }) => void
  vi.mocked(ipc.sshDockerLogs).mockReturnValueOnce(
    new Promise((done) => {
      resolve = done
    })
  )
  const wrapper = mountLog()
  wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 500)
  wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 1000)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(1)
  resolve({ ok: true, logs: 'stale' })
  await flushPromises()
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(2)
  expect(ipc.sshDockerLogs).toHaveBeenLastCalledWith(expect.objectContaining({ lines: 1000 }))
  expect(wrapper.getComponent(UiLogViewer).props('content')).toBe('docker')
  vi.mocked(ipc.sshDockerLogs).mockReturnValueOnce(
    new Promise((done) => {
      resolve = done
    })
  )
  wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 2000)
  wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 100)
  wrapper.unmount()
  resolve({ ok: true, logs: 'late' })
  await flushPromises()
  await vi.advanceTimersByTimeAsync(4000)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(3)
})
