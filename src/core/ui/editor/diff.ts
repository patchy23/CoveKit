/**
 * 差异统计（纯函数）
 *
 * 按行做多重集比对：`added` = 新文本中多出来的行数，`removed` = 原文本中消失的行数，
 * 保持既有多重集口径，行移动不计作增删；视图的变更块仍由 `@codemirror/merge` 计算。
 */
import type { DiffStats } from './types'

/** 按行切分：CRLF 归一为 LF，空文本 0 行，末尾换行不额外产生一行 */
function* lines(text: string): Generator<string> {
  let start = 0
  while (start < text.length) {
    const newline = text.indexOf('\n', start)
    if (newline < 0) {
      yield text.slice(start)
      return
    }
    const end = newline > start && text.charCodeAt(newline - 1) === 13 ? newline - 1 : newline
    yield text.slice(start, end)
    start = newline + 1
  }
}

/**
 * 统计两份文本的差异行数
 *
 * @param original 原文本
 * @param modified 新文本
 */
export function diffStats(original: string, modified: string): DiffStats {
  if (original === modified) return { added: 0, removed: 0, same: true }

  // 原文本各行剩余可配对次数
  const remaining = new Map<string, number>()
  for (const line of lines(original)) {
    remaining.set(line, (remaining.get(line) ?? 0) + 1)
  }

  let added = 0
  for (const line of lines(modified)) {
    const count = remaining.get(line) ?? 0
    if (count > 0) remaining.set(line, count - 1)
    else added += 1
  }

  let removed = 0
  for (const count of remaining.values()) removed += count

  return { added, removed, same: added === 0 && removed === 0 }
}
