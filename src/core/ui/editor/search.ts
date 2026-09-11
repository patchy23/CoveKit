/**
 * 查找替换：匹配扫描（纯函数）+ CM6 查询构建
 *
 * 面板 UI（`EditorSearchBar.vue`）用 `scanMatches` 得到匹配总数与当前位置序号，
 * 用 `toSearchQuery` 把同一份条件交给 `@codemirror/search` 执行跳转与高亮，
 * 保证「面板显示的 n/total」与实际跳转行为来自同一套语义。
 */
import { SearchQuery } from '@codemirror/search'

/** 匹配数量上限（防止超大文档 + 极短查询拖垮扫描） */
export const MAX_MATCHES = 5000

/** 查找条件 */
export interface SearchOptions {
  /** 区分大小写 */
  caseSensitive: boolean
  /** 正则模式 */
  regexp: boolean
  /** 全词匹配 */
  wholeWord: boolean
}

/** 匹配扫描结果 */
export interface MatchScan {
  /** 匹配起点偏移（升序） */
  positions: number[]
  /** 每个匹配的长度（与 positions 一一对应） */
  lengths: number[]
  /** 是否达到扫描上限（面板提示"仅统计前 N 处"） */
  truncated: boolean
  /** 正则非法的中文错误（面板内提示，不弹 toast） */
  error?: string
}

/** 转义正则元字符（字面量模式使用） */
export function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/** 扫描文档中的匹配位置（纯函数）。空查询返回空结果，非法正则返回 error */
export function scanMatches(text: string, query: string, options: SearchOptions): MatchScan {
  if (!query) return { positions: [], lengths: [], truncated: false }

  let source = options.regexp ? query : escapeRegExp(query)
  if (options.wholeWord) source = `\\b(?:${source})\\b`

  let pattern: RegExp
  try {
    pattern = new RegExp(source, options.caseSensitive ? 'g' : 'gi')
  } catch (error) {
    return {
      positions: [],
      lengths: [],
      truncated: false,
      error: `正则表达式无效：${error instanceof Error ? error.message : String(error)}`,
    }
  }

  const positions: number[] = []
  const lengths: number[] = []
  let match = pattern.exec(text)
  while (match) {
    positions.push(match.index)
    lengths.push(match[0].length)
    // 零长匹配（如 `a*`）必须手动推进，否则正则引擎原地打转
    if (match[0].length === 0) pattern.lastIndex += 1
    if (positions.length >= MAX_MATCHES) {
      return { positions, lengths, truncated: true }
    }
    match = pattern.exec(text)
  }
  return { positions, lengths, truncated: false }
}

/** 当前匹配序号（1 起始）：光标所在位置命中某个匹配起点时返回其序号，否则 0 */
export function matchIndexOf(scan: MatchScan, position: number): number {
  const index = scan.positions.indexOf(position)
  return index < 0 ? 0 : index + 1
}

/** 构建 CM6 查询对象（与 scanMatches 同语义） */
export function toSearchQuery(
  query: string,
  replacement: string,
  options: SearchOptions
): SearchQuery {
  return new SearchQuery({
    search: query,
    replace: replacement,
    literal: !options.regexp,
    regexp: options.regexp,
    caseSensitive: options.caseSensitive,
    wholeWord: options.wholeWord,
  })
}
