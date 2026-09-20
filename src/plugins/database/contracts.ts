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
  credentialId?: string | null
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
  credentialId?: string | null
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

/** 可逆原值；binary 使用十六进制，数值字符串保留精度。 */
export interface DbValue {
  kind: string
  value: string | null
}
export interface ExecutionScope {
  database: string
  schema: string
}
export interface ExecutionPreview {
  requiresConfirmation: boolean
  confirmationToken: string | null
  target: string
  summary: string
}

/** SQL 执行结果 */
export interface QueryResult {
  statementIndex?: number | null
  transactionActive?: boolean
  values?: DbValue[][]
  columnTypes?: string[]
  statements?: QueryResult[]
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
  querySql?: string
  queryParams?: DbValue[]
  values?: DbValue[][]
  hasMore?: boolean
  totalKind?: string
  stableOrder?: boolean
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
  database?: string
  schema?: string
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

/** 字符集选项（新建数据库对话框；collationsByCharset 用于字符集→排序规则联动） */
export interface DbCharsetOptions {
  charsets: string[]
  collationsByCharset: Record<string, string[]>
}

/** 数据库用户（授权选择） */
export interface DbUserInfo {
  user: string
  host: string
}

/** 授权目标（建库授权入参；privilege 白名单：all/readwrite/readonly） */
export interface DbGrantInput {
  user: string
  host: string
  privilege: 'all' | 'readwrite' | 'readonly'
}

/** 分步执行结果（建库+授权逐步反馈） */
export interface DbStepResult {
  label: string
  sql: string
  ok: boolean
  error?: string | null
}

/** 索引信息（结构页签 · 索引子页签） */
export interface DbIndexInfo {
  name: string
  columns: string[]
  nonUnique: boolean
  definition: string
}

/** 命令清单 */
export const commands = {
  dbcConnectionSave: 'dbc_connection_save',
  dbcConnectionDelete: 'dbc_connection_delete',
  dbcConnections: 'dbc_connections',
  dbcConnect: 'dbc_connect',
  dbcDisconnect: 'dbc_disconnect',
  dbcTest: 'dbc_test',
  dbcDrafts: 'dbc_drafts',
  dbcDraftsSave: 'dbc_drafts_save',
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
  dbcPrepareExecution: 'dbc_prepare_execution',
  dbcWorkspaceClose: 'dbc_workspace_close',
  dbcDatabases: 'dbc_databases',
  dbcSchemas: 'dbc_schemas',
  dbcObjects: 'dbc_objects',
  dbcColumns: 'dbc_columns',
  dbcTableData: 'dbc_table_data',
  dbcTableCount: 'dbc_table_count',
  dbcTableApply: 'dbc_table_apply',
  dbcCsvPreview: 'dbc_csv_preview',
  dbcCsvImport: 'dbc_csv_import',
  dbcExportCsv: 'dbc_export_csv',
  dbcSqlFileRead: 'dbc_sql_file_read',
  dbcSqlFileWrite: 'dbc_sql_file_write',
  dbcExportRows: 'dbc_export_rows',
  dbcExportQuery: 'dbc_export_query',
  dbcRedisKeys: 'dbc_redis_keys',
  dbcRedisKeyInfo: 'dbc_redis_key_info',
  dbcCharsetOptions: 'dbc_charset_options',
  dbcUsers: 'dbc_users',
  dbcCreateDatabase: 'dbc_create_database',
  dbcDropDatabase: 'dbc_drop_database',
  dbcTableAdmin: 'dbc_table_admin',
  dbcTableDdl: 'dbc_table_ddl',
  dbcTableIndexes: 'dbc_table_indexes',
} as const

/** 命令入参 */
export type Payloads = {
  dbc_connection_save: { config: ConnConfig; password: string; clearPassword?: boolean }
  dbc_connection_delete: { id: string }
  dbc_connections: Record<string, never>
  dbc_connect: { id: string }
  dbc_disconnect: { id: string }
  dbc_test: { config: ConnConfig; password: string; clearPassword?: boolean }
  dbc_drafts: Record<string, never>
  dbc_drafts_save: { drafts: QueryDraft[] }
  dbc_history: Record<string, never>
  dbc_history_add: {
    connId: string
    sql: string
    status: string
    durationMs: number
    scope?: ExecutionScope
  }
  dbc_history_clear: Record<string, never>
  dbc_saved: Record<string, never>
  dbc_saved_add: { title: string; sql: string }
  dbc_saved_update: { id: number; title: string; sql: string }
  dbc_saved_delete: { id: number }
  dbc_driver_status: { dbType: string }
  dbc_execute: {
    connId: string
    sql: string
    maxRows?: number
    requestId: string
    workspaceId?: string
    scope?: ExecutionScope
    confirmationToken?: string
  }
  dbc_prepare_execution: { connId: string; sql: string; requestId: string; scope: ExecutionScope }
  dbc_workspace_close: { connId: string; workspaceId: string }
  dbc_cancel: { requestId: string }
  dbc_databases: { connId: string }
  dbc_schemas: { connId: string; database?: string }
  dbc_objects: { connId: string; database?: string; schema?: string }
  dbc_columns: { connId: string; database?: string; schema?: string; table: string }
  dbc_table_data: {
    connId: string
    database?: string
    schema?: string
    table: string
    page: number
    pageSize: number
    options?: TableOptions
  }
  dbc_table_count: TableTarget & { options?: TableOptions; requestId: string }
  dbc_table_apply: TableTarget & { changes: TableChange[]; requestId: string }
  dbc_csv_preview: { path: string }
  dbc_csv_import: TableTarget & {
    path: string
    fingerprint: string
    mapping: CsvMapping[]
    requestId: string
  }
  dbc_export_csv: { path: string; text: string }
  dbc_sql_file_read: { path: string }
  dbc_sql_file_write: { path: string; sql: string }
  dbc_export_rows: { path: string; columns: string[]; rows: DbValue[][] }
  dbc_export_query: {
    connId: string
    scope: ExecutionScope
    sql: string
    path: string
    requestId: string
  }
  dbc_redis_keys: { connId: string; pattern: string; cursor: number }
  dbc_redis_key_info: { connId: string; key: string }
  dbc_charset_options: { connId: string }
  dbc_users: { connId: string }
  dbc_create_database: {
    connId: string
    name: string
    charset?: string
    collation?: string
    grants?: DbGrantInput[]
  }
  dbc_drop_database: { connId: string; name: string }
  dbc_table_admin: {
    connId: string
    database?: string
    schema?: string
    table: string
    action: 'rename' | 'truncate' | 'drop'
    newName?: string
    kind?: string
  }
  dbc_table_ddl: { connId: string; database?: string; schema?: string; table: string }
  dbc_table_indexes: { connId: string; database?: string; schema?: string; table: string }
}

/** 命令返回 */
export type Results = {
  dbc_connection_save: void
  dbc_connection_delete: void
  dbc_connections: DbConnectionInfo[]
  dbc_connect: DbConnectionInfo
  dbc_disconnect: void
  dbc_test: string
  dbc_drafts: QueryDraft[]
  dbc_drafts_save: void
  dbc_history: HistoryEntry[]
  dbc_history_add: void
  dbc_history_clear: void
  dbc_saved: SavedEntry[]
  dbc_saved_add: number
  dbc_saved_update: void
  dbc_saved_delete: void
  dbc_driver_status: DriverStatus
  dbc_prepare_execution: ExecutionPreview
  dbc_workspace_close: void
  dbc_execute: QueryResult
  dbc_cancel: void
  dbc_databases: string[]
  dbc_schemas: string[]
  dbc_objects: DbObjectInfo[]
  dbc_columns: DbColumnInfo[]
  dbc_table_data: DbTablePage
  dbc_table_count: string
  dbc_table_apply: number
  dbc_csv_preview: CsvPreview
  dbc_csv_import: number
  dbc_export_csv: void
  dbc_sql_file_read: string
  dbc_sql_file_write: void
  dbc_export_rows: number
  dbc_export_query: number
  dbc_redis_keys: [number, string[]]
  dbc_redis_key_info: RedisKeyInfo
  dbc_charset_options: DbCharsetOptions
  dbc_users: DbUserInfo[]
  dbc_create_database: DbStepResult[]
  dbc_drop_database: string
  dbc_table_admin: string
  dbc_table_ddl: string
  dbc_table_indexes: DbIndexInfo[]
}

/** 单表筛选、排序与参数化行变更。 */
export interface TableFilter {
  column: string
  operator:
    'eq' | 'ne' | 'lt' | 'le' | 'gt' | 'ge' | 'like' | 'isNull' | 'notNull' | 'in' | 'between'
  values?: DbValue[]
  value: DbValue
}
export interface TableSort {
  column: string
  descending: boolean
}
export interface TableOptions {
  filters: TableFilter[]
  sort: TableSort[]
}
export interface TableTarget {
  connId: string
  database?: string
  schema?: string
  table: string
}
export interface TableChange {
  action: 'insert' | 'update' | 'delete'
  original: Record<string, DbValue>
  values: Record<string, DbValue>
}

export interface CsvPreview {
  columns: string[]
  rows: string[][]
  total: number
  fingerprint: string
}
export interface CsvMapping {
  source: number
  column: string
}

/** 可恢复文档；不包含查询结果或活动事务。 */
export interface QueryDraft {
  label: string
  sql: string
  connectionId: string
  database: string
  schema: string
  from: number
  to: number
  dirty: boolean
  active: boolean
  savedId?: number | null
  savedTitle?: string | null
  filePath?: string | null
}
