import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { Facet, type Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { getOriginalDoc } from '@codemirror/merge'
import UiCodeDiff from './UiCodeDiff.vue'
import { loadLanguage } from './editor/languages'

vi.mock('./UiScrollArea.vue', () => ({ default: { template: '<div><slot /></div>' } }))
vi.mock('./editor/languages', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./editor/languages')>()),
  loadLanguage: vi.fn(async () => []),
}))

enableAutoUnmount(afterEach)
afterEach(() => {
  vi.restoreAllMocks()
  vi.mocked(loadLanguage).mockReset().mockResolvedValue([])
})

const marker = Facet.define<string, string>({ combine: (values) => values[0] ?? '' })

function editors(element: Element): EditorView[] {
  return [...element.querySelectorAll<HTMLElement>('.cm-editor')].map((node) => {
    const view = EditorView.findFromDOM(node)
    if (!view) throw new Error('未找到实际 CodeMirror 视图')
    return view
  })
}

it('切换语言复用两侧文档、视图和选区，不重复创建编辑器', async () => {
  const wrapper = mount(UiCodeDiff, {
    props: { original: 'old\nunchanged', modified: 'new\nunchanged', filename: 'a.txt' },
  })
  await flushPromises()
  const before = editors(wrapper.element)
  expect(before).toHaveLength(2)
  const documents = before.map((view) => view.state.doc)
  before[1]!.dispatch({ selection: { anchor: 5, head: 9 } })
  vi.mocked(loadLanguage).mockResolvedValueOnce(marker.of('json'))
  await wrapper.setProps({ filename: 'a.json' })
  await flushPromises()
  const after = editors(wrapper.element)
  expect(after[0]).toBe(before[0])
  expect(after[1]).toBe(before[1])
  after.forEach((view, i) => {
    expect(view.state.doc).toBe(documents[i])
    expect(view.state.facet(marker)).toBe('json')
  })
  expect(after[1]!.state.selection.main.anchor).toBe(5)
  expect(after[1]!.state.selection.main.head).toBe(9)
})

it('左右对照切换变更控件不重新加载语言或覆盖视图中的修改', async () => {
  const wrapper = mount(UiCodeDiff, { props: { original: 'old', modified: 'new' } })
  await flushPromises()
  const before = editors(wrapper.element)
  before[1]!.dispatch({ changes: { from: 0, to: 3, insert: 'local' } })
  const loadCount = vi.mocked(loadLanguage).mock.calls.length
  await wrapper.setProps({ readonly: false })
  expect(wrapper.find('.cm-merge-revert').exists()).toBe(true)
  expect(editors(wrapper.element)[1]).toBe(before[1])
  expect(before[1]!.state.doc.toString()).toBe('local')
  await wrapper.setProps({ readonly: true })
  expect(wrapper.find('.cm-merge-revert').exists()).toBe(false)
  expect(editors(wrapper.element)[1]).toBe(before[1])
  expect(loadLanguage).toHaveBeenCalledTimes(loadCount)
})

it('内联视图语言更新保留原文和当前文档', async () => {
  const wrapper = mount(UiCodeDiff, {
    props: { original: 'old', modified: 'new', mode: 'unified' },
  })
  await flushPromises()
  const before = editors(wrapper.element)[0]!
  const original = getOriginalDoc(before.state)
  const document = before.state.doc
  vi.mocked(loadLanguage).mockResolvedValueOnce(marker.of('xml'))
  await wrapper.setProps({ language: 'xml' })
  await flushPromises()
  expect(editors(wrapper.element)[0]).toBe(before)
  expect(before.state.doc).toBe(document)
  expect(getOriginalDoc(before.state)).toBe(original)
  expect(before.state.facet(marker)).toBe('xml')
})

it('异步语言加载期间保留内容，迟到加载不覆盖最新配置', async () => {
  const wrapper = mount(UiCodeDiff, { props: { original: 'old', modified: 'new' } })
  await flushPromises()
  const before = editors(wrapper.element)
  let resolveOld!: (extension: Extension) => void
  vi.mocked(loadLanguage).mockImplementationOnce(
    () => new Promise((resolve) => (resolveOld = resolve))
  )
  await wrapper.setProps({ language: 'json' })
  expect(editors(wrapper.element)[0]).toBe(before[0])
  vi.mocked(loadLanguage).mockResolvedValueOnce(marker.of('xml'))
  await wrapper.setProps({ language: 'xml' })
  await flushPromises()
  resolveOld(marker.of('json'))
  await flushPromises()
  expect(editors(wrapper.element)[0]).toBe(before[0])
  expect(before[0]!.state.facet(marker)).toBe('xml')
})

it('切换正文和形态仍展示最新完整内容，卸载拒绝迟到创建', async () => {
  const wrapper = mount(UiCodeDiff, { props: { original: 'old', modified: 'new' } })
  await flushPromises()
  await wrapper.setProps({ original: 'a\r\nb', modified: 'c\r\nd', mode: 'unified' })
  await flushPromises()
  const current = editors(wrapper.element)
  expect(current).toHaveLength(1)
  expect(current[0]!.state.doc.toString()).toBe('c\nd')
  expect(getOriginalDoc(current[0]!.state).toString()).toBe('a\nb')
  const destroy = vi.spyOn(current[0]!, 'destroy')
  let resolve!: (extension: Extension) => void
  vi.mocked(loadLanguage).mockImplementationOnce(() => new Promise((done) => (resolve = done)))
  await wrapper.setProps({ language: 'json' })
  const element = wrapper.element
  wrapper.unmount()
  resolve([])
  await flushPromises()
  expect(destroy).toHaveBeenCalledTimes(1)
  expect(editors(element)).toHaveLength(0)
})
