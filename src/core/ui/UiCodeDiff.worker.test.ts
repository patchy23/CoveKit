import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { EditorView } from '@codemirror/view'
import {
  acceptChunk,
  getChunks,
  getOriginalDoc,
  rejectChunk,
  setExternalChunks,
} from '@codemirror/merge'
import { Text } from '@codemirror/state'
import UiCodeDiff from './UiCodeDiff.vue'
import { computeDiff, restoreChunks, type DiffRequest } from './editor/diffComputation'

vi.mock('./UiScrollArea.vue', () => ({ default: { template: '<div><slot /></div>' } }))
vi.mock('./editor/languages', async (original) => ({
  ...(await original<typeof import('./editor/languages')>()),
  loadLanguage: vi.fn(async () => []),
}))

class TestWorker {
  static all: TestWorker[] = []
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  onmessageerror: (() => void) | null = null
  request!: DiffRequest & { id: number }
  terminate = vi.fn()
  constructor() {
    TestWorker.all.push(this)
  }
  postMessage(request: DiffRequest & { id: number }) {
    this.request = request
  }
  complete() {
    this.onmessage?.({
      data: { id: this.request.id, result: structuredClone(computeDiff(this.request)) },
    } as MessageEvent)
  }
}
enableAutoUnmount(afterEach)
afterEach(() => {
  vi.unstubAllGlobals()
  TestWorker.all = []
})
const tail = '\n共同正文'.repeat(30000)
function views(element: Element) {
  return [...element.querySelectorAll<HTMLElement>('.cm-editor')].map((node) =>
    EditorView.findFromDOM(node)!
  )
}

it.each(['split', 'unified'] as const)(
  '%s 后台期间可读新正文、拒绝旧块，结果不重建编辑器或选区',
  async (mode) => {
    vi.stubGlobal('Worker', TestWorker)
    const wrapper = mount(UiCodeDiff, {
      props: { original: '旧' + tail, modified: '新' + tail, mode, readonly: false },
    })
    await flushPromises()
    const before = views(wrapper.element)
    const modified = before.at(-1)!
    modified.dispatch({ selection: { anchor: 12, head: 17 } })
    expect(modified.state.doc.toString()).toBe('新' + tail)
    expect(getChunks(modified.state)?.chunks).toEqual([])
    expect(wrapper.text()).toContain('正在计算差异')
    expect(rejectChunk(modified, 0)).toBe(false)
    TestWorker.all[0]!.complete()
    await flushPromises()
    expect(views(wrapper.element)).toEqual(before)
    expect(modified.state.selection.main.anchor).toBe(12)
    expect(modified.state.selection.main.head).toBe(17)
    expect(getChunks(modified.state)?.chunks.length).toBeGreaterThan(0)
    if (mode === 'unified') {
      expect(acceptChunk(modified, 0)).toBe(true)
      expect(getOriginalDoc(modified.state).toString()).toBe('新' + tail)
    }
  }
)

it('版本替换终止旧工作器，迟到结果和旧快照不能操作新正文', async () => {
  vi.stubGlobal('Worker', TestWorker)
  const wrapper = mount(UiCodeDiff, {
    props: { original: '旧' + tail, modified: '新' + tail, mode: 'unified', readonly: false },
  })
  await flushPromises()
  const old = TestWorker.all[0]!
  const late = old.onmessage!
  const view = views(wrapper.element)[0]!
  const docA = getOriginalDoc(view.state)
  const docB = view.state.doc
  await wrapper.setProps({ modified: '另一版' + tail })
  await flushPromises()
  expect(old.terminate).toHaveBeenCalledOnce()
  late({ data: { id: old.request.id, result: computeDiff(old.request) } } as MessageEvent)
  expect(setExternalChunks(view, docA, docB, restoreChunks(computeDiff(old.request)))).toBe(false)
  expect(getChunks(view.state)?.chunks).toEqual([])
  expect(rejectChunk(view, 0)).toBe(false)
  TestWorker.all.at(-1)!.complete()
  await flushPromises()
  expect(rejectChunk(view, 0)).toBe(true)
  expect(view.state.doc.toString()).toBe('旧' + tail)
})

it('故障与取消可见，取消保留完整正文并释放工作器，关闭不接收迟到结果', async () => {
  vi.stubGlobal('Worker', TestWorker)
  const wrapper = mount(UiCodeDiff, {
    props: { original: tail, modified: '新' + tail, showStats: false },
  })
  await flushPromises()
  TestWorker.all[0]!.onerror?.({ message: '后台计算失败' } as ErrorEvent)
  await flushPromises()
  expect(wrapper.text()).toContain('后台计算失败')
  await wrapper.get('button').trigger('click')
  await flushPromises()
  const active = TestWorker.all.at(-1)!
  await wrapper.get('button').trigger('click')
  expect(active.terminate).toHaveBeenCalledOnce()
  expect(wrapper.text()).toContain('已取消')
  expect(views(wrapper.element).at(-1)!.state.doc.length).toBe(tail.length + 1)
  wrapper.unmount()
  expect(() => active.complete()).not.toThrow()
})

it('后台片段和精确性标记保持原算法语义', () => {
  const request = {
    original: '第一行\n删除\n共同',
    modified: '第一行\n新增\n共同',
    versionA: 3,
    versionB: 8,
  }
  const result = computeDiff(request)
  const restored = restoreChunks(structuredClone(result))
  expect(restored).toEqual(result.chunks)
  expect(restored[0]!.endA).toBe(result.chunks[0]!.toA - 1)
  expect(result).toMatchObject({
    versionA: 3,
    versionB: 8,
    stats: { added: 1, removed: 1, same: false },
  })
  expect(Text.of(request.original.split('\n')).length).toBe(request.original.length)
})
