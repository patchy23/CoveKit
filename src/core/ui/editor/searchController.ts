/**
 * 编辑器查找替换控制器
 *
 * 把「面板条件」翻译成 CM6 的 `SearchQuery`（负责跳转与匹配高亮），同时用 `search.ts`
 * 的纯函数扫描文档得出匹配总数与当前序号——两者语义同源（literal / regexp、大小写、全词），
 * 所以面板显示的 `3/17` 与实际跳转到的位置始终一致。
 *
 * 文档变化后统计会过期，调用方需在 content 变化时调用 `refresh()`（宿主已接好）。
 */
import { ref, type Ref } from 'vue'
import type { EditorView } from '@codemirror/view'
import {
  findNext,
  findPrevious,
  replaceAll,
  replaceNext,
  SearchQuery,
  setSearchQuery,
} from '@codemirror/search'
import { matchIndexOf, scanMatches, toSearchQuery, type SearchOptions } from './search'

/** 面板可见的查找状态 */
export interface EditorSearchState {
  /** 匹配总数 */
  total: number
  /** 当前匹配序号（1 起始；0 表示选区不在任一匹配上） */
  current: number
  /** 查询条件是否有效（正则非法时为错误文案） */
  error?: string
}

/** 查找控制器对外能力 */
export interface EditorSearchController {
  /** 面板绑定的状态 */
  state: Ref<EditorSearchState>
  /** 当前条件（用于关闭时清空高亮） */
  apply: (query: string, replacement: string, options: SearchOptions) => void
  /** 跳到下一个匹配（自动回绕） */
  next: () => void
  /** 跳到上一个匹配 */
  previous: () => void
  /** 替换当前匹配 */
  replaceCurrent: () => void
  /** 替换全部匹配 */
  replaceAllMatches: () => void
  /** 清空查询与高亮 */
  clear: () => void
  /** 文档变化后重算统计（不改查询条件） */
  refresh: () => void
  /** 当前查询条件（关闭面板后仍需用于「继续查找」） */
  current: Ref<{ query: string; replacement: string; options: SearchOptions }>
}

/** 默认条件：区分大小写关闭、正则关闭、全词关闭 */
const DEFAULT_OPTIONS: SearchOptions = { caseSensitive: false, regexp: false, wholeWord: false }

/**
 * 创建查找控制器
 *
 * @param getView 读取当前 EditorView（未挂载返回 null）
 * @param afterReplace 替换成功后的回调（宿主用于恢复焦点）
 */
export function createSearchController(
  getView: () => EditorView | null,
  afterReplace?: () => void
): EditorSearchController {
  const state = ref<EditorSearchState>({ total: 0, current: 0 })
  const current = ref<{ query: string; replacement: string; options: SearchOptions }>({
    query: '',
    replacement: '',
    options: { ...DEFAULT_OPTIONS },
  })

  /** 按当前条件重算 total / current（不改文档） */
  function recount(): void {
    const view = getView()
    const { query, options } = current.value
    if (!view || !query) {
      state.value = { total: 0, current: 0 }
      return
    }
    const scan = scanMatches(view.state.doc.toString(), query, options)
    if (scan.error) {
      state.value = { total: 0, current: 0, error: scan.error }
      return
    }
    const anchor = view.state.selection.main.from
    state.value = {
      total: scan.positions.length,
      current: matchIndexOf(scan, anchor),
    }
  }

  /** 把条件写入编辑器状态（高亮 + 供 下一个/替换 命令读取） */
  function pushQuery(): void {
    const view = getView()
    if (!view) return
    const { query, replacement, options } = current.value
    view.dispatch({
      effects: setSearchQuery.of(
        query ? toSearchQuery(query, replacement, options) : new SearchQuery({ search: '' })
      ),
    })
  }

  function apply(query: string, replacement: string, options: SearchOptions): void {
    const view = getView()
    if (!view) return
    const queryChanged = query !== current.value.query
    current.value = { query, replacement, options }
    pushQuery()
    if (!query) {
      state.value = { total: 0, current: 0 }
      return
    }
    // 条件变化时跳到第一个匹配，避免停在旧位置让用户以为没反应
    if (queryChanged) findNext(view)
    recount()
  }

  function next(): void {
    const view = getView()
    if (!view) return
    findNext(view)
    recount()
  }

  function previous(): void {
    const view = getView()
    if (!view) return
    findPrevious(view)
    recount()
  }

  function replaceCurrent(): void {
    const view = getView()
    if (!view) return
    replaceNext(view)
    afterReplace?.()
    recount()
  }

  function replaceAllMatches(): void {
    const view = getView()
    if (!view) return
    replaceAll(view)
    afterReplace?.()
    recount()
  }

  function clear(): void {
    current.value = { query: '', replacement: '', options: { ...DEFAULT_OPTIONS } }
    state.value = { total: 0, current: 0 }
    pushQuery()
  }

  /**
   * 文档或选区变化后重算统计
   *
   * 无查询条件时直接返回：否则每次输入都会扫一遍全文（大文档上是可观的浪费）。
   */
  function refresh(): void {
    if (!current.value.query) return
    recount()
  }

  return {
    state,
    current,
    apply,
    next,
    previous,
    replaceCurrent,
    replaceAllMatches,
    clear,
    refresh,
  }
}
