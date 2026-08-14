/**
 * database 模拟数据层
 * 作用：驱动 DatabaseWorkbench 的纯前端交互，不连接任何真实数据库
 * 数据模型：
 *  - V2Connection       连接（含环境/类型/状态/时延）
 *  - V2Tab              工作台页签（query/data/structure 三种）
 *  - V2QueryState       单个查询页签的 SQL/结果状态
 *  - V2HistoryEntry     查询历史条目（按时间倒序）
 *  - V2SavedEntry       收藏的 SQL
 * 备注：所有 timer 在 onBeforeUnmount 清理
 */
import { computed, onBeforeUnmount, ref } from 'vue'
import type { UiDataGridColumn, UiTabItem, UiTreeItem, UiWorkbenchTab } from '@/core/ui'
import mysqlIcon from '@/assets/db-icons/mysql.svg'
import postgresqlIcon from '@/assets/db-icons/postgresql.svg'
import oracleIcon from '@/assets/db-icons/oracle.svg'
import sqliteIcon from '@/assets/db-icons/sqlite.svg'
import redisIcon from '@/assets/db-icons/redis.svg'
import damengIcon from '@/assets/db-icons/dameng.svg'
import vastbaseIcon from '@/assets/db-icons/vastbase.svg'
import kingbaseIcon from '@/assets/db-icons/kingbase.svg'
import polardbIcon from '@/assets/db-icons/polardb.webp'

/** 支持的数据库类型（图标资源来自 dbx 项目 apps/desktop/public/icons/database） */
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

// ──────────────────────────────────────────────────────────────────────────
// 树层级能力（参考 dbx lib/database/databaseCapabilitySets.ts）
// ──────────────────────────────────────────────────────────────────────────

/** 数据库 → schema → 对象分组（PG 系：TREE_SCHEMA_TYPES） */
const SCHEMA_TREE_TYPES = new Set<V2DbType>(['postgresql', 'polardb', 'kingbase', 'vastbase'])
/** 连接根直接挂 schema/用户，无 database 层（CONNECTION_ROOT_SCHEMA_TYPES + SINGLE_DATABASE_TYPES） */
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
export type V2Env = '生产' | '测试' | '开发'
export type V2Status = 'online' | 'offline' | 'connecting'
export type V2TabKind = 'query' | 'data' | 'structure'
export type V2QueryStatus = 'idle' | 'running' | 'success' | 'error' | 'empty' | 'cancelled'

export interface V2Connection {
  id: string
  label: string
  type: V2DbType
  env: V2Env
  status: V2Status
  version: string
  latency: number
  readonly?: boolean
  host: string
  database: string
}

export interface V2Tab extends UiWorkbenchTab {
  kind: V2TabKind
}

export interface V2QueryState {
  sql: string
  status: V2QueryStatus
  error: string
  resultTab: string
  filter: string
  page: number
  sortAsc: boolean
  selectedRow: string
  dirty: boolean
  durationMs: number
  affected: number
}

export interface V2HistoryEntry {
  id: string
  sql: string
  connectionId: string
  at: string
  durationMs: number
  status: 'success' | 'error'
}

export interface V2SavedEntry {
  id: string
  title: string
  sql: string
}

export interface V2StructureColumn {
  name: string
  type: string
  nullable: string
  defaultValue: string
  key: string
  comment?: string
}

const DEFAULT_SQL = `SELECT id, username, display_name, email, status, created_at
FROM users
WHERE status = 'active'
ORDER BY created_at DESC
LIMIT 100;`

function makeQueryState(sql = DEFAULT_SQL): V2QueryState {
  return {
    sql,
    status: 'success',
    error: '',
    resultTab: 'data',
    filter: '',
    page: 1,
    sortAsc: false,
    selectedRow: '1',
    dirty: false,
    durationMs: 36,
    affected: 48,
  }
}

