import { toRaw } from 'vue'
import type { DbValue } from './contracts'

export type ResultPageAction = 'advance' | 'load-next' | 'end'

/** 在构造网格投影前截取当前显示页，保留原行对象身份。 */
export function sliceResultPage<T>(rows: T[], page: number, pageSize: number): T[] {
  const size = Math.max(1, Math.floor(pageSize))
  const start = (Math.max(1, Math.floor(page)) - 1) * size
  return rows.slice(start, start + size)
}

/** 判断显式下一步是切页还是继续读取下一页所需的数据。 */
export function resultPageAction(
  page: number,
  pageSize: number,
  loadedCount: number,
  hasMore: boolean
): ResultPageAction {
  const size = Math.max(1, Math.floor(pageSize))
  const end = Math.max(1, Math.floor(page)) * size
  if (loadedCount > end) return 'advance'
  return hasMore ? 'load-next' : 'end'
}

/** 一个结果快照共用行身份索引；弱键不额外持有正文，重复引用沿用 indexOf 的首项语义。 */
export function indexResultRows(rows: string[][]): (row: string[]) => number {
  // 小结果直接查找更便宜；只选择算法，不限制可筛选、复制或导出的行数。
  if (rows.length <= 128)
    return (row) => {
      const key = toRaw(row)
      return rows.findIndex((candidate) => toRaw(candidate) === key)
    }
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

/** 为已加载结果构建网格投影，复用单元格字符串；行号查找不随列数重复执行。 */
export function resultGridRows(
  columns: string[],
  rows: string[][],
  values: DbValue[][],
  indexOf: (row: string[]) => number,
  rowLabel?: (row: string[], index: number) => number
): Array<{ __row: string; __label?: string } & Record<string, string | null>> {
  return rows.map((row) => {
    const index = indexOf(row)
    const typed = values[index]
    return {
      __row: String(index),
      ...(rowLabel ? { __label: String(rowLabel(row, index)) } : {}),
      ...Object.fromEntries(
        columns.map((_, col) => [
          `c${col}`,
          typed?.[col]?.kind === 'null' ? null : (row[col] ?? ''),
        ])
      ),
    }
  })
}
