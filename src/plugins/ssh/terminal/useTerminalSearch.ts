/**
 * SSH 终端搜索（Ctrl+F）· SearchAddon 生命周期 + 防抖 + 结果计数
 *
 * 职责边界：本文件只管「搜什么、怎么搜、命中几条」，不碰面板 DOM；
 * 面板 `TerminalSearchBar.vue` 只收集条件（输入框 / Aa / .* / 上下一个），
 * 两侧共用同一套计数语义（formatResultCount），保证显示与实际跳转一致。
 *
 * 依赖实测结论（2026-09-12，vitest 起真 xterm 实例验证）：
 * - `@xterm/addon-search@0.15.0` 与项目在用的旧包名 `xterm@5.3.0` 运行期兼容（addon 不
 *   import 包体，只操作传入的 Terminal 实例，与现有 addon-fit@0.11.0 同模式）：findNext /
 *   findPrevious / onDidChangeResults / clearDecorations 全部走公开 API；实测 0.14.0 行为
 *   完全一致，故不按下限回退。
 * - 传 `decorations` 时 addon 会调 xterm 的 `registerDecoration`（5.3.0 里属 proposed API）：
 *   Terminal 未开 `allowProposedApi` 会直接抛 "You must set the allowProposedApi option to
 *   true to use proposed API"（实测）。不开就只剩「命中/未命中」、拿不到 n-of-total，
 *   因此 TerminalTab 已显式开启该选项。
 * - 正则非法时 addon 抛 SyntaxError（实测），这里捕获成面板红字，不抛到 console。
 */
import { ref, watch } from 'vue'
import type { Terminal } from 'xterm'
import { SearchAddon } from '@xterm/addon-search'
import type { ISearchOptions } from '@xterm/addon-search'

/** 输入防抖（任务书 §1.4 : 150ms）：停顿后再搜，避免逐字符扫整个缓冲 */
export const SEARCH_DEBOUNCE_MS = 150

/**
 * 匹配高亮配色（任务书 §1.4：取 DESIGN tokens，禁止新造色值）。
 * xterm 只接受 `#RRGGBB` 字面量，故这里写 token 的字面值并注明来源（终端底色 #0d1117，
 * 取深色档 token）：其余匹配淡高亮 = `--color-tertiary-soft-dark`；当前匹配强调底 =
 * `--color-tertiary-strong`、描边 = `--color-tertiary-dark`；概览标尺 = `--color-tertiary`
 * （终端未开 overviewRulerWidth，预置为以后开启即可用）。
 */
export const SEARCH_DECORATIONS: NonNullable<ISearchOptions['decorations']> = {
  matchBackground: '#3a2116',
  matchBorder: '#f0562c',
  matchOverviewRuler: '#f0562c',
  activeMatchBackground: '#c2410c',
  activeMatchBorder: '#ff7a4d',
  activeMatchColorOverviewRuler: '#ff7a4d',
}

/** 搜索条件（面板 → 本 composable） */
export interface TerminalSearchFlags {
  /** 区分大小写（面板 Aa） */
  caseSensitive: boolean
  /** 正则模式（面板 .*） */
  regex: boolean
}

/** addon 回调里的结果计数 */
export interface TerminalSearchResult {
  /** 当前匹配序号（0 起始；-1 = 未定位 / 超出高亮上限） */
  resultIndex: number
  /** 命中总数 */
  resultCount: number
}

/** addon 的最小使用面：单测可注入桩，不必起真终端 */
export interface TerminalSearchAddon {
  findNext(term: string, options?: ISearchOptions): boolean
  findPrevious(term: string, options?: ISearchOptions): boolean
  clearDecorations(): void
  onDidChangeResults(listener: (event: TerminalSearchResult) => void): { dispose: () => void }
}

/** 组装 addon 选项：incremental 常开；decorations 常开，否则拿不到 n-of-total */
export function searchOptions(flags: TerminalSearchFlags): ISearchOptions {
  return {
    caseSensitive: flags.caseSensitive,
    regex: flags.regex,
    incremental: true,
    decorations: SEARCH_DECORATIONS,
  }
}

/**
 * 计数文案：`3/17`；无结果 `0/0`；超出高亮上限（总数有、当前未定位）`-/总数`。
 * 传 `query` 时再做一层空查询门禁 → `''`（任务书 §1.3 的三种输出）；
 * 不传则纯按 index/count 计数（宿主自己判断是否已有关键字）。
 */
