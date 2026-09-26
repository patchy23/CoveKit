import { afterEach, describe, expect, it, vi } from 'vitest'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { search } from '@codemirror/search'
import { createSearchController } from './searchController'
import * as searchFunctions from './search'

const options = { caseSensitive: false, regexp: false, wholeWord: false }
const views: EditorView[] = []
afterEach(() => {
  views.splice(0).forEach((view) => view.destroy())
  vi.restoreAllMocks()
})

function setup(doc = 'foo Foo foobar') {
  const controller = createSearchController(() => view)
  const view = new EditorView({
    state: EditorState.create({
      doc,
      extensions: [
        search(),
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

  it('条件变化重新扫描，单独修改替换文本复用统计', () => {
    const { controller, scan } = setup()
    controller.apply('foo', '', options)
    controller.apply('foo', 'bar', options)
    expect(scan).toHaveBeenCalledTimes(1)
    controller.apply('foo', '', { ...options, caseSensitive: true })
    expect(controller.state.value.total).toBe(2)
    controller.apply('foo', '', { ...options, wholeWord: true })
    expect(controller.state.value.total).toBe(2)
    controller.apply('f.o', '', { ...options, regexp: true })
    expect(controller.state.value.total).toBe(3)
    expect(scan).toHaveBeenCalledTimes(4)
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

  it('非法正则错误可复用，修正查询后恢复', () => {
    const { view, controller, scan } = setup()
    controller.apply('([', '', { ...options, regexp: true })
    expect(controller.state.value.error).toContain('正则表达式无效')
    view.dispatch({ selection: { anchor: 1 } })
    expect(scan).toHaveBeenCalledTimes(1)
    controller.apply('foo', '', options)
    expect(controller.state.value.error).toBeUndefined()
    expect(controller.state.value.total).toBe(3)
  })
})
