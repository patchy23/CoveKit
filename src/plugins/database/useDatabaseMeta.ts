/**
 * database 类型元信息（常量与纯函数，无状态；供数据层与组件共用）
 * 图标资源来自 dbx 项目 apps/desktop/public/icons/database（Apache-2.0）。
 */
import mysqlIcon from '@/assets/db-icons/mysql.svg'
import postgresqlIcon from '@/assets/db-icons/postgresql.svg'
import oracleIcon from '@/assets/db-icons/oracle.svg'
import sqliteIcon from '@/assets/db-icons/sqlite.svg'
import redisIcon from '@/assets/db-icons/redis.svg'
import damengIcon from '@/assets/db-icons/dameng.svg'
import vastbaseIcon from '@/assets/db-icons/vastbase.svg'
import kingbaseIcon from '@/assets/db-icons/kingbase.svg'
import polardbIcon from '@/assets/db-icons/polardb.webp'

/** 支持的数据库类型（与后端 DbType 一致；dameng 后端暂未实现） */
export type V2DbType =
  | 'mysql'
  | 'postgresql'
  | 'oracle'
  | 'dameng'
  | 'vastbase'
  | 'kingbase'
  | 'polardb'
  | 'redis'
  | 'sqlite'

/** 类型元信息：显示名 + logo 图标（新建对话框、树节点、工具栏徽标共用） */
export const DB_TYPE_META: Record<V2DbType, { label: string; icon: string }> = {
  mysql: { label: 'MySQL', icon: mysqlIcon },
  postgresql: { label: 'PostgreSQL', icon: postgresqlIcon },
  oracle: { label: 'Oracle', icon: oracleIcon },
  dameng: { label: '达梦', icon: damengIcon },
  vastbase: { label: 'Vastbase', icon: vastbaseIcon },
  kingbase: { label: 'Kingbase', icon: kingbaseIcon },
  polardb: { label: 'PolarDB', icon: polardbIcon },
  redis: { label: 'Redis', icon: redisIcon },
  sqlite: { label: 'SQLite', icon: sqliteIcon },
}

/** 新建连接对话框的类型选项（九宫格，含图标） */
export const DB_TYPE_OPTIONS = (Object.keys(DB_TYPE_META) as V2DbType[]).map((value) => ({
  value,
  ...DB_TYPE_META[value],
}))

/** 默认端口（新建对话框预填） */
export const DEFAULT_PORT: Record<V2DbType, number> = {
  mysql: 3306,
  postgresql: 5432,
  oracle: 1521,
  dameng: 5236,
  vastbase: 5432,
  kingbase: 54321,
  polardb: 3306,
  redis: 6379,
  sqlite: 0,
}

/** 数据库 → schema → 对象分组（PG 系；polardb 为 MySQL 兼容，不在此列） */
const SCHEMA_TREE_TYPES = new Set<V2DbType>(['postgresql', 'kingbase', 'vastbase'])
/** 连接根直接挂 schema/用户，无 database 层（单库类型） */
const CONNECTION_ROOT_SCHEMA_TYPES = new Set<V2DbType>(['oracle', 'dameng'])

/** PG 系：连接 → 数据库 → schema → 分组 */
export function usesSchemaTree(type: V2DbType): boolean {
  return SCHEMA_TREE_TYPES.has(type)
}

/** Oracle/达梦：单库，连接 → schema（用户）→ 分组 */
export function usesConnectionRootSchema(type: V2DbType): boolean {
  return CONNECTION_ROOT_SCHEMA_TYPES.has(type)
}

/** 各类型新建连接的默认数据库名 */
export function defaultDatabaseFor(type: V2DbType): string {
  if (type === 'sqlite') return 'main'
  if (type === 'redis') return 'db0'
  if (type === 'oracle') return 'ORCL'
  if (type === 'dameng') return 'DAMENG'
  return 'patchybox'
}

/** 工作台页签（id/label/kind；kind 支持 redis 键页签） */
export interface V2Tab {
  /** 页签唯一 id */
  id: string
  /** 页签标题 */
  label: string
  /** 页签业务类型 */
  kind: V2TabKind
  /** 内容是否未保存（编辑器脏标记） */
  dirty?: boolean
}

/** 新建连接对话框的类型选项（九宫格，含图标） */
export type V2Env = '生产' | '测试' | '开发'
export type V2Status = 'online' | 'offline' | 'connecting'
export type V2TabKind = 'query' | 'data' | 'structure' | 'redis'
export type V2QueryStatus = 'idle' | 'running' | 'success' | 'error' | 'empty' | 'cancelled'

/** 是否 SQLite（新建对话框切换为文件选择） */
export function isFileType(type: V2DbType): boolean {
  return type === 'sqlite'
}

/** 是否 Redis（无 schema 语义） */
export function isRedisType(type: V2DbType): boolean {
  return type === 'redis'
}

/** 是否 agent 驱动类型（连接前提示驱动就绪状态） */
export function isAgentType(type: V2DbType): boolean {
  return type === 'oracle' || type === 'vastbase' || type === 'kingbase'
}

/** 是否 dameng（后端暂未实现，连接时提示） */
export function isUnsupportedType(type: V2DbType): boolean {
  return type === 'dameng'
}