export function formatResultCount(index: number, count: number, query?: string): string {
  if (query !== undefined && !shouldSearch(query)) return ''
  if (count <= 0) return '0/0'
  if (index < 0) return `-/${count}`
  return `${index + 1}/${count}`
}

/** 是否值得触发搜索：空白/空串返回 false（否则会全量高亮整个缓冲），面板据此留空计数 */
export function shouldSearch(query: string): boolean {
  return query.trim().length > 0
}

/** Ctrl+F 判定：xterm 的 attachCustomKeyEventHandler 用，纯函数便于单测 */
export function isSearchShortcut(
  event: Pick<KeyboardEvent, 'type' | 'ctrlKey' | 'metaKey' | 'key'>
): boolean {
  return (
    event.type === 'keydown' && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f'
  )
}

/**
 * 终端搜索控制器。
 * `getTerm` 用取值函数（终端在 onMounted 里才创建），与 useTerminalContextMenu 同形；
 * `addon` 由调用方创建并 `term.loadAddon()`（任务书 §1.5），本函数只管调度与计数。
 * 结果订阅在创建时挂上，卸载时调 `dispose()`。
 */
export function createTerminalSearch(
  getTerm: () => Terminal | null,
  addon: TerminalSearchAddon = new SearchAddon()
) {
  const visible = ref(false)
  const query = ref('')
  const caseSensitive = ref(false)
  const regex = ref(false)
  /** 当前匹配序号（0 起始）/ 命中总数；0/0 = 无匹配 */
  const resultIndex = ref(0)
  const resultCount = ref(0)
  /** 正则非法等异常的中文提示：面板内红字 + 输入框标红（不弹 toast、不进 console） */
  const regexError = ref('')

  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  const resultsSubscription = addon.onDidChangeResults((event) => {
    resultIndex.value = event.resultIndex
    resultCount.value = event.resultCount
  })

  function cancelDebounce() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
  }

  function resetResult() {
    resultIndex.value = 0
    resultCount.value = 0
  }

  /** 清高亮并归零计数（空查询、关闭面板、换连接/终端 reset 时用） */
  function clearDecorations() {
    resetResult()
    addon.clearDecorations()
  }

  function runSearch(direction: 'next' | 'previous') {
    const term = getTerm()
    if (!term || !shouldSearch(query.value)) return
    const flags: TerminalSearchFlags = { caseSensitive: caseSensitive.value, regex: regex.value }
    try {
      const hit =
        direction === 'next'
          ? addon.findNext(query.value, searchOptions(flags))
          : addon.findPrevious(query.value, searchOptions(flags))
      regexError.value = ''
      if (!hit) resetResult()
    } catch (error) {
      // 正则非法（SyntaxError）等：清高亮 + 计数归零，把原因留在面板红字里
      regexError.value = error instanceof SyntaxError ? '正则表达式无效' : `搜索失败：${error}`
      resetResult()
      addon.clearDecorations()
    }
  }

  /** 下一个匹配（Enter / 面板按钮；立即执行，不等防抖） */
  function findNext() {
    cancelDebounce()
    runSearch('next')
  }

  /** 上一个匹配（Shift+Enter / 面板按钮） */
  function findPrevious() {
    cancelDebounce()
    runSearch('previous')
  }

  /** 打开面板；关键字还在时重跑一次（关闭时已清掉高亮） */
  function open() {
    visible.value = true
    if (shouldSearch(query.value)) runSearch('next')
  }

  /** 关闭面板：撤高亮、清计数，并归还终端焦点（否则光标消失、键盘输入无响应） */
  function close() {
    visible.value = false
    cancelDebounce()
    regexError.value = ''
    clearDecorations()
    getTerm()?.focus()
  }

  /** 卸载时调用：清防抖与结果订阅（addon 本身随 term.dispose() 释放） */
  function dispose() {
    cancelDebounce()
    resultsSubscription.dispose()
  }

  // 输入即搜（150ms 防抖）；条件一变先归零，避免短暂显示上一条查询的计数
  watch([query, caseSensitive, regex], () => {
    cancelDebounce()
    regexError.value = ''
    if (!shouldSearch(query.value)) {
      clearDecorations()
      return
    }
    resetResult()
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      runSearch('next')
    }, SEARCH_DEBOUNCE_MS)
  })

  return {
    visible,
    query,
    caseSensitive,
    regex,
    resultIndex,
    resultCount,
    regexError,
    open,
    close,
    findNext,
    findPrevious,
    clearDecorations,
    dispose,
  }
}
