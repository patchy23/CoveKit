/**
 * 数据库工作台插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type {
  ConnConfig,
  DbCharsetOptions,
  DbConnectionInfo,
  DbGrantInput,
  DbIndexInfo,
  DbObjectInfo,
  DbStepResult,
  DbTablePage,
  DbUserInfo,
  DriverStatus,
  HistoryEntry,
  Payloads,
  QueryResult,
  RedisKeyInfo,
  Results,
  SavedEntry,
} from './contracts'

async function call<K extends keyof Payloads & keyof Results>(
  command: K,
  payload: Payloads[K]
): Promise<Results[K]> {
  return invokeCommand<Payloads[K], Results[K]>(command, payload)
}

/** 连接管理 */
export const connectionIpc = {
  list: (): Promise<DbConnectionInfo[]> => call('dbc_connections', {}),
  save: (config: ConnConfig, password: string): Promise<void> =>
    call('dbc_connection_save', { config, password }),
  remove: (id: string): Promise<void> => call('dbc_connection_delete', { id }),
  connect: (id: string): Promise<DbConnectionInfo> => call('dbc_connect', { id }),
  disconnect: (id: string): Promise<void> => call('dbc_disconnect', { id }),
  test: (config: ConnConfig, password: string): Promise<string> =>
    call('dbc_test', { config, password }),
  driverStatus: (dbType: string): Promise<DriverStatus> => call('dbc_driver_status', { dbType }),
}

/** 查询与元数据 */
export const queryIpc = {
  /** 执行 SQL；`requestId` 是本次请求身份，取消（`cancel`）按它命中，同连接多页签互不牵连 */
  execute: (
    connId: string,
    sql: string,
    maxRows: number | undefined,
    requestId: string
  ): Promise<QueryResult> => call('dbc_execute', { connId, sql, maxRows, requestId }),
  cancel: (requestId: string): Promise<void> => call('dbc_cancel', { requestId }),
  databases: (connId: string): Promise<string[]> => call('dbc_databases', { connId }),
  schemas: (connId: string): Promise<string[]> => call('dbc_schemas', { connId }),
  objects: (connId: string, schema?: string): Promise<DbObjectInfo[]> =>
    call('dbc_objects', { connId, schema }),
  columns: (
    connId: string,
    table: string,
    schema?: string
  ): Promise<import('./contracts').DbColumnInfo[]> =>
    call('dbc_columns', { connId, schema, table }),
  tableData: (
    connId: string,
    table: string,
    page: number,
    pageSize: number,
    schema?: string
  ): Promise<DbTablePage> => call('dbc_table_data', { connId, schema, table, page, pageSize }),
  redisKeys: (connId: string, pattern: string, cursor: number): Promise<[number, string[]]> =>
    call('dbc_redis_keys', { connId, pattern, cursor }),
  redisKeyInfo: (connId: string, key: string): Promise<RedisKeyInfo> =>
    call('dbc_redis_key_info', { connId, key }),
}

/** 管理操作（建库/授权/DDL/索引/表维护；SQL 由后端方言构造，前端只传选项） */
export const adminIpc = {
  charsetOptions: (connId: string): Promise<DbCharsetOptions> =>
    call('dbc_charset_options', { connId }),
  users: (connId: string): Promise<DbUserInfo[]> => call('dbc_users', { connId }),
  createDatabase: (
    connId: string,
    name: string,
    charset?: string,
    collation?: string,
    grants?: DbGrantInput[]
  ): Promise<DbStepResult[]> =>
    call('dbc_create_database', { connId, name, charset, collation, grants }),
  dropDatabase: (connId: string, name: string): Promise<string> =>
    call('dbc_drop_database', { connId, name }),
  tableAdmin: (
    connId: string,
    table: string,
    action: 'rename' | 'truncate' | 'drop',
    options?: { schema?: string; newName?: string; kind?: string }
  ): Promise<string> =>
    call('dbc_table_admin', {
      connId,
      schema: options?.schema,
      table,
      action,
      newName: options?.newName,
      kind: options?.kind,
    }),
  tableDdl: (connId: string, table: string, schema?: string): Promise<string> =>
    call('dbc_table_ddl', { connId, schema, table }),
  tableIndexes: (connId: string, table: string, schema?: string): Promise<DbIndexInfo[]> =>
    call('dbc_table_indexes', { connId, schema, table }),
}

/** 历史与收藏 */
export const historyIpc = {
  list: (): Promise<HistoryEntry[]> => call('dbc_history', {}),
  add: (connId: string, sql: string, status: string, durationMs: number): Promise<void> =>
    call('dbc_history_add', { connId, sql, status, durationMs }),
  clear: (): Promise<void> => call('dbc_history_clear', {}),
}

export const savedIpc = {
  list: (): Promise<SavedEntry[]> => call('dbc_saved', {}),
  add: (title: string, sql: string): Promise<number> => call('dbc_saved_add', { title, sql }),
  update: (id: number, title: string, sql: string): Promise<void> =>
    call('dbc_saved_update', { id, title, sql }),
  remove: (id: number): Promise<void> => call('dbc_saved_delete', { id }),
}
