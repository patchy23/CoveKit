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

it('大目录首字母定位使用完整模型，离屏拖拽源在松手前保持节点', async () => {
  const prototype = files()[0]!
  const source = Array.from({ length: 5000 }, (_, index) => ({
    ...prototype,
    path: `/file-${index}`,
    name: `file-${index}`,
  }))
  source[4999]!.name = 'z-last'
  const wrapper = mount(FileBrowser, {
    props: { currentPath: '/', files: source, sortKey: 'name', sortDirection: 'asc' },
    global,
  })
  const root = wrapper.get('[aria-label="远程文件列表，输入首字母可循环定位"]')
  expect(wrapper.findAll('[data-file-row]').length).toBeLessThan(100)
  const first = wrapper.get('[data-file-row="/file-0"]')
  await first.trigger('pointerdown', { button: 0 })
  ;(root.element as HTMLElement).scrollTop = 35 * 4000
  await root.trigger('scroll')
  expect(wrapper.get('[data-file-row="/file-0"]').element).toBe(first.element)
  document.dispatchEvent(new PointerEvent('pointerup'))
  await nextTick()
  expect(wrapper.find('[data-file-row="/file-0"]').exists()).toBe(false)
  const scroll = vi.fn()
  Object.defineProperty(root.element, 'scrollTo', { configurable: true, value: scroll })
  await root.trigger('keydown', { key: 'z' })
  expect(toRaw(wrapper.emitted('rowClick')?.at(-1)?.[1])).toBe(source[4999])
  expect(scroll).toHaveBeenCalledWith({ top: 4999 * 35, behavior: 'smooth' })
  expect(wrapper.findAll('[data-file-row]').length).toBeLessThan(100)
})

it('本地大目录按完整模型测量长文件名，切换目录取消旧测量，卸载释放辅助节点', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  const frames: FrameRequestCallback[] = []
  const schedule = vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback)
    return frames.length
  })
  const rect = vi
    .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
    .mockImplementation(function (this: HTMLElement) {
      return new DOMRect(
        0,
        0,
        (this.textContent?.length ?? 0) * 8,
        this.tagName === 'THEAD' ? 33 : 0
      )
    })
  try {
    const first = files()[0]!
    const source = Array.from({ length: 1000 }, (_, index) => ({
      ...first,
      path: `/file-${index}`,
      name: index === 999 ? '很长的完整文件名'.repeat(100) : `file-${index}`,
    }))
    env.read.mockResolvedValue({ ok: true, path: '/', parentPath: null, files: source })
    const wrapper = mount(LocalBrowser, { props: { initialPath: '/' }, global })
    await flushPromises()
    expect(wrapper.findAll('[data-file-row]').length).toBeLessThan(100)
    while (frames.length) frames.shift()!(0)
    await nextTick()
    expect(parseFloat((wrapper.get('col').element as HTMLElement).style.width)).toBeGreaterThan(
      6000
    )
    // 最宽文件仍未挂载，列宽依据完整模型而不是这一屏。
    expect(wrapper.find('[data-file-row="/file-999"]').exists()).toBe(false)
    env.time.mockClear()
    window.dispatchEvent(new Event('resize'))
    while (frames.length) frames.shift()!(0)
    await nextTick()
    expect(env.time).not.toHaveBeenCalled()
    const width = (wrapper.get('col').element as HTMLElement).style.width
    await vi.advanceTimersByTimeAsync(10 * 60 * 1000)
    expect((wrapper.get('col').element as HTMLElement).style.width).toBe(width)
    window.dispatchEvent(new Event('resize'))
    while (frames.length) frames.shift()!(0)
    await nextTick()
    expect(env.time).toHaveBeenCalledTimes(1000)
    const replacement = source.slice(0, 500).map((file) => ({ ...file, name: 'short' }))
    env.read.mockResolvedValue({ ok: true, path: '/next', parentPath: '/', files: replacement })
    await wrapper.vm.refresh()
    await flushPromises()
    while (frames.length) frames.shift()!(0)
    await nextTick()
    expect(parseFloat((wrapper.get('col').element as HTMLElement).style.width)).toBeLessThan(200)
    wrapper.unmount()
    expect(document.querySelector('[aria-hidden="true"][style*="visibility: hidden"]')).toBeNull()
  } finally {
    schedule.mockRestore()
    rect.mockRestore()
    vi.useRealTimers()
  }
})

it('跨行原生选区滚离视口后仍完整，清空选择后释放保留节点', async () => {
  const prototype = files()[0]!
  const source = Array.from({ length: 1000 }, (_, i) => ({
    ...prototype,
    path: `/file-${i}`,
    name: `name-${i}`,
  }))
  const wrapper = mount(FileBrowser, {
    attachTo: document.body,
    props: { currentPath: '/', files: source, sortKey: 'name', sortDirection: 'asc' },
    global,
  })
  const root = wrapper.get('[aria-label="远程文件列表，输入首字母可循环定位"]')
  const first = wrapper.get('[data-file-row="/file-0"]')
  const last = wrapper.get('[data-file-row="/file-5"]')
  const range = document.createRange()
  range.setStartBefore(first.element.firstChild!)
  range.setEndAfter(last.element.lastChild!)
  const selection = document.getSelection()!
  selection.removeAllRanges()
  selection.addRange(range)
  document.dispatchEvent(new Event('selectionchange'))
  await nextTick()
  const text = selection.toString()
  ;(root.element as HTMLElement).scrollTop = 35 * 900
  await root.trigger('scroll')
  expect(wrapper.get('[data-file-row="/file-0"]').element).toBe(first.element)
  expect(selection.toString()).toBe(text)
  expect(text).toContain('name-3')
  selection.removeAllRanges()
  document.dispatchEvent(new Event('selectionchange'))
  await nextTick()
  expect(wrapper.find('[data-file-row="/file-0"]').exists()).toBe(false)
  expect(wrapper.findAll('[data-file-row]').length).toBeLessThan(100)
})
