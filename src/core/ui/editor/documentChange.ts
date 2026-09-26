/** 对比不可变文档的公共前后缀，仅替换变化区间，不将旧文档序列化为全文字符串。 */
import type { Text } from '@codemirror/state'

function matchingLength(left: Text, right: Text, direction: 1 | -1, limit: number): number {
  const a = left.iter(direction)
  const b = right.iter(direction)
  a.next()
  b.next()
  let aOffset = 0
  let bOffset = 0
  let matched = 0
  while (matched < limit && !a.done && !b.done) {
    const size = Math.min(a.value.length - aOffset, b.value.length - bOffset, limit - matched)
    // 相同行走原生字符串比较；仅边界处的不同片段逐 UTF-16 码元检查。
    if (!(aOffset === 0 && bOffset === 0 && a.value === b.value)) {
      for (let i = 0; i < size; i++) {
        const ai = direction === 1 ? aOffset + i : a.value.length - aOffset - i - 1
        const bi = direction === 1 ? bOffset + i : b.value.length - bOffset - i - 1
        if (a.value.charCodeAt(ai) !== b.value.charCodeAt(bi)) return matched + i
      }
    }
    matched += size
    aOffset += size
    bOffset += size
    if (aOffset === a.value.length) {
      a.next()
      aOffset = 0
    }
    if (bOffset === b.value.length) {
      b.next()
      bOffset = 0
    }
  }
  return matched
}

/** 返回一个连续替换区间；相同内容返回 null，保留原文档及其选区、折叠状态。 */
export function documentChange(
  current: Text,
  next: Text
): { from: number; to: number; insert: Text } | null {
  if (current === next) return null
  const shortest = Math.min(current.length, next.length)
  const from = matchingLength(current, next, 1, shortest)
  if (from === current.length && from === next.length) return null
  const suffix = matchingLength(current, next, -1, shortest - from)
  return { from, to: current.length - suffix, insert: next.slice(from, next.length - suffix) }
}
