import { enableAutoUnmount, flushPromises, mount, shallowMount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { UiLogViewer } from '@/core/ui'
import LiveLogDialog from './LiveLogDialog.vue'
import { ipc } from '../ipc'
import { readLogSnapshot } from './logRequests'

vi.mock('../ipc', () => ({ ipc: { sshDockerLogs: vi.fn(), sshServiceLogs: vi.fn() } }))
vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(ipc.sshDockerLogs).mockReset().mockResolvedValue({ ok: true, logs: 'docker' })
  vi.mocked(ipc.sshServiceLogs).mockReset().mockResolvedValue({ ok: true, logs: 'service' })
})
afterEach(() => vi.useRealTimers())

it.each(['docker', 'service'] as const)(
  '%s 多窗口共享相同在途读取，关闭其中一个不影响其他窗口，完成后重新读取',
  async (kind) => {
    const command = kind === 'docker' ? ipc.sshDockerLogs : ipc.sshServiceLogs
    let resolve!: (value: { ok: boolean; logs: string }) => void
    vi.mocked(command).mockReturnValueOnce(new Promise((done) => (resolve = done)))
    const first = mountLog(kind)
    const second = mountLog(kind)
    expect(command).toHaveBeenCalledTimes(1)
    first.unmount()
    resolve({ ok: true, logs: '共享完整正文' })
    await flushPromises()
    expect(second.getComponent(UiLogViewer).props('content')).toBe('共享完整正文')
    await vi.advanceTimersByTimeAsync(2000)
    expect(command).toHaveBeenCalledTimes(2)
  }
)

it('共享读取失败同时反馈，下一次刷新可以重试且不保留失败结果', async () => {
  let reject!: (error: Error) => void
  vi.mocked(ipc.sshDockerLogs).mockReturnValueOnce(new Promise((_done, fail) => (reject = fail)))
  const first = mountLog()
  const second = mountLog()
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(1)
  reject(new Error('连接已断开'))
  await flushPromises()
  for (const wrapper of [first, second]) {
    expect(wrapper.getComponent(UiLogViewer).props('error')).toContain('连接已断开')
  }
  second.getComponent(UiLogViewer).vm.$emit('refresh')
  await flushPromises()
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(2)
  expect(second.getComponent(UiLogViewer).props('content')).toBe('docker')
  expect(second.getComponent(UiLogViewer).props('error')).toBe('')
})

it('不同来源、连接、行数或正文基线不能共用无变化响应', async () => {
  let resolve!: (value: { ok: boolean; logs: string }) => void
  const result = new Promise<{ ok: boolean; logs: string }>((done) => (resolve = done))
  vi.mocked(ipc.sshDockerLogs).mockReturnValue(result)
  vi.mocked(ipc.sshServiceLogs).mockReturnValue(result)
  const first = readLogSnapshot('docker', 'c', 't', 300, 'base')
  const joined = readLogSnapshot('docker', 'c', 't', 300, 'base')
  const others = [
    readLogSnapshot('docker', 'c2', 't', 300, 'base'),
    readLogSnapshot('docker', 'c', 't2', 300, 'base'),
    readLogSnapshot('docker', 'c', 't', 100, 'base'),
    readLogSnapshot('docker', 'c', 't', 300, 'different'),
    readLogSnapshot('docker', 'c', 't', 300),
    readLogSnapshot('service', 'c', 't', 300, 'base'),
  ]
  expect(joined).toBe(first)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(6)
  expect(ipc.sshServiceLogs).toHaveBeenCalledOnce()
  resolve({ ok: true, logs: '完整正文' })
  await Promise.all([first, joined, ...others])
  await readLogSnapshot('docker', 'c', 't', 300, 'base')
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(7)
})

function mountLog(kind: 'docker' | 'service' = 'docker') {
  return shallowMount(LiveLogDialog, {
    props: { title: '日志', connectionId: 'connection', targetId: 'target', kind },
    global: { renderStubDefaultSlot: true },
  })
}

it('无变化确认保留完整正文并继续两秒轮询，档位变化不用旧基线', async () => {
  vi.mocked(ipc.sshDockerLogs)
    .mockResolvedValueOnce({ ok: true, logs: '原始正文\n', fingerprint: 'first', unchanged: false })
    .mockResolvedValueOnce({ ok: true, fingerprint: 'first', unchanged: true })
  const wrapper = mountLog()
  await flushPromises()
  await vi.advanceTimersByTimeAsync(2000)
  expect(ipc.sshDockerLogs).toHaveBeenCalledTimes(2)
  expect(ipc.sshDockerLogs).toHaveBeenLastCalledWith(
    expect.objectContaining({ previousFingerprint: 'first', lines: 300 })
  )
  expect(wrapper.getComponent(UiLogViewer).props('content')).toBe('原始正文\n')
  wrapper.getComponent(UiLogViewer).vm.$emit('limit-change', 2000)
  await flushPromises()
  expect(vi.mocked(ipc.sshDockerLogs).mock.lastCall![0]).not.toHaveProperty('previousFingerprint')
})

it('错误的无变化确认不会清空正文，随后请求重新读取完整内容', async () => {
  vi.mocked(ipc.sshDockerLogs)
    .mockResolvedValueOnce({ ok: true, logs: 'kept', fingerprint: 'first' })
    .mockResolvedValueOnce({ ok: true, fingerprint: 'wrong', unchanged: true })
    .mockResolvedValueOnce({ ok: true, logs: '', fingerprint: 'empty' })
  const wrapper = mountLog()
  await flushPromises()
  await vi.advanceTimersByTimeAsync(2000)
  expect(wrapper.getComponent(UiLogViewer).props('content')).toBe('kept')
  expect(wrapper.getComponent(UiLogViewer).props('error')).toContain('基线不一致')
  await vi.advanceTimersByTimeAsync(2000)
  expect(vi.mocked(ipc.sshDockerLogs).mock.lastCall![0]).not.toHaveProperty('previousFingerprint')
  expect(wrapper.getComponent(UiLogViewer).props('content')).toBe('')
  expect(wrapper.getComponent(UiLogViewer).props('error')).toBe('')
})

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
