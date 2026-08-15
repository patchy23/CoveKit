/**
 * 可视化建表 · CREATE TABLE SQL 生成（纯函数，MySQL 系）
 * 由 CreateTableTab 的列编辑状态生成 DDL 预览与执行语句；
 * 标识符一律反引号引用，注释单引号转义，防注入。
 */
import { MYSQL_TYPES_WITH_LENGTH } from './useDatabaseMeta'

/** 可视化建表的列编辑行 */
export interface CreateTableColumn {
  /** 列名 */
  name: string
  /** 列类型（大写，如 VARCHAR） */
  type: string
  /** 长度/精度（VARCHAR(64)、DECIMAL(10,2)；空 = 不带） */
  length: string
  /** 可空 */
  nullable: boolean
  /** 默认值（空串 = 不设；数字与 CURRENT_TIMESTAMP 等关键字原样写入，其余按字符串字面量加引号） */
  defaultValue: string
  /** 自增 */
  autoIncrement: boolean
  /** 主键 */
  primary: boolean
  /** 注释 */
  comment: string
}

/** 可视化建表的表级选项 */
export interface CreateTableOptions {
  /** 库名（空 = 当前会话默认库） */
  database: string
  /** 表名 */
  table: string
  /** 存储引擎（空 = 不指定） */
  engine: string
  /** 字符集（空 = 不指定） */
  charset: string
  /** 表注释 */
  comment: string
  /** 列清单 */
  columns: CreateTableColumn[]
}

/** 标识符引用（反引号转义） */
function quoteIdent(name: string): string {
  return `\`${name.replace(/`/g, '``')}\``
}

/** 字符串字面量引用（单引号转义） */
function quoteLiteral(value: string): string {
  return `'${value.replace(/'/g, "''")}'`
}

/** 空列行（新建/追加用默认值） */
export function emptyColumn(): CreateTableColumn {
  return {
    name: '',
    type: 'VARCHAR',
    length: '64',
    nullable: true,
    defaultValue: '',
    autoIncrement: false,
    primary: false,
    comment: '',
  }
}

/** 默认值是否为「原样写入」的非字面量（数字 / NULL / TRUE/FALSE / CURRENT_* 关键字） */
function isRawDefault(value: string): boolean {
  const v = value.trim()
  return /^-?\d+(\.\d+)?$/.test(v) || /^(NULL|TRUE|FALSE|CURRENT_\w+)$/i.test(v)
}

/** 生成 CREATE TABLE DDL；列为空/表名为空时返回空串（预览区据此提示） */
export function buildCreateTableSql(options: CreateTableOptions): string {
  const table = options.table.trim()
  const validColumns = options.columns.filter((c) => c.name.trim())
  if (!table || !validColumns.length) return ''

  const lines = validColumns.map((c) => {
    const name = c.name.trim()
    const type = c.type.toUpperCase()
    // 长度仅对支持的类型拼接；DECIMAL 允许 "10,2"
    const length = c.length.trim()
    const typeSql =
      length && MYSQL_TYPES_WITH_LENGTH.has(type) && /^[\d,]+$/.test(length)
        ? `${type}(${length})`
        : type
    let line = `  ${quoteIdent(name)} ${typeSql}`
    if (!c.nullable) line += ' NOT NULL'
    if (c.autoIncrement) line += ' AUTO_INCREMENT'
    if (c.defaultValue !== '') {
      line += ` DEFAULT ${isRawDefault(c.defaultValue) ? c.defaultValue.trim() : quoteLiteral(c.defaultValue)}`
    }
    if (c.comment.trim()) line += ` COMMENT ${quoteLiteral(c.comment.trim())}`
    return line
  })

  const primaryKeys = validColumns.filter((c) => c.primary).map((c) => quoteIdent(c.name.trim()))
  if (primaryKeys.length) lines.push(`  PRIMARY KEY (${primaryKeys.join(', ')})`)

  const qualified = options.database.trim()
    ? `${quoteIdent(options.database.trim())}.${quoteIdent(table)}`
    : quoteIdent(table)
  let sql = `CREATE TABLE ${qualified} (\n${lines.join(',\n')}\n)`
  if (options.engine) sql += ` ENGINE=${options.engine}`
  if (options.charset) sql += ` DEFAULT CHARACTER SET ${options.charset}`
  if (options.comment.trim()) sql += ` COMMENT=${quoteLiteral(options.comment.trim())}`
  return sql
}