const CONNECTIONS_SEED: V2Connection[] = [
  {
    id: 'mysql-prod',
    label: '生产 · 订单库',
    type: 'mysql',
    env: '生产',
    status: 'online',
    version: '8.0.36',
    latency: 18,
    readonly: true,
    host: 'mysql-prod.internal:3306',
    database: 'patchybox',
  },
  {
    id: 'pg-dev',
    label: '开发 · PG 主库',
    type: 'postgresql',
    env: '开发',
    status: 'online',
    version: '16.3',
    latency: 6,
    host: '127.0.0.1:5432',
    database: 'patchybox',
  },
  {
    id: 'oracle-test',
    label: '测试 · Oracle',
    type: 'oracle',
    env: '测试',
    status: 'offline',
    version: '19c',
    latency: 0,
    host: 'ora-test.internal:1521',
    database: 'ORCL',
  },
  {
    id: 'dameng-test',
    label: '测试 · 达梦',
    type: 'dameng',
    env: '测试',
    status: 'online',
    version: 'DM8',
    latency: 12,
    host: 'dm-test.internal:5236',
    database: 'DAMENG',
  },
  {
    id: 'kingbase-dev',
    label: '开发 · 人大金仓',
    type: 'kingbase',
    env: '开发',
    status: 'online',
    version: 'V8R6',
    latency: 9,
    host: '127.0.0.1:54321',
    database: 'patchybox',
  },
  {
    id: 'vastbase-dev',
    label: '开发 · Vastbase',
    type: 'vastbase',
    env: '开发',
    status: 'online',
    version: 'G100 2.2',
    latency: 11,
    host: '127.0.0.1:5434',
    database: 'patchybox',
  },
  {
    id: 'polardb-prod',
    label: '生产 · PolarDB',
    type: 'polardb',
    env: '生产',
    status: 'online',
    version: '2.0 (PG 14)',
    latency: 22,
    readonly: true,
    host: 'polardb-prod.rds.aliyuncs.com:5432',
    database: 'patchybox',
  },
  {
    id: 'redis-cache',
    label: '缓存 · Redis',
    type: 'redis',
    env: '开发',
    status: 'online',
    version: '7.2',
    latency: 2,
    host: '127.0.0.1:6379',
    database: 'db0',
  },
  {
    id: 'sqlite-local',
    label: '本地 SQLite',
    type: 'sqlite',
    env: '开发',
    status: 'online',
    version: '3.46',
    latency: 1,
    host: 'file:./data/app.db',
    database: 'main',
  },
]

const TABLE_COLUMNS: UiDataGridColumn[] = [
  { key: 'id', label: 'id', width: 56, align: 'right', content: 'numeric' },
  { key: 'username', label: 'username', width: 108, content: 'technical' },
  { key: 'displayName', label: 'display_name', width: 118 },
  { key: 'email', label: 'email', width: 196, content: 'technical' },
  { key: 'status', label: 'status', width: 78, content: 'status' },
  { key: 'createdAt', label: 'created_at', width: 148, content: 'technical' },
]

const STRUCTURE_COLUMNS: V2StructureColumn[] = [
  {
    name: 'id',
    type: 'bigint',
    nullable: '否',
    defaultValue: 'identity',
    key: 'PK',
    comment: '主键',
  },
  {
    name: 'username',
    type: 'varchar(64)',
    nullable: '否',
    defaultValue: 'NULL',
    key: '—',
    comment: '登录名',
  },
  {
    name: 'display_name',
    type: 'varchar(128)',
    nullable: '是',
    defaultValue: 'NULL',
    key: '—',
    comment: '显示名',
  },
  {
    name: 'email',
    type: 'varchar(255)',
    nullable: '否',
    defaultValue: 'NULL',
    key: 'UK',
    comment: '邮箱',
  },
  {
    name: 'status',
    type: 'varchar(16)',
    nullable: '否',
    defaultValue: "'active'",
    key: '—',
    comment: '状态',
  },
  {
    name: 'created_at',
    type: 'timestamp',
    nullable: '否',
    defaultValue: 'now()',
    key: '—',
    comment: '创建时间',
  },
]

