/**
 * 差异统计（纯函数）
 *
 * 按行做多重集比对：`added` = 新文本中多出来的行数，`removed` = 原文本中消失的行数，
 * 与 GitHub 的 `+N/-M` 口径一致。刻意不引 diff 算法：行级差异在文本很大时会退化为
 * 粗粒度结果，而统计口径一旦与视图不一致反而误导用户（视图自身由 `@codemirror/merge` 精确呈现）。
 */
import type { DiffStats } from './types'

/** 按行切分：CRLF 归一为 LF，空文本 0 行，末尾换行不额外产生一行 */
function toLines(text: string): string[] {
  const normalized = text.replace(/\r\n/g, '\n')
  if (normalized === '') return []
  const lines = normalized.split('\n')
  if (lines[lines.length - 1] === '') lines.pop()
  return lines
}

/**
 * 统计两份文本的差异行数
 *
 * @param original 原文本
 * @param modified 新文本
 */
export function diffStats(original: string, modified: string): DiffStats {
  const originalLines = toLines(original)
  const modifiedLines = toLines(modified)

  // 原文本各行剩余可配对次数
  const remaining = new Map<string, number>()
  for (const line of originalLines) {
    remaining.set(line, (remaining.get(line) ?? 0) + 1)
  }

  let added = 0
  for (const line of modifiedLines) {
    const count = remaining.get(line) ?? 0
    if (count > 0) remaining.set(line, count - 1)
    else added += 1
  }

  let removed = 0
  for (const count of remaining.values()) removed += count

  return { added, removed, same: added === 0 && removed === 0 }
}
