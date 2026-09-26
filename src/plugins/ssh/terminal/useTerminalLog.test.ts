import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { useTerminalLog } from './useTerminalLog'

const mock = vi.hoisted(() => ({ listen: vi.fn(), toast: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mock.listen }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mock.toast }) }))
vi.mock('../ipc', () => ({ ipc: {} }))
let wrapper: VueWrapper | undefined
beforeEach(() => vi.resetAllMocks())
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
})
function setup() {
  wrapper = mount(
    defineComponent({
      setup() {
        useTerminalLog(() => 'terminal')
        return () => h('div')
      },
    })
  )
}

it('卸载后迟到的录制错误监听被释放，晚到事件不通知旧页面', async () => {
  let resolve!: (stop: () => void) => void
  mock.listen.mockImplementationOnce(
    () =>
      new Promise((yes) => {
        resolve = yes
      })
  )
  setup()
  const handler = mock.listen.mock.calls[0][1]
  wrapper!.unmount()
  wrapper = undefined
  const stop = vi.fn()
  resolve(stop)
  await flushPromises()
  handler({ payload: { terminalId: 'terminal', message: '晚到错误' } })
  expect(stop).toHaveBeenCalledTimes(1)
  expect(mock.toast).not.toHaveBeenCalled()
})

it('正常录制错误按终端过滤，关闭后解绑一次', async () => {
  const stop = vi.fn()
  mock.listen.mockResolvedValue(stop)
  setup()
  await flushPromises()
  const handler = mock.listen.mock.calls[0][1]
  handler({ payload: { terminalId: 'other', message: '其他终端' } })
  expect(mock.toast).not.toHaveBeenCalled()
  handler({ payload: { terminalId: 'terminal', message: '写入失败' } })
  expect(mock.toast).toHaveBeenCalledTimes(1)
  wrapper!.unmount()
  wrapper = undefined
  expect(stop).toHaveBeenCalledTimes(1)
})

it('订阅失败可见且没有未处理的 Promise 拒绝', async () => {
  mock.listen.mockRejectedValue(new Error('事件通道不可用'))
  setup()
  await flushPromises()
  expect(mock.toast).toHaveBeenCalledWith(expect.stringContaining('事件通道不可用'))
})

it('退订失败可见且不向 Vue 卸载过程抛出错误', async () => {
  mock.listen.mockResolvedValue(() => {
    throw new Error('解绑失败')
  })
  setup()
  await flushPromises()
  expect(() => wrapper!.unmount()).not.toThrow()
  wrapper = undefined
  expect(mock.toast).toHaveBeenCalledWith(expect.stringContaining('退订失败'))
})
