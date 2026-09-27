/**
 * 差异统计（纯函数）
 *
 * 按行做多重集比对：`added` = 新文本中多出来的行数，`removed` = 原文本中消失的行数，
 * 保持既有多重集口径，行移动不计作增删；视图的变更块仍由 `@codemirror/merge` 计算。
 */
import type { DiffStats } from './types'

/** 按行切分：CRLF 归一为 LF，空文本 0 行，末尾换行不额外产生一行 */
function* lines(text: string, start: number, limit: number): Generator<string> {
  while (start < limit) {
    const newline = text.indexOf('\n', start)
    if (newline < 0 || newline >= limit) {
      yield text.slice(start, limit)
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

  // 相同的前后行可直接配对，不必把大文件未变化部分放入计数表。
  // 范围保留原始换行边界：只在 LF 前忽略 CR，末尾独立 CR 仍是正文。
  let fromA = 0
  let fromB = 0
  let toA = original.length
  let toB = modified.length
  while (fromA < toA && fromB < toB) {
    const newlineA = original.indexOf('\n', fromA)
    const newlineB = modified.indexOf('\n', fromB)
    const endA = newlineA < 0 ? toA : newlineA
    const endB = newlineB < 0 ? toB : newlineB
    const contentA = newlineA >= 0 && original.charCodeAt(endA - 1) === 13 ? endA - 1 : endA
    const contentB = newlineB >= 0 && modified.charCodeAt(endB - 1) === 13 ? endB - 1 : endB
    if (original.slice(fromA, contentA) !== modified.slice(fromB, contentB)) break
    fromA = newlineA < 0 ? toA : newlineA + 1
    fromB = newlineB < 0 ? toB : newlineB + 1
  }
  while (fromA < toA && fromB < toB) {
    const newlineA = original.charCodeAt(toA - 1) === 10
    const newlineB = modified.charCodeAt(toB - 1) === 10
    const endA = newlineA ? toA - 1 : toA
    const endB = newlineB ? toB - 1 : toB
    const startA = endA > fromA ? original.lastIndexOf('\n', endA - 1) + 1 : fromA
    const startB = endB > fromB ? modified.lastIndexOf('\n', endB - 1) + 1 : fromB
    const contentA = newlineA && original.charCodeAt(endA - 1) === 13 ? endA - 1 : endA
    const contentB = newlineB && modified.charCodeAt(endB - 1) === 13 ? endB - 1 : endB
    if (original.slice(startA, contentA) !== modified.slice(startB, contentB)) break
    toA = startA
    toB = startB
  }

  // 原文本各行剩余可配对次数
  const remaining = new Map<string, number>()
  for (const line of lines(original, fromA, toA)) {
    remaining.set(line, (remaining.get(line) ?? 0) + 1)
  }

  let added = 0
  for (const line of lines(modified, fromB, toB)) {
    const count = remaining.get(line) ?? 0
    if (count === 1) remaining.delete(line)
    else if (count > 1) remaining.set(line, count - 1)
    else added += 1
  }

  let removed = 0
  for (const count of remaining.values()) removed += count

  return { added, removed, same: added === 0 && removed === 0 }
}
