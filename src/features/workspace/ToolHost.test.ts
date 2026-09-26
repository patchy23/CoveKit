import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { h } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import ToolHost from './ToolHost.vue'

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false }))
vi.mock('@tauri-apps/plugin-log', () => ({ warn: vi.fn() }))

let wrapper: VueWrapper | undefined
const loaded = { __esModule: true, default: { render: () => h('div', '工具内容') } }
beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {})
})
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  vi.restoreAllMocks()
  vi.useRealTimers()
})

it('两次加载失败后停止自动请求，人工重试开启独立的一轮并能恢复', async () => {
  // 第三次若被错误地自动调用会成功，避免旧实现无限重试挂住测试。
  const loader = vi
    .fn()
    .mockRejectedValueOnce(new Error('加载失败'))
    .mockRejectedValueOnce(new Error('加载失败'))
    .mockResolvedValue(loaded)
  wrapper = mount(ToolHost, { props: { toolId: 'test', title: '测试工具', loader } })
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).toContain('加载失败')
  loader.mockRejectedValueOnce(new Error('临时失败'))
  await wrapper.get('button').trigger('click')
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(4)
  expect(wrapper.text()).toContain('工具内容')
})

it('首次成功只加载一次，偶发失败只自动重试一次', async () => {
  const loader = vi.fn().mockResolvedValue(loaded)
  wrapper = mount(ToolHost, { props: { toolId: 'test', title: '测试工具', loader } })
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(1)
  expect(wrapper.text()).toContain('工具内容')
  wrapper.unmount()
  loader.mockClear().mockRejectedValueOnce(new Error('偶发失败'))
  wrapper = mount(ToolHost, { props: { toolId: 'test', title: '测试工具', loader } })
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).toContain('工具内容')
})

it('关闭工具后迟到的加载失败不再发起自动重试', async () => {
  let reject!: (error: Error) => void
  const loader = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((_, no) => {
          reject = no
        })
    )
    .mockResolvedValue(loaded)
  wrapper = mount(ToolHost, { props: { toolId: 'test', title: '测试工具', loader } })
  wrapper.unmount()
  wrapper = undefined
  reject(new Error('晚到失败'))
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(1)
})

it('超时后可人工恢复，旧一轮的迟到失败不重试或替换已恢复的工具', async () => {
  vi.useFakeTimers()
  let reject!: (error: Error) => void
  const loader = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((_, no) => {
          reject = no
        })
    )
    .mockResolvedValue(loaded)
  wrapper = mount(ToolHost, { props: { toolId: 'test', title: '测试工具', loader } })
  await vi.advanceTimersByTimeAsync(30000)
  expect(wrapper.text()).toContain('30000')
  await wrapper.get('button').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('工具内容')
  reject(new Error('旧一轮失败'))
  await flushPromises()
  expect(loader).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).toContain('工具内容')
})
