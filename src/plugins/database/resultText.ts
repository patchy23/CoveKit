import type { DbValue } from './contracts'

/** 与 CSV 文件约定一致：NULL 用 \\N，文本反斜线前缀转义，二进制为 hex。 */
export function cellTransferText(cell: DbValue): string {
  if (cell.kind === 'null') return '\\N'
  const value = cell.value ?? ''
  return value.startsWith('\\') ? '\\' + value : value
}

/** TSV 按字段引用制表符、换行和引号，避免一格内容变成多行多列。 */
export function rowsToTsv(rows: DbValue[][]): string {
  return rows
    .map((row) =>
      row
        .map((cell) => {
          const text = cellTransferText(cell)
          return /[\t\r\n"]/.test(text) ? '"' + text.replaceAll('"', '""') + '"' : text
        })
        .join('\t')
    )
    .join('\r\n')
}
