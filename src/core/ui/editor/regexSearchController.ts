import { StateEffect, StateField, type Text } from '@codemirror/state'
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view'
import type { EditorSearchState } from './searchController'
import { matchIndexOf, type MatchScan, type SearchOptions } from './search'
import { createRegexClient } from './regexSearchClient'
import type { RegexAction, SearchRange } from './regexSearch'

const updateHighlights = StateEffect.define<SearchRange[]>()
const highlights = StateField.define<{ ranges: SearchRange[]; decorations: DecorationSet }>({
  create: () => ({ ranges: [], decorations: Decoration.none }),
  update(value, transaction) {
    let ranges = transaction.docChanged ? [] : value.ranges
    for (const effect of transaction.effects) if (effect.is(updateHighlights)) ranges = effect.value
    if (ranges === value.ranges && !transaction.selection) return value
    const selection = transaction.state.selection.main
    return {
      ranges,
      decorations: Decoration.set(
        ranges
          .filter((range) => range.from < range.to)
          .map((range) =>
            Decoration.mark({
              class:
                range.from === selection.from && range.to === selection.to
                  ? 'cm-searchMatch cm-searchMatch-selected'
                  : 'cm-searchMatch',
            }).range(range.from, range.to)
          ),
        true
      ),
    }
  },
  provide: (field) => EditorView.decorations.from(field, (value) => value.decorations),
})

/** 正则统计、导航、高亮、替换都离开主线程；不能只搬走计数而留下同步正则。 */
export function createRegexController(
  getView: () => EditorView | null,
  readText: (document: Text) => string,
  publish: (state: EditorSearchState) => void,
  afterReplace?: () => void
) {
  const client = createRegexClient(readText)
  let input: { query: string; replacement: string; options: SearchOptions } | undefined
  let key = ''
  let document: Text | undefined
  let scan: MatchScan | undefined
  let generation = 0
  let running: object | undefined
  let queue: RegexAction[] = []
  let scheduled = false
  let applying = false

  function status() {
    const view = getView()
    publish({
      total: scan?.positions.length ?? 0,
      current: scan && view ? matchIndexOf(scan, view.state.selection.main.from) : 0,
      ...(scan?.error ? { error: scan.error } : {}),
      ...(running || queue.length ? { pending: true } : {}),
    })
  }
  function invalidate(keepQueue = false, release = false) {
    generation++
    // 空闲 Worker 可直接更换条件并复用文档；仅中断在途计算或关闭时销毁。
    if (running || release) client.reset()
    running = undefined
    if (!keepQueue) queue = []
    scan = undefined
  }
  function schedule() {
    if (scheduled) return
    scheduled = true
    queueMicrotask(() => {
      scheduled = false
      void pump()
    })
  }
  function request(action: RegexAction) {
    if (!input || !getView()) return
    if (action !== 'scan' || !queue.includes('scan')) queue.push(action)
    status()
    schedule()
  }
  async function pump() {
    if (running || !queue.length || !input) return
    const view = getView()
    if (!view) return
    const job = {}
    running = job
    const version = generation
    const doc = view.state.doc
    const action = queue.shift()!
    const selection = view.state.selection.main
    try {
      const result = await client.run(doc, {
        ...input,
        action,
        selection: { from: selection.from, to: selection.to },
        viewport: view.viewport,
        wordChars: view.state.languageDataAt<string>('wordChars', selection.head).join(''),
      })
      if (version !== generation || getView() !== view || view.state.doc !== doc) return
      scan = result.scan
      const sameSelection = view.state.selection.main === selection
      const canReplace = !view.state.readOnly && (action === 'replaceAll' || sameSelection)
      const changes = canReplace ? result.changes : undefined
      const selected =
        sameSelection && (!result.changes?.length || canReplace) ? result.selection : undefined
      applying = true
      try {
        if (!view.state.field(highlights, false))
          view.dispatch({ effects: StateEffect.appendConfig.of(highlights) })
        view.dispatch({
          changes,
          selection: selected ? { anchor: selected.from, head: selected.to } : undefined,
          effects: updateHighlights.of(changes?.length ? [] : result.highlights),
          scrollIntoView: Boolean(selected),
          userEvent: changes?.length
            ? action === 'replaceAll'
              ? 'input.replace.all'
              : 'input.replace'
            : 'select.search',
        })
      } finally {
        applying = false
      }
      if (changes?.length) afterReplace?.()
    } catch (error) {
      if (version === generation && !(error instanceof DOMException && error.name === 'AbortError'))
        scan = { positions: [], lengths: [], truncated: false, error: `查找失败：${String(error)}` }
    } finally {
      if (running === job) running = undefined
      if (input) status()
      schedule()
    }
  }
  function apply(next: NonNullable<typeof input>, jump: boolean) {
    const nextKey = JSON.stringify([next.query, next.options])
    const changed = key !== nextKey || document !== getView()?.state.doc
    input = next
    key = nextKey
    if (!changed) return
    invalidate()
    const view = getView()
    if (view?.state.field(highlights, false)) view.dispatch({ effects: updateHighlights.of([]) })
    document = getView()?.state.doc
    request(jump ? 'next' : 'scan')
  }
  function refresh(viewport = false) {
    if (!input) return
    const doc = getView()?.state.doc
    if (doc !== document) {
      invalidate(applying)
      document = doc
      request('scan')
    } else {
      status()
      if (viewport) request('scan')
    }
  }
  function clear() {
    input = undefined
    key = ''
    document = undefined
    invalidate(false, true)
    const view = getView()
    if (view?.state.field(highlights, false)) view.dispatch({ effects: updateHighlights.of([]) })
  }
  return { apply, refresh, clear, request }
}
