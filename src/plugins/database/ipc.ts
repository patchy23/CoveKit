/**
 * 数据库工作台插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type {
  ConnConfig,
  TableOptions,
  TableTarget,
  TableChange,
  CsvMapping,
  ExecutionScope,
  DbValue,
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
  save: (config: ConnConfig, password: string, clearPassword = false): Promise<void> =>
    call('dbc_connection_save', { config, password, clearPassword }),
  remove: (id: string): Promise<void> => call('dbc_connection_delete', { id }),
  connect: (id: string): Promise<DbConnectionInfo> => call('dbc_connect', { id }),
  disconnect: (id: string): Promise<void> => call('dbc_disconnect', { id }),
  test: (config: ConnConfig, password: string, clearPassword = false): Promise<string> =>
    call('dbc_test', { config, password, clearPassword }),
  driverStatus: (dbType: string): Promise<DriverStatus> => call('dbc_driver_status', { dbType }),
}

/** 查询与元数据 */
export const queryIpc = {
  /** 执行 SQL；`requestId` 是本次请求身份，取消（`cancel`）按它命中，同连接多页签互不牵连 */
  execute: (
    connId: string,
    sql: string,
    maxRows: number | undefined,
    requestId: string,
    workspaceId?: string,
    scope?: ExecutionScope,
    confirmationToken?: string
  ): Promise<QueryResult> =>
    call('dbc_execute', { connId, sql, maxRows, requestId, workspaceId, scope, confirmationToken }),
  prepare: (connId: string, sql: string, requestId: string, scope: ExecutionScope) =>
    call('dbc_prepare_execution', { connId, sql, requestId, scope }),
  closeWorkspace: (connId: string, workspaceId: string) =>
    call('dbc_workspace_close', { connId, workspaceId }),
  cancel: (requestId: string): Promise<void> => call('dbc_cancel', { requestId }),
  databases: (connId: string): Promise<string[]> => call('dbc_databases', { connId }),
  schemas: (connId: string, database?: string): Promise<string[]> =>
    call('dbc_schemas', { connId, database }),
  objects: (connId: string, schema?: string, database?: string): Promise<DbObjectInfo[]> =>
    call('dbc_objects', { connId, schema, database }),
  columns: (
    connId: string,
    table: string,
    schema?: string,
    database?: string
  ): Promise<import('./contracts').DbColumnInfo[]> =>
    call('dbc_columns', { connId, schema, table, database }),
  tableData: (
    connId: string,
    table: string,
    page: number,
    pageSize: number,
    schema?: string,
    database?: string,
    options?: TableOptions
  ): Promise<DbTablePage> =>
    call('dbc_table_data', { connId, schema, table, page, pageSize, database, options }),
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
    options?: { database?: string; schema?: string; newName?: string; kind?: string }
  ): Promise<string> =>
    call('dbc_table_admin', {
      connId,
      database: options?.database,
      schema: options?.schema,
      table,
      action,
      newName: options?.newName,
      kind: options?.kind,
    }),
  tableDdl: (connId: string, table: string, schema?: string, database?: string): Promise<string> =>
    call('dbc_table_ddl', { connId, schema, table, database }),
  tableIndexes: (
    connId: string,
    table: string,
    schema?: string,
    database?: string
  ): Promise<DbIndexInfo[]> => call('dbc_table_indexes', { connId, schema, table, database }),
}

/** 历史与收藏 */
export const historyIpc = {
  list: (): Promise<HistoryEntry[]> => call('dbc_history', {}),
  add: (
    connId: string,
    sql: string,
    status: string,
    durationMs: number,
    scope?: ExecutionScope
  ): Promise<void> => call('dbc_history_add', { connId, sql, status, durationMs, scope }),
  clear: (): Promise<void> => call('dbc_history_clear', {}),
}

export const savedIpc = {
  list: (): Promise<SavedEntry[]> => call('dbc_saved', {}),
  add: (title: string, sql: string): Promise<number> => call('dbc_saved_add', { title, sql }),
  update: (id: number, title: string, sql: string): Promise<void> =>
    call('dbc_saved_update', { id, title, sql }),
  remove: (id: number): Promise<void> => call('dbc_saved_delete', { id }),
}

/** SQL 文件与有界结果导出。 */
export const fileIpc = {
  exportQuery: (
    connId: string,
    scope: ExecutionScope,
    sql: string,
    path: string,
    requestId: string
  ) => call('dbc_export_query', { connId, scope, sql, path, requestId }),
  readSql: (path: string) => call('dbc_sql_file_read', { path }),
  writeSql: (path: string, sql: string) => call('dbc_sql_file_write', { path, sql }),
  exportRows: (path: string, columns: string[], rows: DbValue[][]) =>
    call('dbc_export_rows', { path, columns, rows }),
}

export const tableIpc = {
  count: (target: TableTarget, options: TableOptions | undefined, requestId: string) =>
    call('dbc_table_count', { ...target, options, requestId }),
  apply: (target: TableTarget, changes: TableChange[], requestId: string) =>
    call('dbc_table_apply', { ...target, changes, requestId }),
}

export const csvIpc = {
  preview: (path: string) => call('dbc_csv_preview', { path }),
  import: (
    target: TableTarget,
    path: string,
    fingerprint: string,
    mapping: CsvMapping[],
    requestId: string
  ) => call('dbc_csv_import', { ...target, path, fingerprint, mapping, requestId }),
}

export const draftIpc = {
  list: () => call('dbc_drafts', {}),
  save: (drafts: import('./contracts').QueryDraft[]) => call('dbc_drafts_save', { drafts }),
}
