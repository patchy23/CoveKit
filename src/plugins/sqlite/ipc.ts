/**
 * SQLite 数据库插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from "@/core/ipc/ipc";
import type { DbOpenResult, DbQueryResult } from "./contracts";

export const ipc = {
  dbOpen: (path: string): Promise<DbOpenResult> => invokeCommand("db_open", { path }),
  dbClose: (): Promise<void> => invokeCommand("db_close", {}),
  dbTables: (): Promise<string[]> => invokeCommand("db_tables", {}),
  dbExecute: (sql: string): Promise<DbQueryResult> => invokeCommand("db_execute", { sql }),
  dbQueryTable: (table: string, limit = 100): Promise<DbQueryResult> =>
    invokeCommand("db_query_table", { table, limit }),
};
