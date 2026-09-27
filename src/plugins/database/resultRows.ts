import { toRaw } from 'vue'
import type { DbValue } from './contracts'

/** 一个结果快照共用行身份索引；弱键不额外持有正文，重复引用沿用 indexOf 的首项语义。 */
export function indexResultRows(rows: string[][]): (row: string[]) => number {
  // 小结果直接查找更便宜；只选择算法，不限制可筛选、复制或导出的行数。
  if (rows.length <= 128) return (row) => rows.indexOf(row)
  const positions = new WeakMap<string[], number>()
  rows.forEach((row, index) => {
    const key = toRaw(row)
    if (!positions.has(key)) positions.set(key, index)
  })
  return (row) => positions.get(toRaw(row)) ?? -1
}

/** 筛选只改变顺序与可见范围，复制/导出仍使用原始类型值。 */
export function selectResultValues(
  rows: string[][],
  values: DbValue[][],
  indexOf: (row: string[]) => number
): DbValue[][] {
  return rows.map(
    (row) => values[indexOf(row)] ?? row.map((value): DbValue => ({ kind: 'text', value }))
  )
}

/** 仅为当前页构建网格对象，行号查找不随列数重复执行。 */
export function resultGridRows(
  columns: string[],
  rows: string[][],
  values: DbValue[][],
  indexOf: (row: string[]) => number
): Array<{ __row: string } & Record<string, string | null>> {
  return rows.map((row) => {
    const index = indexOf(row)
    const typed = values[index]
    return {
      __row: String(index),
      ...Object.fromEntries(
        columns.map((_, col) => [
          `c${col}`,
          typed?.[col]?.kind === 'null' ? null : (row[col] ?? ''),
        ])
      ),
    }
  })
}
