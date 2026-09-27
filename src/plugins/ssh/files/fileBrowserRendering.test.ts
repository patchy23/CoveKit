import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { reactive, nextTick, toRaw } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { RemoteFile } from '../contracts'
import FileBrowser from './FileBrowser.vue'
import LocalBrowser from './LocalBrowser.vue'

const env = vi.hoisted(() => ({
  bytes: vi.fn((value: number) => `${value} B`),
  time: vi.fn((value: number) => `时间 ${value}`),
  read: vi.fn(),
}))
vi.mock('../connection/useSsh', () => ({ formatBytes: env.bytes, formatTime: env.time }))
vi.mock('../ipc', () => ({
  ipc: { sshLocalDefaultDirectory: async (path: string) => [path, null] },
}))
vi.mock('./useLocalDirectory', () => ({ useLocalDirectory: () => ({ read: env.read }) }))
enableAutoUnmount(afterEach)
beforeEach(() => vi.clearAllMocks())

const global = {
  stubs: {
    UiScrollArea: { inheritAttrs: false, template: '<slot />' },
    UiTooltip: { inheritAttrs: false, template: '<slot />' },
    RemotePathToolbar: true,
    PathBreadcrumbs: true,
  },
}
function files(): RemoteFile[] {
  return Array.from({ length: 100 }, (_, i) => ({
    path: `/file-${i}`,
    name: `file-${i}`,
    isDir: false,
    size: i,
    modifiedAt: i,
    permissions: 'rw-r--r--',
    owner: 'user',
  }))
}

it('远程表格进度更新不重算文件行，多选与元数据变化仍即时反映', async () => {
  const source = reactive(files())
  const wrapper = mount(FileBrowser, {
    props: { currentPath: '/', files: source, sortKey: 'name', sortDirection: 'asc' },
    global,
  })
  await nextTick()
  env.time.mockClear()
  env.bytes.mockClear()
  const firstRow = wrapper.get('tbody tr').element
  await wrapper.setProps({ transferStatus: '传输 50%' })
  expect(wrapper.text()).toContain('传输 50%')
  expect(env.time).not.toHaveBeenCalled()
  expect(env.bytes).not.toHaveBeenCalled()
  await wrapper.setProps({ selectedPaths: new Set(['/file-0']), selectedCount: 1 })
  expect(env.time).toHaveBeenCalledTimes(1)
  expect(wrapper.get('tbody tr').classes()).toContain('bg-tertiary-soft')
  await wrapper.setProps({ selectedPaths: new Set(['/file-1']) })
  expect(env.time).toHaveBeenCalledTimes(3)
  expect(wrapper.get('tbody tr').classes()).not.toContain('bg-tertiary-soft')
  expect(wrapper.get('tbody tr').element).toBe(firstRow)
  source[0].name = 'renamed'
  source[0].owner = 'new-owner'
  source[0].size = 777
  await nextTick()
  expect(wrapper.get('tbody tr').text()).toContain('renamed')
  expect(wrapper.get('tbody tr').text()).toContain('new-owner')
  expect(wrapper.get('tbody tr').text()).toContain('777 B')
  await wrapper.get('tbody tr').trigger('pointerdown')
  expect(wrapper.emitted('rowPointerDown')?.at(-1)?.[1]).toBe(source[0])
})

it('同路径的新快照和重排更新行事件目标，首字母定位仍覆盖完整目录', async () => {
  const source = files()
  const wrapper = mount(FileBrowser, {
    props: { currentPath: '/', files: source, sortKey: 'name', sortDirection: 'asc' },
    global,
  })
  const replacement = { ...source[0], name: 'target' }
  await wrapper.setProps({ files: [...source.slice(1).reverse(), replacement] })
  const last = wrapper.findAll('tbody tr').at(-1)!
  expect(last.text()).toContain('target')
  await last.trigger('dblclick')
  expect(toRaw(wrapper.emitted('open')?.at(-1)?.[0])).toBe(replacement)
  const viewport = wrapper.get('[aria-label="远程文件列表，输入首字母可循环定位"]')
  const scroll = vi.fn()
  Object.defineProperty(viewport.element, 'scrollTo', { configurable: true, value: scroll })
  await viewport.trigger('keydown', { key: 't' })
  expect(toRaw(wrapper.emitted('rowClick')?.at(-1)?.[1])).toBe(replacement)
  expect(scroll).toHaveBeenCalledOnce()
})

it('本地表格保留全量数据，多选只重算变化行且刷新后事件使用新文件', async () => {
  const source = files()
  env.read.mockResolvedValue({ ok: true, path: '/', parentPath: null, files: source })
  const wrapper = mount(LocalBrowser, { props: { initialPath: '/' }, global })
  await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(source.length)
  env.time.mockClear()
  await wrapper.setProps({ selectedPaths: new Set(['/file-0', '/file-99']) })
  expect(env.time).toHaveBeenCalledTimes(2)
  const rows = wrapper.findAll('tbody tr')
  expect(rows[0].classes()).toContain('bg-tertiary-soft')
  expect(rows[99].classes()).toContain('bg-tertiary-soft')
  await rows[99].trigger('contextmenu')
  expect(wrapper.emitted('rowContext')?.at(-1)?.[1]).toBe(source[99])
  const replacement = { ...source[0], name: 'new snapshot' }
  env.read.mockResolvedValue({ ok: true, path: '/', parentPath: null, files: [replacement] })
  await wrapper.vm.refresh()
  await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(1)
  await wrapper.get('tbody tr').trigger('click')
  expect(wrapper.emitted('rowClick')?.at(-1)?.[1]).toBe(replacement)
})