function makeRows() {
  const names = ['林晓', '陈嘉祺', '王明远', '周宁']
  return Array.from({ length: 48 }, (_, index) => ({
    id: index + 1,
    username: `user_${String(index + 1).padStart(3, '0')}`,
    displayName: names[index % names.length],
    email: `user${index + 1}@patchybox.dev`,
    status: index % 9 === 0 ? 'locked' : 'active',
    createdAt: `2026-08-${String((index % 12) + 1).padStart(2, '0')} 10:${String(index).padStart(2, '0')}:26`,
  }))
}

const HISTORY_SEED: V2HistoryEntry[] = [
  {
    id: 'h1',
    sql: "SELECT id, username FROM users WHERE status = 'active' LIMIT 100;",
    connectionId: 'mysql-prod',
    at: '10:24',
    durationMs: 36,
    status: 'success',
  },
  {
    id: 'h2',
    sql: "UPDATE orders SET status = 'paid' WHERE id = 8123;",
    connectionId: 'mysql-prod',
    at: '10:18',
    durationMs: 54,
    status: 'success',
  },
  {
    id: 'h3',
    sql: 'SELEC * FROM users;',
    connectionId: 'pg-dev',
    at: '09:56',
    durationMs: 12,
    status: 'error',
  },
  {
    id: 'h4',
    sql: 'SELECT count(*) FROM audit_logs GROUP BY action;',
    connectionId: 'pg-dev',
    at: '09:41',
    durationMs: 320,
    status: 'success',
  },
  {
    id: 'h5',
    sql: 'SELECT * FROM sessions WHERE expires_at < now();',
    connectionId: 'sqlite-local',
    at: '09:12',
    durationMs: 4,
    status: 'success',
  },
]

const SAVED_SEED: V2SavedEntry[] = [
  { id: 's1', title: '活跃用户', sql: "SELECT * FROM users WHERE status = 'active';" },
  {
    id: 's2',
    title: '近 7 日订单',
    sql: "SELECT * FROM orders WHERE created_at > now() - interval '7 days';",
  },
  { id: 's3', title: '锁表排查', sql: 'SELECT * FROM pg_locks WHERE NOT granted;' },
]

