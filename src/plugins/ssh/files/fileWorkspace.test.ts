import { enableAutoUnmount, flushPromises, mount, shallowMount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import FileManagerTab from './FileManagerTab.vue'
import ArchivePreview from './ArchivePreview.vue'
import type { ServerConnection } from '../contracts'
const env = vi.hoisted(() => ({
  list: vi.fn(async (_id: string, path: string) => ({ ok: true, path, files: [] })),
}))
vi.mock('../ipc', () => ({
  ipc: { sshFileList: env.list },
  onTransferProgress: async () => () => {},
}))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }),
}))
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
)
enableAutoUnmount(afterEach)
it('终端首次导航只读取目标目录，断线且没有导航请求也能挂载文件页', async () => {
  const connection = { sessionId: 's', status: 'connected' } as ServerConnection
  shallowMount(FileManagerTab, {
    props: { connection, navigation: { id: 1, sessionId: 's', path: '/srv/work' } },
    global: { plugins: [createPinia()] },
  })
  await flushPromises()
  expect(env.list).toHaveBeenCalledWith('s', '/srv/work')
  expect(env.list).not.toHaveBeenCalledWith('s', '/')
  expect(() =>
    shallowMount(FileManagerTab, {
      props: { connection: { ...connection, status: 'disconnected' } },
      global: { plugins: [createPinia()] },
    })
  ).not.toThrow()
})
it('压缩包预览分页并允许进入隐含的父目录', async () => {
  const wrapper = mount(ArchivePreview, {
    props: {
      path: '/a.zip',
      busy: false,
      error: '',
      entries: Array.from({ length: 450 }, (_, i) => ({
        path: 'folder/' + i + '.txt',
        size: 10,
        isDir: false,
        modifiedAt: 0,
      })),
    },
  })
  const folder = wrapper.findAll('button').find((button) => button.text() === 'folder/')!
  await folder.trigger('click')
  expect(wrapper.findAll('tbody tr')).toHaveLength(200)
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '下一页')!
    .trigger('click')
  expect(wrapper.findAll('tbody tr')).toHaveLength(200)
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '下一页')!
    .trigger('click')
  expect(wrapper.findAll('tbody tr')).toHaveLength(50)
})
