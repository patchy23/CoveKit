/**
 * SQLite 数据库插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/modules/db.rs（sqlx）的 serde 结构同步。
 */

/** 打开数据库结果 */
export interface DbOpenResult {
  ok: boolean
  tables: string[]
  error?: string
}

/** SQL 执行结果（查询返回表格，非查询返回影响行数） */
export interface DbQueryResult {
  ok: boolean
  columns: string[]
  rows: string[][]
  rowsAffected: number
  isQuery: boolean
  error?: string
}

/** 命令清单 */
export const commands = {
  dbOpen: 'db_open',
  dbClose: 'db_close',
  dbTables: 'db_tables',
  dbExecute: 'db_execute',
  dbQueryTable: 'db_query_table',
} as const

/** 命令入参 */
export type Payloads = {
  db_open: { path: string }
  db_close: Record<string, never>
  db_tables: Record<string, never>
  db_execute: { sql: string }
  db_query_table: { table: string; limit?: number }
}

/** 命令返回 */
export type Results = {
  db_open: DbOpenResult
  db_close: void
  db_tables: string[]
  db_execute: DbQueryResult
  db_query_table: DbQueryResult
}
