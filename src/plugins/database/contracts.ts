/**
 * 数据库工作台插件 · IPC 契约（本插件私有）
 * 与 src-tauri/src/plugins/database/models.rs 的 serde 结构逐字段同步（camelCase）。
 * 命令前缀 dbc_；错误统一抛 IpcError（命令内不返回 ok:false 结构体）。
 */

/** 数据库类型（与 Rust DbType 一致；dameng 后端暂未实现） */
export type DbType =
  | 'mysql'
  | 'postgresql'
  | 'oracle'
  | 'dameng'
  | 'vastbase'
  | 'kingbase'
  | 'polardb'
  | 'redis'
  | 'sqlite'

/** 连接配置（密码不入本结构，独立走 stronghold 保存/读取） */
export interface ConnConfig {
  /** 连接唯一 id */
  id: string
  /** 展示名称 */
  label: string
  /** 数据库类型 */
  dbType: DbType
  /** 主机地址（sqlite 为文件路径） */
  host: string
  /** 端口（sqlite 为 0） */
  port: number
  /** 用户名 */
  username: string
  /** 默认数据库（redis 为 db0 等） */
  database: string
  /** 环境标记：生产/测试/开发 */
  env: string
  /** 只读连接 */
  readonly: boolean
  /** 是否启用 TLS */
  ssl: boolean
  /** 连接超时（毫秒） */
  connectTimeoutMs: number
}

/** 连接状态 */
export type ConnStatus = 'online' | 'offline' | 'connecting'

/** 连接快照（列表/树节点展示） */
export interface DbConnectionInfo {
  id: string
  label: string
  dbType: DbType
  env: string
  status: ConnStatus
  version: string
  latencyMs: number
  readonly: boolean
  host: string
  database: string
  error?: string | null
  /** 建立连接时间（epoch 秒；未连接为 0） */
  connectedAt: number
  /** 端口（sqlite 为 0；编辑对话框回填用） */
  port: number
  /** 用户名（编辑对话框回填用） */
  username: string
  /** 是否启用 TLS（编辑对话框回填用） */
  ssl: boolean
  /** 连接超时（毫秒；编辑对话框回填用） */
  connectTimeoutMs: number
}

/** 对象树叶子信息 */
export interface DbObjectInfo {
  /** 对象类型：table/view/function/sequence/procedure/package/synonym/index/trigger/event */
  kind: string
  /** 对象名 */
  name: string
}

/** 表结构列信息 */
export interface DbColumnInfo {
  name: string
  dataType: string
  nullable: string
  defaultValue: string
  key: string
  comment: string
}

/** SQL 执行结果 */
export interface QueryResult {
  ok: boolean
  columns: string[]
  rows: string[][]
  rowsAffected: number
  isQuery: boolean
  durationMs: number
  truncated: boolean
  error?: string | null
}

/** 表数据分页 */
export interface DbTablePage {
  columns: string[]
  rows: string[][]
  total: number
  page: number
  pageSize: number
  durationMs: number
  error?: string | null
}

/** Redis 键信息 */
export interface RedisKeyInfo {
  key: string
  kind: string
  ttl: number
  value: string
}

/** 查询历史条目 */
export interface HistoryEntry {
  id: number
  connId: string
  sql: string
  status: 'success' | 'error'
  durationMs: number
  at: string
}

/** 收藏 SQL 条目 */
export interface SavedEntry {
  id: number
  title: string
  sql: string
  at: string
}

/** agent 驱动状态（诊断） */
export interface DriverStatus {
  ready: boolean
  kind: 'native' | 'agent'
  dir?: string
  version?: string | null
  note?: string
}

/** 命令清单 */
export const commands = {
  dbcConnectionSave: 'dbc_connection_save',
  dbcConnectionDelete: 'dbc_connection_delete',
  dbcConnections: 'dbc_connections',
  dbcConnect: 'dbc_connect',
  dbcDisconnect: 'dbc_disconnect',
  dbcTest: 'dbc_test',
  dbcHistory: 'dbc_history',
  dbcHistoryAdd: 'dbc_history_add',
  dbcHistoryClear: 'dbc_history_clear',
  dbcSaved: 'dbc_saved',
  dbcSavedAdd: 'dbc_saved_add',
  dbcSavedUpdate: 'dbc_saved_update',
  dbcSavedDelete: 'dbc_saved_delete',
  dbcDriverStatus: 'dbc_driver_status',
  dbcExecute: 'dbc_execute',
  dbcCancel: 'dbc_cancel',
  dbcDatabases: 'dbc_databases',
  dbcSchemas: 'dbc_schemas',
  dbcObjects: 'dbc_objects',
  dbcColumns: 'dbc_columns',
  dbcTableData: 'dbc_table_data',
  dbcExportCsv: 'dbc_export_csv',
  dbcRedisKeys: 'dbc_redis_keys',
  dbcRedisKeyInfo: 'dbc_redis_key_info',
} as const

/** 命令入参 */
export type Payloads = {
  dbc_connection_save: { config: ConnConfig; password: string }
  dbc_connection_delete: { id: string }
  dbc_connections: Record<string, never>
  dbc_connect: { id: string }
  dbc_disconnect: { id: string }
  dbc_test: { config: ConnConfig; password: string }
  dbc_history: Record<string, never>
  dbc_history_add: { connId: string; sql: string; status: string; durationMs: number }
  dbc_history_clear: Record<string, never>
  dbc_saved: Record<string, never>
  dbc_saved_add: { title: string; sql: string }
  dbc_saved_update: { id: number; title: string; sql: string }
  dbc_saved_delete: { id: number }
  dbc_driver_status: { dbType: string }
  dbc_execute: { connId: string; sql: string; maxRows?: number }
  dbc_cancel: { connId: string }
  dbc_databases: { connId: string }
  dbc_schemas: { connId: string }
  dbc_objects: { connId: string; schema?: string }
  dbc_columns: { connId: string; schema?: string; table: string }
  dbc_table_data: { connId: string; schema?: string; table: string; page: number; pageSize: number }
  dbc_export_csv: { path: string; text: string }
  dbc_redis_keys: { connId: string; pattern: string; cursor: number }
  dbc_redis_key_info: { connId: string; key: string }
}

/** 命令返回 */
export type Results = {
  dbc_connection_save: void
  dbc_connection_delete: void
  dbc_connections: DbConnectionInfo[]
  dbc_connect: DbConnectionInfo
  dbc_disconnect: void
  dbc_test: string
  dbc_history: HistoryEntry[]
  dbc_history_add: void
  dbc_history_clear: void
  dbc_saved: SavedEntry[]
  dbc_saved_add: number
  dbc_saved_update: void
  dbc_saved_delete: void
  dbc_driver_status: DriverStatus
  dbc_execute: QueryResult
  dbc_cancel: void
  dbc_databases: string[]
  dbc_schemas: string[]
  dbc_objects: DbObjectInfo[]
  dbc_columns: DbColumnInfo[]
  dbc_table_data: DbTablePage
  dbc_export_csv: void
  dbc_redis_keys: [number, string[]]
  dbc_redis_key_info: RedisKeyInfo
}
