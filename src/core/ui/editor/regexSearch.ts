/** 正则查找的 Worker 计算域；光标、全词和零长匹配沿用 CodeMirror 的公开游标。 */
import { EditorSelection, EditorState } from '@codemirror/state'
import { SearchQuery } from '@codemirror/search'
import { MAX_MATCHES, type MatchScan, type SearchOptions } from './search'

export type RegexAction = 'scan' | 'next' | 'previous' | 'replace' | 'replaceAll'
export interface SearchRange {
  from: number
  to: number
}
export interface RegexRequest {
  id: number
  document?: string
  query: string
  replacement: string
  options: SearchOptions
  wordChars: string
  action: RegexAction
  selection: SearchRange
  viewport: SearchRange
}
export interface RegexResponse {
  id: number
  scan: MatchScan
  highlights: SearchRange[]
  selection?: SearchRange
  changes?: (SearchRange & { insert: string })[]
}

export function createRegexEngine() {
  let state = EditorState.create()
  let wordChars = ''
  let cached: { key: string; scan: MatchScan } | undefined
  return (request: RegexRequest): RegexResponse => {
    if (request.document !== undefined || request.wordChars !== wordChars) {
      wordChars = request.wordChars
      state = EditorState.create({
        doc: request.document ?? state.doc,
        extensions: EditorState.languageData.of(() => [{ wordChars }]),
      })
      cached = undefined
    }
    const query = new SearchQuery({
      search: request.query,
      replace: request.replacement,
      ...request.options,
    })
    const response: RegexResponse = {
      id: request.id,
      scan: { positions: [], lengths: [], truncated: false },
      highlights: [],
    }
    if (!query.valid) {
      response.scan.error = '正则表达式无效'
      return response
    }
    const key = JSON.stringify([request.query, request.options])
    if (cached?.key !== key) {
      const scan: MatchScan = { positions: [], lengths: [], truncated: false }
      const cursor = query.getCursor(state)
      while (!cursor.next().done) {
        scan.positions.push(cursor.value.from)
        scan.lengths.push(cursor.value.to - cursor.value.from)
        if (scan.positions.length === MAX_MATCHES) {
          scan.truncated = true
          break
        }
      }
      cached = { key, scan }
    }
    response.scan = cached.scan
    const cursor = query.getCursor(
      state,
      Math.max(0, request.viewport.from - 250),
      Math.min(state.doc.length, request.viewport.to + 250)
    )
    while (!cursor.next().done)
      response.highlights.push({ from: cursor.value.from, to: cursor.value.to })
    function next(from: number, to: number) {
      let cursor = query.getCursor(state, to).next()
      if (cursor.done) cursor = query.getCursor(state, 0, from).next()
      return cursor.done ? undefined : cursor.value
    }
    function previous(from: number, to: number) {
      for (let size = 1; ; size++) {
        const start = Math.max(from, to - size * 10000)
        const cursor = query.getCursor(state, start, to)
        let last: typeof cursor.value | undefined
        while (!cursor.next().done) last = cursor.value
        if (last && (start === from || last.from > start + 10)) return last
        if (start === from) return undefined
      }
    }
    function replacement(match: ReturnType<typeof next>): string {
      if (!match || !('match' in match)) return request.replacement
      const captures = match.match
      const text = request.replacement.replace(
        /\\([nrt\\])/g,
        (_, char: string) => ({ n: '\n', r: '\r', t: '\t', '\\': '\\' })[char]!
      )
      return text.replace(/\$([$&]|\d+)/g, (token, group: string) => {
        if (group === '$') return '$'
        if (group === '&') return captures[0]
        for (let length = group.length; length > 0; length--) {
          const index = Number(group.slice(0, length))
          if (index > 0 && index < captures.length) return captures[index] + group.slice(length)
        }
        return token
      })
    }
    const { from, to } = request.selection
    let selected: ReturnType<typeof next>
    if (request.action === 'next') selected = next(to, to)
    if (request.action === 'previous')
      selected = previous(0, from) ?? previous(from, state.doc.length)
    if (request.action === 'replace') {
      selected = next(from, from)
      if (selected?.from === from && selected.to === to) {
        response.changes = [{ from, to, insert: replacement(selected) }]
        selected = next(from, to)
      }
    }
    if (request.action === 'replaceAll') {
      response.changes = []
      const cursor = query.getCursor(state)
      while (!cursor.next().done)
        response.changes.push({
          from: cursor.value.from,
          to: cursor.value.to,
          insert: replacement(cursor.value),
        })
    }
    if (selected) {
      const selection = EditorSelection.range(selected.from, selected.to).map(
        state.changes(response.changes ?? [])
      )
      response.selection = { from: selection.from, to: selection.to }
    }
    return response
  }
}
