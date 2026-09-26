import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import RemoteEditorTree from './RemoteEditorTree.vue'
import type { RemoteFile } from '../contracts'
import { UiTree, UiInput } from '@/core/ui'

const list = vi.hoisted(() => vi.fn())
vi.mock('../ipc', () => ({ ipc: { sshFileList: list } }))
vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
vi.mock('@/core/ui', () => ({
  UiTree: { props: ['items'], emits: ['toggle', 'open'], template: '<div />' },
  UiToolbar: { template: '<div><slot /></div>' },
  UiInput: { props: ['modelValue'], emits: ['update:modelValue'], template: '<input />' },
  UiIconButton: { template: '<button><slot /></button>' },
  UiIcon: { template: '<span />' },
  UiAlert: { template: '<div><slot /></div>' },
  UiContextMenu: { template: '<div />' },
  UiInputDialog: { template: '<div />' },
}))
afterEach(() => {
  vi.restoreAllMocks()
  list.mockReset()
})
const file = (name: string, isDir = false): RemoteFile =>
  ({ name, path: '/' + name, isDir }) as RemoteFile

it('目录只在取得快照时排序，筛选和展开不重复排序，打开保留原始文件对象', async () => {
  const files = [file('z'), file('a'), file('folder', true)]
  list.mockResolvedValueOnce({ ok: true, files }).mockResolvedValueOnce({ ok: true, files: [] })
  const compare = vi.spyOn(String.prototype, 'localeCompare')
  const wrapper = mount(RemoteEditorTree, {
    props: { connected: true, connectionId: 'a', initialPath: '/' },
  })
  try {
    await flushPromises()
    const tree = wrapper.getComponent(UiTree)
    expect(tree.props('items').map((item: { label: string }) => item.label)).toEqual([
      'folder',
      'a',
      'z',
    ])
    const sortedCalls = compare.mock.calls.length
    expect(sortedCalls).toBeGreaterThan(0)
    const search = wrapper.findAllComponents(UiInput)[1]!
    search.vm.$emit('update:modelValue', 'Z')
    await wrapper.vm.$nextTick()
    expect(tree.props('items').map((item: { label: string }) => item.label)).toEqual([
      'folder',
      'z',
    ])
    tree.vm.$emit('toggle', { id: '/folder' })
    await flushPromises()
    tree.vm.$emit('open', { id: '/z' })
    expect(wrapper.emitted('open')?.[0]?.[0]).toBe(files.find((entry) => entry.name === 'z'))
    expect(compare).toHaveBeenCalledTimes(sortedCalls)
  } finally {
    wrapper.unmount()
  }
})

it('切换连接清空旧目录，旧读取晚到不能恢复旧缓存', async () => {
  let resolve!: (value: { ok: boolean; files: RemoteFile[] }) => void
  list
    .mockReturnValueOnce(
      new Promise((yes) => {
        resolve = yes
      })
    )
    .mockResolvedValueOnce({ ok: true, files: [file('current')] })
  const wrapper = mount(RemoteEditorTree, {
    props: { connected: true, connectionId: 'a', initialPath: '/' },
  })
  try {
    await wrapper.setProps({ connectionId: 'b' })
    await flushPromises()
    resolve({ ok: true, files: [file('expired')] })
    await flushPromises()
    expect(
      wrapper
        .getComponent(UiTree)
        .props('items')
        .map((item: { label: string }) => item.label)
    ).toEqual(['current'])
  } finally {
    wrapper.unmount()
  }
})
