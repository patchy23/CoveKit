import type { DbType, DbValue, TableTarget } from './contracts'

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

/** 复制单行 INSERT；未知来源使用 result 占位表，不推断 JOIN 或别名的写入目标。 */
export function rowToInsertSql(
  kind: DbType,
  columns: string[],
  row: DbValue[],
  target?: TableTarget | null
): string {
  if (kind === 'redis') throw new Error('Redis 结果不支持复制为 SQL')
  if (!columns.length || row.length !== columns.length || new Set(columns).size !== columns.length)
    throw new Error('结果列不完整或存在重名列，无法生成 INSERT')
  const mysql = kind === 'mysql' || kind === 'polardb'
  const postgres = ['postgresql', 'vastbase', 'kingbase'].includes(kind)
  const quote = (name: string) => {
    const delimiter = mysql ? '`' : '"'
    return delimiter + name.replaceAll(delimiter, delimiter + delimiter) + delimiter
  }
  const hex = (text: string) =>
    Array.from(new TextEncoder().encode(text), (byte) => byte.toString(16).padStart(2, '0')).join(
      ''
    )
  const literal = (cell: DbValue): string => {
    if (cell.kind === 'null') return 'NULL'
    const value = cell.value ?? ''
    if (cell.kind === 'binary') {
      if (!/^(?:[\da-f]{2})*$/i.test(value)) throw new Error('二进制值不是有效的十六进制数据')
      if (postgres) return `decode('${value}', 'hex')`
      if (mysql || kind === 'sqlite') return `X'${value}'`
      return `HEXTORAW('${value}')`
    }
    if (
      ['integer', 'decimal', 'float'].includes(cell.kind) &&
      /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(value)
    )
      return value
    if (cell.kind === 'boolean' && /^(true|false|0|1)$/i.test(value)) {
      const truth = /^(true|1)$/i.test(value)
      return postgres ? (truth ? 'TRUE' : 'FALSE') : truth ? '1' : '0'
    }
    if (mysql && (value.includes('\\') || value.includes('\u0000')))
      return `CONVERT(X'${hex(value)}' USING utf8mb4)`
    if (kind === 'sqlite' && value.includes('\u0000')) return `CAST(X'${hex(value)}' AS TEXT)`
    if (value.includes('\u0000')) throw new Error('该数据库的 SQL 文本值不支持零字节')
    const text = value.replaceAll("'", "''")
    return postgres ? `E'${text.replaceAll('\\', '\\\\')}'` : `'${text}'`
  }
  const scope = target && (mysql ? target.database || target.schema : target.schema)
  const table = target
    ? [kind !== 'sqlite' ? scope : undefined, target.table]
        .filter(Boolean)
        .map((name) => quote(name!))
        .join('.')
    : quote('result')
  return `INSERT INTO ${table} (${columns.map(quote).join(', ')}) VALUES (${row.map(literal).join(', ')});`
}
