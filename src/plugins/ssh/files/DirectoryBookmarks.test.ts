import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import DirectoryBookmarks from './DirectoryBookmarks.vue'
const env = vi.hoisted(() => ({ list: vi.fn(), remove: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshBookmarkList: env.list, sshBookmarkDelete: env.remove } }))
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
)
enableAutoUnmount(afterEach)
it('行尾删除不导航、不关闭书签浮层', async () => {
  env.list.mockResolvedValue([{ id: 'b', name: '目录', path: '/srv' }])
  env.remove.mockImplementation(async () => {
    env.list.mockResolvedValue([])
  })
  const wrapper = mount(DirectoryBookmarks, {
    attachTo: document.body,
    props: { profileId: 'p', path: '/srv', open: false },
  })
  await flushPromises()
  await wrapper.setProps({ open: true })
  await flushPromises()
  const closes = wrapper.emitted('update:open')?.length ?? 0
  const button = document.querySelector<HTMLButtonElement>('[aria-label="删除书签"]')!
  expect(button).not.toBeNull()
  button.click()
  await flushPromises()
  expect(env.remove).toHaveBeenCalledWith('b')
  expect(wrapper.emitted('navigate')).toBeUndefined()
  expect(wrapper.emitted('update:open')?.length ?? 0).toBe(closes)
  expect(document.querySelector('[aria-label="目录书签"]')).not.toBeNull()
})