export function useDatabaseMock() {
  const connections = ref<V2Connection[]>(CONNECTIONS_SEED.map((item) => ({ ...item })))
  const activeConnectionId = ref('mysql-prod')
  const selectedResource = ref('mysql-prod::table:users')
  const keyword = ref('')
  const metadataLoading = ref(false)

  // 初始只保留查询页签；数据/结构页签由树节点点击动态创建
  const tabs = ref<V2Tab[]>([{ id: 'q1', label: '查询 1', kind: 'query', dirty: false }])
  const activeTabId = ref('q1')

  const tabContexts = ref<
    Record<string, { connectionId: string; database: string; schema: string }>
  >({
    q1: { connectionId: 'mysql-prod', database: 'patchybox', schema: 'public' },
  })

  const queryStates = ref<Record<string, V2QueryState>>({
    q1: makeQueryState(),
  })
  let querySequence = 1
  const timers = new Map<string, ReturnType<typeof setTimeout>>()
  let metadataTimer: ReturnType<typeof setTimeout> | undefined

  const history = ref<V2HistoryEntry[]>([...HISTORY_SEED])
  const savedSql = ref<V2SavedEntry[]>([...SAVED_SEED])

  const rows = ref(makeRows())

  // ──────────────────────────────────────────────────────────────────────────
  // 派生：当前连接 / 当前 tab / 当前查询状态
  // ──────────────────────────────────────────────────────────────────────────

  const activeConnection = computed<V2Connection>(
    () =>
      connections.value.find((connection) => connection.id === activeConnectionId.value) ??
      connections.value[0] ??
      CONNECTIONS_SEED[0]
  )

  const activeTab = computed<V2Tab | undefined>(() =>
    tabs.value.find((tab) => tab.id === activeTabId.value)
  )

  const activeTabKind = computed<V2TabKind>(() => activeTab.value?.kind ?? 'query')

  const activeTabContext = computed(
    () =>
      tabContexts.value[activeTabId.value] ?? {
        connectionId: activeConnectionId.value,
        database: 'patchybox',
        schema: 'public',
      }
  )

  const activeTabConnection = computed<V2Connection>(
    () =>
      connections.value.find(
        (connection) => connection.id === activeTabContext.value.connectionId
      ) ?? activeConnection.value
  )

  function ensureQueryState(id: string): V2QueryState {
    queryStates.value[id] ??= makeQueryState()
    return queryStates.value[id]
  }

  const queryState = computed(() => ensureQueryState(activeTabId.value))

  function patchQueryState(patch: Partial<V2QueryState>) {
    const state = ensureQueryState(activeTabId.value)
    Object.assign(state, patch)
  }

  // ──────────────────────────────────────────────────────────────────────────
  // 下拉选项
  // ──────────────────────────────────────────────────────────────────────────

  const connectionOptions = computed(() =>
    connections.value.map((connection) => ({
      value: connection.id,
      label: connection.label,
    }))
  )

  const databaseOptions = [
    { value: 'patchybox', label: 'patchybox' },
    { value: 'information_schema', label: 'information_schema' },
  ]
  const schemaOptions = [
    { value: 'public', label: 'public' },
    { value: 'audit', label: 'audit' },
  ]
  const rowLimitOptions = ['50', '100', '500', '1000'].map((value) => ({
    value,
    label: `${value}`,
  }))

  // ──────────────────────────────────────────────────────────────────────────
  // 左侧对象树（按连接分组 → 库 → schema → 表/视图/函数）
  // ──────────────────────────────────────────────────────────────────────────

  // ──────────────────────────────────────────────────────────────────────────
  // 左侧对象树（连接 → 数据库 → schema → 表/视图/函数，每个连接独立对象集）
  // 参考 dbx ConnectionTree：连接节点可展开，含类型徽章与连接状态点
  // ──────────────────────────────────────────────────────────────────────────

  /** 每个连接独立的表集（按连接 id 区分，模拟真实数据库对象差异） */
  const TABLE_SETS: Record<string, string[]> = {
    'mysql-prod': ['users', 'orders', 'audit_logs', 'sessions', 'payments'],
    'pg-dev': ['users', 'orders', 'products', 'inventory', 'pg_stat_activity'],
    'sqlite-local': ['app_config', 'cache_entries', 'download_tasks'],
    'oracle-test': ['EMP', 'DEPT', 'BONUS'],
    'dameng-test': ['SYS_USER', 'ORDERS', 'PRODUCTS'],
    'kingbase-dev': ['users', 'orders', 'products'],
    'vastbase-dev': ['users', 'orders', 'products'],
    'polardb-prod': ['users', 'orders', 'audit_logs'],
  }

  /** Redis 示例键（redis 连接无表结构，叶子为 key） */
  const REDIS_KEYS = [
    'session:1001',
    'session:1002',
    'cache:hot-list',
    'queue:jobs',
    'lock:order:8123',
  ]

  /** schema 集（oracle/dameng 的 schema 即用户；PG 系为数据库内 schema） */
  const SCHEMA_SETS: Partial<Record<V2DbType, string[]>> = {
    postgresql: ['public', 'audit'],
    polardb: ['public', 'audit'],
    kingbase: ['public', 'audit'],
    vastbase: ['public', 'audit'],
    oracle: ['SCOTT', 'HR'],
    dameng: ['SYSDBA', 'TEST'],
  }

  function tablesFor(connection: V2Connection): string[] {
    return TABLE_SETS[connection.id] ?? ['users', 'orders', 'audit_logs']
  }

  function schemasFor(connection: V2Connection): string[] {
    return SCHEMA_SETS[connection.type] ?? ['public']
  }

  /**
   * 对象分组（参考 dbx TreeNodeType 的 group-*：按数据库类型裁剪）
   * children 为空的分组仅展示数量徽标（模拟数据不展开明细）
   */
  interface V2ObjectGroup {
    key: string
    label: string
    childKind: string
    children?: string[]
    count?: number
  }

  function objectGroupsFor(connection: V2Connection): V2ObjectGroup[] {
    const tables = tablesFor(connection)
    switch (connection.type) {
      case 'mysql':
        return [
          { key: 'tables', label: '表', childKind: 'table', children: tables },
          { key: 'views', label: '视图', childKind: 'view', count: 2 },
          { key: 'funcs', label: '函数', childKind: 'function', count: 5 },
          { key: 'events', label: '事件', childKind: 'event', count: 1 },
        ]
      case 'postgresql':
      case 'polardb':
      case 'kingbase':
      case 'vastbase':
        return [
          { key: 'tables', label: '表', childKind: 'table', children: tables },
          { key: 'views', label: '视图', childKind: 'view', count: 2 },
          { key: 'funcs', label: '函数', childKind: 'function', count: 5 },
          { key: 'seqs', label: '序列', childKind: 'sequence', count: 3 },
        ]
      case 'oracle':
      case 'dameng':
        return [
          { key: 'tables', label: '表', childKind: 'table', children: tables },
          { key: 'views', label: '视图', childKind: 'view', count: 2 },
          { key: 'funcs', label: '函数', childKind: 'function', count: 5 },
          { key: 'procs', label: '存储过程', childKind: 'procedure', count: 3 },
          { key: 'packages', label: '包', childKind: 'package', count: 4 },
          { key: 'seqs', label: '序列', childKind: 'sequence', count: 3 },
          { key: 'synonyms', label: '同义词', childKind: 'synonym', count: 2 },
        ]
      case 'sqlite':
        return [
          { key: 'tables', label: '表', childKind: 'table', children: tables },
          { key: 'views', label: '视图', childKind: 'view', count: 1 },
          { key: 'indexes', label: '索引', childKind: 'index', count: 4 },
          { key: 'triggers', label: '触发器', childKind: 'trigger', count: 2 },
        ]
      case 'redis':
        return [{ key: 'keys', label: '键', childKind: 'key', children: REDIS_KEYS }]
    }
  }

  /** 树节点展开状态（独立持久化，避免 computed 重建丢失） */
  const expandedIds = ref<Set<string>>(
    new Set(['mysql-prod', 'mysql-prod::db', 'mysql-prod::tables'])
  )

  function isExpanded(id: string): boolean {
    return expandedIds.value.has(id)
  }

  function toggleExpanded(id: string) {
    const next = new Set(expandedIds.value)
    if (next.has(id)) {
      next.delete(id)
    } else {
      next.add(id)
    }
    expandedIds.value = next
  }

  /** 生成一个可展开节点（展开状态走 expandedIds） */
  function branch(
    id: string,
    label: string,
    depth: number,
    kind: string,
    badge?: string | number
  ): UiTreeItem {
    return { id, label, depth, kind, badge, expandable: true, expanded: isExpanded(id) }
  }

  /** 在 items 末尾追加一组对象分组及其叶子（scope 用于 schema 层连接的 id 隔离） */
  function pushGroups(
    items: UiTreeItem[],
    prefix: string,
    depth: number,
    connection: V2Connection
  ) {
    for (const group of objectGroupsFor(connection)) {
      items.push(
        branch(
          `${prefix}::${group.key}`,
          group.label,
          depth,
          'group',
          group.children?.length ?? group.count
        )
      )
      for (const name of group.children ?? []) {
        items.push({
          id: `${prefix}::${group.childKind}:${name}`,
          label: name,
          depth: depth + 1,
          kind: group.childKind,
        })
      }
    }
  }

  /**
   * 单连接树（层级随数据库类型变化，参考 dbx ConnectionTree 分派）：
   *  - mysql / sqlite / redis：连接 → 数据库 → 分组
   *  - postgresql / polardb / kingbase / vastbase：连接 → 数据库 → schema → 分组
   *  - oracle / dameng（单库）：连接 → schema（用户）→ 分组
   */
  function buildTreeForConnection(connection: V2Connection): UiTreeItem[] {
    const prefix = connection.id
    const headerBadge =
      connection.status === 'offline'
        ? '断开'
        : connection.readonly
          ? '只读'
          : DB_TYPE_META[connection.type].label
    const connected = connection.status === 'online'
    const items: UiTreeItem[] = [
      {
        id: prefix,
        label: connection.label,
        depth: 0,
        kind: 'connection',
        expandable: true,
        expanded: isExpanded(prefix),
        badge: headerBadge,
        muted: !connected,
      },
    ]

    if (usesConnectionRootSchema(connection.type)) {
      // Oracle / 达梦：连接根直接挂 schema（用户）
      for (const schema of schemasFor(connection)) {
        const scope = `${prefix}::${schema}`
        items.push(branch(scope, schema, 1, 'schema'))
        pushGroups(items, scope, 2, connection)
      }
      return items
    }

    // 其余类型：连接 → 数据库
    items.push(branch(`${prefix}::db`, connection.database, 1, 'database'))

    if (usesSchemaTree(connection.type)) {
      // PG 系：数据库 → schema → 分组
      for (const schema of schemasFor(connection)) {
        const scope = `${prefix}::${schema}`
        items.push(branch(scope, schema, 2, 'schema'))
        pushGroups(items, scope, 3, connection)
      }
      return items
    }

    // MySQL / SQLite / Redis：数据库下直接挂分组（redis 的分组即键）
    pushGroups(items, prefix, 2, connection)
    return items
  }

  const treeItems = computed<UiTreeItem[]>(() =>
    connections.value.flatMap((connection) => buildTreeForConnection(connection))
  )

  const visibleTreeItems = computed<UiTreeItem[]>(() => {
    const filter = keyword.value.trim().toLowerCase()
    const source = treeItems.value
    if (filter) {
      const kept = new Set<number>()
      source.forEach((item, index) => {
        if (!item.label.toLowerCase().includes(filter)) return
        kept.add(index)
        let depth = item.depth - 1
        for (let parent = index - 1; parent >= 0 && depth >= 0; parent -= 1) {
          if (source[parent].depth === depth) {
            kept.add(parent)
            depth -= 1
          }
        }
      })
      return source.filter((_, index) => kept.has(index))
    }
    const visible: UiTreeItem[] = []
    let hiddenDepth = Number.POSITIVE_INFINITY
    for (const item of source) {
      // 遇到同级或更上级节点才重置隐藏深度（严格小于，避免子节点误触发）
      if (item.depth < hiddenDepth) hiddenDepth = Number.POSITIVE_INFINITY
      if (item.depth >= hiddenDepth) continue
      visible.push(item)
      if (item.expandable && !item.expanded) hiddenDepth = item.depth + 1
    }
    return visible
  })

  // ──────────────────────────────────────────────────────────────────────────
  // 结果集（过滤 + 排序 + 分页）
  // ──────────────────────────────────────────────────────────────────────────

  const filteredRows = computed(() => {
    const filter = queryState.value.filter.trim().toLowerCase()
    const result = filter
      ? rows.value.filter((row) =>
          Object.values(row).some((value) => String(value).toLowerCase().includes(filter))
        )
      : rows.value
    return [...result].sort((a, b) => {
      const first = String(a.createdAt)
      const second = String(b.createdAt)
      return queryState.value.sortAsc ? first.localeCompare(second) : second.localeCompare(first)
    })
  })

  const PAGE_SIZE = 20
  const totalPages = computed(() => Math.max(1, Math.ceil(filteredRows.value.length / PAGE_SIZE)))
  const pageRows = computed(() =>
    filteredRows.value.slice(
      (queryState.value.page - 1) * PAGE_SIZE,
      queryState.value.page * PAGE_SIZE
    )
  )

  const resultTabs = computed<UiTabItem[]>(() => [
    {
      value: 'data',
      label: '数据',
      badge: queryState.value.status === 'success' ? filteredRows.value.length : undefined,
    },
    { value: 'message', label: '消息' },
    { value: 'plan', label: '执行计划' },
  ])

  // ──────────────────────────────────────────────────────────────────────────
  // 动作
  // ──────────────────────────────────────────────────────────────────────────

  function toggleTree(item: UiTreeItem) {
    toggleExpanded(item.id)
  }

  function selectResource(id: string) {
    selectedResource.value = id
    const connectionId = id.split('::')[0]
    if (connections.value.some((connection) => connection.id === connectionId)) {
      activeConnectionId.value = connectionId
    }
    // 叶子 id 末段固定为 ::table:<名> 或 ::key:<名>（schema 层连接的 id 带 schema 前缀）
    const leaf = id.match(/::(table|key):(.+)$/)
    if (leaf) {
      const name = leaf[2]
      // tab id 带连接前缀，避免不同连接的同名表互相覆盖
      openOrFocusTab(`data-${connectionId}-${name}`, `${name} · 数据`, 'data', connectionId)
    }
  }

  function openOrFocusTab(id: string, label: string, kind: V2TabKind, connectionId: string) {
    if (!tabs.value.some((tab) => tab.id === id)) {
      tabs.value.push({ id, label, kind })
    }
    tabContexts.value[id] = {
      connectionId,
      database: activeTabContext.value.database,
      schema: activeTabContext.value.schema,
    }
    activeTabId.value = id
  }

  function openStructure() {
    openOrFocusTab(
      `structure-${activeConnectionId.value}-users`,
      'users · 结构',
      'structure',
      activeConnectionId.value
    )
  }

  function openData() {
    openOrFocusTab(
      `data-${activeConnectionId.value}-users`,
      'users · 数据',
      'data',
      activeConnectionId.value
    )
  }

  function createQuery() {
    querySequence += 1
    const id = `q${querySequence}`
    tabs.value.push({ id, label: `查询 ${querySequence}`, kind: 'query' })
    queryStates.value[id] = makeQueryState('')
    queryStates.value[id].status = 'idle'
    tabContexts.value[id] = {
      connectionId: activeConnectionId.value,
      database: activeTabContext.value.database,
      schema: activeTabContext.value.schema,
    }
    activeTabId.value = id
  }

  function closeTab(id: string) {
    tabs.value = tabs.value.filter((tab) => tab.id !== id)
    delete tabContexts.value[id]
    delete queryStates.value[id]
    const timer = timers.get(id)
    if (timer) clearTimeout(timer)
    timers.delete(id)
    if (activeTabId.value === id) activeTabId.value = tabs.value[0]?.id ?? ''
  }

  function runQuery() {
    const tabId = activeTabId.value
    const state = ensureQueryState(tabId)
    if (state.status === 'running') return
    if (activeTabConnection.value.status !== 'online') {
      patchQueryState({
        status: 'error',
        error: '当前连接已断开，无法执行查询。请先重连或切换到可用连接。',
        resultTab: 'message',
      })
      return
    }
    const previous = timers.get(tabId)
    if (previous) clearTimeout(previous)
    patchQueryState({ status: 'running', error: '', page: 1 })
    const startedAt = Date.now()
    const timer = setTimeout(() => {
      timers.delete(tabId)
      const statement = state.sql.trim()
      if (/invalid|error/i.test(statement) || /^SELEC(?:\s|$)/i.test(statement)) {
        patchQueryState({
          status: 'error',
          error: /^SELEC(?:\s|$)/i.test(statement)
            ? 'ERROR 1064 · 模拟语法错误：SELECT 关键字拼写不完整。'
            : 'ERROR 1064 · 模拟语法错误：请检查当前语句附近的标识符。',
          resultTab: 'message',
        })
        pushHistory(state.sql, 'error', Date.now() - startedAt)
        return
      }
      const hasRows = filteredRows.value.length > 0
      patchQueryState({
        status: hasRows ? 'success' : 'empty',
        resultTab: 'data',
        durationMs: Date.now() - startedAt,
        affected: filteredRows.value.length,
      })
      pushHistory(state.sql, 'success', Date.now() - startedAt)
    }, 700)
    timers.set(tabId, timer)
  }

  function cancelQuery() {
    const tabId = activeTabId.value
    const state = ensureQueryState(tabId)
    if (state.status !== 'running') return
    const timer = timers.get(tabId)
    if (timer) clearTimeout(timer)
    timers.delete(tabId)
    patchQueryState({ status: 'cancelled', resultTab: 'message' })
  }

  function refreshMetadata() {
    metadataLoading.value = true
    if (metadataTimer) clearTimeout(metadataTimer)
    metadataTimer = setTimeout(() => {
      metadataLoading.value = false
    }, 600)
  }

  function pushHistory(sql: string, status: 'success' | 'error', durationMs: number) {
    const trimmed = sql.trim().replace(/\s+/g, ' ')
    if (!trimmed) return
    history.value.unshift({
      id: `h${Date.now()}`,
      sql: trimmed.length > 80 ? `${trimmed.slice(0, 80)}…` : trimmed,
      connectionId: activeTabContext.value.connectionId,
      at: new Date().toTimeString().slice(0, 5),
      durationMs,
      status,
    })
    if (history.value.length > 30) history.value.pop()
  }

  function applyHistory(entry: V2HistoryEntry) {
    if (activeTabKind.value !== 'query') createQuery()
    patchQueryState({ sql: entry.sql, dirty: true })
  }

  function applySaved(entry: V2SavedEntry) {
    if (activeTabKind.value !== 'query') createQuery()
    patchQueryState({ sql: entry.sql, dirty: true })
  }

  function saveCurrentSql(title: string) {
    const sql = queryState.value.sql.trim()
    if (!sql) return
    savedSql.value.unshift({
      id: `s${Date.now()}`,
      title: title || `未命名 · ${new Date().toTimeString().slice(0, 5)}`,
      sql,
    })
  }

  function removeSaved(id: string) {
    savedSql.value = savedSql.value.filter((entry) => entry.id !== id)
  }

  const showConnectionDialog = ref(false)
  const newConnectionType = ref<V2DbType>('postgresql')
  const newConnectionName = ref('PostgreSQL / 本地演示')

  function addConnection() {
    const id = `mock-${Date.now()}`
    connections.value.push({
      id,
      label: newConnectionName.value.trim() || '新建数据库连接',
      type: newConnectionType.value,
      env: '开发',
      status: 'online',
      version: '演示驱动',
      latency: 24,
      host: '127.0.0.1',
      database: defaultDatabaseFor(newConnectionType.value),
    })
    activeConnectionId.value = id
    showConnectionDialog.value = false
  }

  function removeConnection(id: string) {
    if (connections.value.length === 1) return
    connections.value = connections.value.filter((connection) => connection.id !== id)
    if (activeConnectionId.value === id) activeConnectionId.value = connections.value[0].id
  }

  function setPage(nextPage: number) {
    patchQueryState({ page: Math.min(Math.max(1, nextPage), totalPages.value) })
  }

  onBeforeUnmount(() => {
    timers.forEach((timer) => clearTimeout(timer))
    timers.clear()
    if (metadataTimer) clearTimeout(metadataTimer)
  })

  return {
    // 连接
    connections,
    activeConnection,
    activeConnectionId,
    connectionOptions,
    removeConnection,
    // tab
    tabs,
    activeTab,
    activeTabId,
    activeTabKind,
    activeTabContext,
    activeTabConnection,
    tabContexts,
    createQuery,
    closeTab,
    openData,
    openStructure,
    // 查询
    queryState,
    patchQueryState,
    runQuery,
    cancelQuery,
    // 树
    treeItems,
    visibleTreeItems,
    keyword,
    selectedResource,
    selectResource,
    toggleTree,
    metadataLoading,
    refreshMetadata,
    // 结果
    rows,
    filteredRows,
    pageRows,
    totalPages,
    setPage,
    resultTabs,
    tableColumns: TABLE_COLUMNS,
    structureColumns: STRUCTURE_COLUMNS,
    // 历史/收藏
    history,
    savedSql,
    applyHistory,
    applySaved,
    saveCurrentSql,
    removeSaved,
    // 连接对话框
    showConnectionDialog,
    newConnectionType,
    newConnectionName,
    addConnection,
    // 下拉
    databaseOptions,
    schemaOptions,
    rowLimitOptions,
  }
}
