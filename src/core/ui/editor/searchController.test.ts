import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { search } from '@codemirror/search'
import { history, undo } from '@codemirror/commands'
import { createSearchController } from './searchController'
import * as searchFunctions from './search'
import { RegexTestWorker } from './regexSearch.test.worker'

const controllers: ReturnType<typeof createSearchController>[] = []
beforeEach(() => vi.stubGlobal('Worker', RegexTestWorker))

const options = { caseSensitive: false, regexp: false, wholeWord: false }
const views: EditorView[] = []
afterEach(() => {
  controllers.splice(0).forEach((controller) => controller.clear())
  views.splice(0).forEach((view) => view.destroy())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

function setup(doc = 'foo Foo foobar') {
  const controller = createSearchController(() => view)
  controllers.push(controller)
  const view = new EditorView({
    state: EditorState.create({
      doc,
      extensions: [
        search(),
        history(),
        EditorView.updateListener.of((update) => {
          if (update.docChanged || update.selectionSet) controller.refresh()
        }),
      ],
    }),
  })
  views.push(view)
  return { view, controller, scan: vi.spyOn(searchFunctions, 'scanMatches') }
}

describe('编辑器查找扫描复用', () => {
  it('大文档移动光标与连续跳转不再重复全文统计，序号仍实时更新', () => {
    const { view, controller, scan } = setup('foo ' + 'x'.repeat(200000) + ' foo')
    controller.apply('foo', '', options)
    expect(scan).toHaveBeenCalledTimes(1)
    for (let i = 0; i < 100; i++) view.dispatch({ selection: { anchor: i + 4 } })
    expect(controller.state.value).toEqual({ total: 2, current: 0 })
    controller.next()
    expect(controller.state.value).toEqual({ total: 2, current: 2 })
    controller.previous()
    expect(controller.state.value).toEqual({ total: 2, current: 1 })
    expect(scan).toHaveBeenCalledTimes(1)
  })

  it('条件变化重新扫描，单独修改替换文本复用统计', async () => {
    const { controller, scan } = setup()
    controller.apply('foo', '', options)
    controller.apply('foo', 'bar', options)
    expect(scan).toHaveBeenCalledTimes(1)
    controller.apply('foo', '', { ...options, caseSensitive: true })
    expect(controller.state.value.total).toBe(2)
    controller.apply('foo', '', { ...options, wholeWord: true })
    expect(controller.state.value.total).toBe(2)
    controller.apply('f.o', '', { ...options, regexp: true })
    await flushPromises()
    expect(controller.state.value.total).toBe(3)
    expect(scan).toHaveBeenCalledTimes(3)
  })

  it('编辑与全部替换立即更新计数，清空后不保留旧查询', () => {
    const { view, controller, scan } = setup('foo foo')
    controller.apply('foo', 'bar', options)
    view.dispatch({ changes: { from: 0, to: 3, insert: 'bar' } })
    expect(controller.state.value.total).toBe(1)
    expect(scan).toHaveBeenCalledTimes(2)
    controller.replaceAllMatches()
    expect(view.state.doc.toString()).toBe('bar bar')
    expect(controller.state.value.total).toBe(0)
    expect(scan).toHaveBeenCalledTimes(3)
    controller.clear()
    controller.apply('foo', '', options)
    expect(scan).toHaveBeenCalledTimes(4)
  })

  it('非法正则错误可复用，修正查询后恢复', async () => {
    const { view, controller, scan } = setup()
    controller.apply('([', '', { ...options, regexp: true })
    await flushPromises()
    expect(controller.state.value.error).toContain('正则表达式无效')
    view.dispatch({ selection: { anchor: 1 } })
    expect(scan).not.toHaveBeenCalled()
    controller.apply('foo', '', options)
    expect(controller.state.value.error).toBeUndefined()
    expect(controller.state.value.total).toBe(3)
  })

  it('正则导航与捕获组替换异步完成，全部替换仍可一次撤销', async () => {
    const { view, controller, scan } = setup('foo1 foo2 foo3')
    controller.apply('foo(\\d)', 'bar$1', { ...options, regexp: true })
    expect(controller.state.value.pending).toBe(true)
    await flushPromises()
    expect(view.state.selection.main.from).toBe(0)
    controller.next()
    controller.next()
    await flushPromises()
    expect(view.state.selection.main.from).toBe(10)
    controller.replaceCurrent()
    await flushPromises()
    expect(view.state.doc.toString()).toBe('foo1 foo2 bar3')
    controller.replaceAllMatches()
    await flushPromises()
    expect(view.state.doc.toString()).toBe('bar1 bar2 bar3')
    expect(scan).not.toHaveBeenCalled()
    undo(view)
    await flushPromises()
    expect(view.state.doc.toString()).toBe('foo1 foo2 bar3')
  })

  it('计算期间更换文档或关闭查询，旧结果不修改选区和正文', async () => {
    const { view, controller } = setup('foo foo')
    controller.apply('foo', 'bar', { ...options, regexp: true })
    controller.replaceAllMatches()
    view.dispatch({ changes: { from: 0, to: 7, insert: 'new document' } })
    controller.clear()
    await flushPromises()
    expect(view.state.doc.toString()).toBe('new document')
    expect(controller.state.value).toEqual({ total: 0, current: 0 })
  })

  it('计算期间用户移动选区，旧单次替换不改写新的选区内容', async () => {
    const { view, controller } = setup('foo foo')
    controller.apply('foo', 'bar', { ...options, regexp: true })
    await flushPromises()
    controller.replaceCurrent()
    await Promise.resolve()
    view.dispatch({ selection: { anchor: 4, head: 7 } })
    await flushPromises()
    expect(view.state.doc.toString()).toBe('foo foo')
    expect(view.state.selection.main.from).toBe(4)
  })
})
