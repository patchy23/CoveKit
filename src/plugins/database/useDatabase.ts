/**
 * database 数据层（真实 IPC，替换 useDatabaseMock）
 * 职责：连接/会话状态、对象树元数据（懒加载）、查询页签状态机、历史/收藏。
 * 纯函数（树过滤/默认值）导出以便单测；所有 timer/异步请求在组件卸载时兜底。
 */
import { computed, onBeforeUnmount, ref } from 'vue'
import type { UiDataGridColumn, UiTabItem, UiTreeItem } from '@/core/ui'
import {
  DB_TYPE_META,
  type V2DbType,
  type V2QueryStatus,
  type V2Tab,
  type V2TabKind,
  isSystemSchema,
  usesConnectionRootSchema,
  usesSchemaTree,
} from './useDatabaseMeta'
import type {
  ConnConfig,
  DbColumnInfo,
  DbConnectionInfo,
  DbObjectInfo,
  HistoryEntry,
  SavedEntry,
} from './contracts'
import { connectionIpc, historyIpc, queryIpc, savedIpc } from './ipc'
import { formatSql } from './sqlFormat'

/**
 * 树过滤纯函数：关键字命中保留祖先链；无关键字时按展开状态裁剪子树
 * @param source 全量树节点（扁平数组，深度递增）
 * @param filter 小写关键字（空串 = 仅按展开状态裁剪）
 */
export function filterTreeItems(source: UiTreeItem[], filter: string): UiTreeItem[] {
  const kw = filter.trim().toLowerCase()
  if (!kw) {
    const visible: UiTreeItem[] = []
    let hiddenDepth = Number.POSITIVE_INFINITY
    for (const item of source) {
      if (item.depth < hiddenDepth) hiddenDepth = Number.POSITIVE_INFINITY
      if (item.depth >= hiddenDepth) continue
      visible.push(item)
      if (item.expandable && !item.expanded) hiddenDepth = item.depth + 1
    }
    return visible
  }
  const kept = new Set<number>()
  source.forEach((item, index) => {
    if (!item.label.toLowerCase().includes(kw)) return
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

/** 查询页签状态（真实后端字段） */
export interface QueryState {
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
  /** 结果列（动态） */
  columns: string[]
  /** 当前页数据行 */
  rows: string[][]
  /** 后端总数（分页） */
  total: number
  /** 是否截断 */
  truncated: boolean
  /** 已保存的收藏 id（二次保存直接 update，无需再确认） */
  savedId?: number
  /** 保存用的标题（首次保存确认后写入；重命名别名优先） */
  savedTitle?: string
}

/** 给 Promise 加超时兜底（后端挂起时前端也能报错收尾） */
export function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      }
    )
  })
}

/** 提取待执行 SQL：有选区取选区文本；无选区取光标所在整行（不含行尾换行） */
export function extractExecSql(text: string, start: number, end: number): string {
  if (start !== end) return text.slice(start, end)
  const lineStart = text.lastIndexOf('\n', Math.max(0, start - 1)) + 1
  const lineEndIdx = text.indexOf('\n', start)
  const lineEnd = lineEndIdx === -1 ? text.length : lineEndIdx
  return text.slice(lineStart, lineEnd)
}

function makeQueryState(sql = ''): QueryState {
  return {
    sql,
    status: 'idle',
    error: '',
    resultTab: 'data',
    filter: '',
    page: 1,
    sortAsc: false,
    selectedRow: '',
    dirty: false,
    durationMs: 0,
    affected: 0,
    columns: [],
    rows: [],
    total: 0,
    truncated: false,
  }
}

/** 每连接元数据（懒加载缓存） */
interface ConnMeta {
  databases: string[]
  schemas: string[]
  /** schemaKey（mysql 用 database；pg 用 schema）→ 对象列表 */
  objects: Record<string, DbObjectInfo[]>
  redisKeys: string[]
  loading: boolean
  loaded: boolean
}

/** 页签上下文：连接 + 库 + schema + 对象名（表/键） */
export interface TabContext {
  connectionId: string
  database: string
  schema: string
  /** 对象名（data/structure/redis 页签的目标表或键） */
  table?: string
}

const PAGE_SIZE = 50

export function useDatabase() {
  // ── 连接与会话 ──────────────────────────────────────────────────────────
  const connections = ref<DbConnectionInfo[]>([])
  const activeConnectionId = ref('')
  const connecting = ref<Record<string, boolean>>({})
  const connectError = ref<Record<string, string>>({})
  /** 连接取消标记（cancelConnect 置位；连接结果返回后据此丢弃并断开） */
  const cancelledConnect = ref<Record<string, boolean>>({})

  // ── 元数据缓存 ──────────────────────────────────────────────────────────
  const metas = ref<Record<string, ConnMeta>>({})

  // ── 页签 ────────────────────────────────────────────────────────────────
  const tabs = ref<V2Tab[]>([])
  const activeTabId = ref('')
  const tabContexts = ref<Record<string, TabContext>>({})
  const queryStates = ref<Record<string, QueryState>>({})
  const structureColumns = ref<Record<string, DbColumnInfo[]>>({})
  let tabSequence = 0

  // ── 历史 / 收藏 ─────────────────────────────────────────────────────────
  const history = ref<HistoryEntry[]>([])
  const savedSql = ref<SavedEntry[]>([])

  // ── 树 ──────────────────────────────────────────────────────────────────
  const keyword = ref('')
  const selectedResource = ref('')
  const expandedIds = ref<Set<string>>(new Set())

  // ──────────────────────────────────────────────────────────────────────
  // 派生
  // ──────────────────────────────────────────────────────────────────────

  const activeConnection = computed<DbConnectionInfo | undefined>(() =>
    connections.value.find((c) => c.id === activeConnectionId.value)
  )

  const activeTab = computed<V2Tab | undefined>(() =>
    tabs.value.find((t) => t.id === activeTabId.value)
  )

  const activeTabKind = computed<V2TabKind>(() => activeTab.value?.kind ?? 'query')

  const activeTabContext = computed<TabContext>(
    () =>
      tabContexts.value[activeTabId.value] ?? {
        connectionId: activeConnectionId.value,
        database: '',
        schema: '',
      }
  )

  const activeTabConnection = computed<DbConnectionInfo | undefined>(() =>
    connections.value.find((c) => c.id === activeTabContext.value.connectionId)
  )

  const queryState = computed<QueryState>(() => {
    queryStates.value[activeTabId.value] ??= makeQueryState()
    return queryStates.value[activeTabId.value]
  })

  /** 每连接「显示系统库」开关（默认隐藏；右键菜单切换） */
  const showSystemSchemas = ref<Record<string, boolean>>({})

  function toggleSystemSchemas(connId: string) {
    showSystemSchemas.value = {
      ...showSystemSchemas.value,
      [connId]: !showSystemSchemas.value[connId],
    }
  }

  /** 系统库过滤（仅隐藏开关关闭且命中系统清单时过滤） */
  function filterSystem(connId: string, dbType: string, names: string[]): string[] {
    if (showSystemSchemas.value[connId]) return names
    return names.filter((n) => !isSystemSchema(dbType, n))
  }

  const connectionOptions = computed(() =>
    connections.value.map((c) => ({ value: c.id, label: c.label }))
  )

  const databaseOptions = computed(() => {
    const connId = activeTabContext.value.connectionId
    const meta = metas.value[connId]
    if (!meta || meta.databases.length === 0) return []
    const conn = connections.value.find((c) => c.id === connId)
    const names = conn ? filterSystem(connId, conn.dbType, meta.databases) : meta.databases
    return names.map((name) => ({ value: name, label: name }))
  })

  const schemaOptions = computed(() => {
    const connId = activeTabContext.value.connectionId
    const meta = metas.value[connId]
    if (!meta || meta.schemas.length === 0) return []
    const conn = connections.value.find((c) => c.id === connId)
    const names = conn ? filterSystem(connId, conn.dbType, meta.schemas) : meta.schemas
    return names.map((name) => ({ value: name, label: name }))
  })

  /** SQL 编辑器补全元数据：当前连接已加载的表/视图（列暂不缓存，先补表名） */
  const completionTables = computed<{ name: string; columns: { name: string }[] }[]>(() => {
    const meta = metas.value[activeTabContext.value.connectionId]
    if (!meta) return []
    const names = new Set<string>()
    for (const objects of Object.values(meta.objects)) {
      for (const obj of objects) {
        if ((obj.kind === 'table' || obj.kind === 'view') && obj.name) names.add(obj.name)
      }
    }
    return [...names].map((name) => ({ name, columns: [] }))
  })

  /** 编辑器补全列缓存（表名 → 列名，跨 schema 以连接为界） */
  const editorColumnCache = new Map<string, string[]>()

  /** 编辑器「表.」后补全：查列（带缓存，命中直接返回） */
  async function resolveEditorColumns(table: string): Promise<string[]> {
    const connId = activeTabContext.value.connectionId
    const cacheKey = `${connId}::${table.toLowerCase()}`
    const cached = editorColumnCache.get(cacheKey)
    if (cached) return cached
    try {
      const cols = await queryIpc.columns(connId, table, activeTabContext.value.schema || undefined)
      const names = cols.map((c) => c.name)
      editorColumnCache.set(cacheKey, names)
      return names
    } catch {
      editorColumnCache.set(cacheKey, [])
      return []
    }
  }

  const rowLimitOptions = ['50', '100', '500', '1000'].map((v) => ({ value: v, label: v }))

  // ──────────────────────────────────────────────────────────────────────
  // 连接管理
  // ──────────────────────────────────────────────────────────────────────

  async function refreshConnections() {
    try {
      connections.value = await connectionIpc.list()
      if (!activeConnectionId.value && connections.value.length) {
        activeConnectionId.value = connections.value[0].id
      }
    } catch (err) {
      showError(err)
    }
  }

  async function connect(conn: DbConnectionInfo) {
    connecting.value[conn.id] = true
    connectError.value[conn.id] = ''
    cancelledConnect.value[conn.id] = false
    try {
      const info = await withTimeout(
        connectionIpc.connect(conn.id),
        30000,
        '连接超时（30 秒）：请检查网络与服务器配置'
      )
      // 连接过程中被取消：立即断开，避免留下幽灵会话
      if (cancelledConnect.value[conn.id]) {
        cancelledConnect.value[conn.id] = false
        try {
          await connectionIpc.disconnect(conn.id)
        } catch {
          // 忽略断开失败（会话可能尚未建立）
        }
        return
      }
      const index = connections.value.findIndex((c) => c.id === info.id)
      if (index >= 0) connections.value[index] = info
      activeConnectionId.value = info.id
      // 连接成功后展开树节点并预取库/schema 列表
      const expanded = new Set(expandedIds.value)
      expanded.add(info.id)
      expandedIds.value = expanded
      void ensureMeta(info.id)
    } catch (err) {
      const wasCancelled = cancelledConnect.value[conn.id]
      cancelledConnect.value[conn.id] = false
      if (wasCancelled) return
      connectError.value[conn.id] = String(err)
      const index = connections.value.findIndex((c) => c.id === conn.id)
      if (index >= 0) connections.value[index] = { ...conn, status: 'offline', error: String(err) }
      throw err
    } finally {
      connecting.value[conn.id] = false
    }
  }

  /** 取消进行中的连接（结果返回后自动断开，UI 立即复位） */
  function cancelConnect(connId: string) {
    cancelledConnect.value[connId] = true
    connecting.value[connId] = false
    connectError.value[connId] = ''
  }

  async function disconnect(conn: DbConnectionInfo) {
    try {
      await connectionIpc.disconnect(conn.id)
    } catch {
      // 断开失败不阻断状态更新
    }
    const index = connections.value.findIndex((c) => c.id === conn.id)
    if (index >= 0)
      connections.value[index] = {
        ...conn,
        status: 'offline',
        version: '',
        latencyMs: 0,
        connectedAt: 0,
      }
    delete metas.value[conn.id]
  }

  /** 保存连接配置（可选保存后立即连接） */
  async function saveConnection(config: ConnConfig, password: string, connectAfter = true) {
    await connectionIpc.save(config, password)
    await refreshConnections()
    if (connectAfter) {
      const conn = connections.value.find((c) => c.id === config.id)
      if (conn) await connect(conn)
    }
  }

  async function removeConnection(id: string) {
    if (connections.value.length <= 1) return
    try {
      await connectionIpc.disconnect(id)
    } catch {
      // 未连接时忽略
    }
    await connectionIpc.remove(id)
    // 关闭该连接下的页签
    tabs.value = tabs.value.filter((t) => tabContexts.value[t.id]?.connectionId !== id)
    Object.keys(tabContexts.value).forEach((tid) => {
      if (tabContexts.value[tid].connectionId === id) {
        delete tabContexts.value[tid]
        delete queryStates.value[tid]
      }
    })
    delete metas.value[id]
    await refreshConnections()
    if (!connections.value.some((c) => c.id === activeConnectionId.value)) {
      activeConnectionId.value = connections.value[0]?.id ?? ''
    }
  }

  // ──────────────────────────────────────────────────────────────────────
  // 元数据懒加载
  // ──────────────────────────────────────────────────────────────────────

  function metaFor(connId: string): ConnMeta {
    metas.value[connId] ??= {
      databases: [],
      schemas: [],
      objects: {},
      redisKeys: [],
      loading: false,
      loaded: false,
    }
    return metas.value[connId]
  }

  async function ensureMeta(connId: string) {
    const conn = connections.value.find((c) => c.id === connId)
    if (!conn) return
    const meta = metaFor(connId)
    if (meta.loaded || meta.loading) return
    meta.loading = true
    try {
      const [databases, schemas] = await Promise.all([
        queryIpc.databases(connId),
        queryIpc.schemas(connId),
      ])
      meta.databases = databases
      meta.schemas = schemas
      meta.loaded = true
    } catch (err) {
      showError(err)
    } finally {
      meta.loading = false
    }
  }

  /** 加载某 schema（或 mysql 的 database）下的对象列表 */
  async function ensureObjects(connId: string, schema: string) {
    const meta = metaFor(connId)
    const key = `${connId}::${schema}`
    if (meta.objects[key]) return
    try {
      meta.objects[key] = await queryIpc.objects(connId, schema)
    } catch (err) {
      meta.objects[key] = []
      showError(err)
    }
  }

  /** 加载 Redis 键列表 */
  async function ensureRedisKeys(connId: string) {
    const meta = metaFor(connId)
    if (meta.redisKeys.length) return
    try {
      const [, keys] = await queryIpc.redisKeys(connId, '', 0)
      meta.redisKeys = keys
    } catch (err) {
      showError(err)
    }
  }

  // ──────────────────────────────────────────────────────────────────────
  // 对象树（层级随数据库类型，与 mock 语义一致；叶子懒加载）
  // ──────────────────────────────────────────────────────────────────────

  function isExpanded(id: string): boolean {
    return expandedIds.value.has(id)
  }

  function toggleExpanded(id: string) {
    const next = new Set(expandedIds.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    expandedIds.value = next
  }

  function branch(
    id: string,
    label: string,
    depth: number,
    kind: string,
    badge?: string | number
  ): UiTreeItem {
    return { id, label, depth, kind, badge, expandable: true, expanded: isExpanded(id) }
  }

  function leaf(id: string, label: string, depth: number, kind: string): UiTreeItem {
    return { id, label, depth, kind, expandable: false, expanded: false }
  }

  /** 对象分组（每类型固定分组；子项来自懒加载缓存） */
  function objectGroups(conn: DbConnectionInfo, schemaKey: string, depth: number): UiTreeItem[] {
    const meta = metaFor(conn.id)
    const objects = meta.objects[schemaKey] ?? []
    const groups: { key: string; label: string; kinds: string[] }[] =
      conn.dbType === 'redis'
        ? [{ key: 'keys', label: '键', kinds: ['key'] }]
        : [
            { key: 'tables', label: '表', kinds: ['table'] },
            { key: 'views', label: '视图', kinds: ['view', 'materialized_view'] },
            { key: 'funcs', label: '函数', kinds: ['function'] },
            { key: 'seqs', label: '序列', kinds: ['sequence'] },
          ]
    const items: UiTreeItem[] = []
    for (const group of groups) {
      const members = objects.filter((o) => group.kinds.includes(o.kind))
      const groupId = `${schemaKey}::${group.key}`
      items.push(
        branch(groupId, group.label, depth, `group-${group.key}`, members.length || undefined)
      )
      for (const obj of members) {
        const itemId = `${schemaKey}::${obj.kind}:${obj.name}`
        items.push(leaf(itemId, obj.name, depth + 1, obj.kind))
      }
    }
    return items
  }

  const treeItems = computed<UiTreeItem[]>(() => {
    const items: UiTreeItem[] = []
    for (const conn of connections.value) {
      const prefix = conn.id
      // 徽标只放类型/只读；错误信息不进徽标（名字始终完整显示，错误走状态点 tooltip）
      const badge =
        conn.status === 'online'
          ? conn.readonly
            ? '只读'
            : (DB_TYPE_META[conn.dbType as V2DbType]?.label ?? conn.dbType)
          : undefined
      items.push({
        id: prefix,
        label: conn.label,
        depth: 0,
        kind: 'connection',
        // 未连接时无折叠箭头（双击连接后展开）；已连接可折叠/展开
        expandable: conn.status === 'online',
        expanded: isExpanded(prefix),
        badge,
        muted: conn.status !== 'online',
      })
      if (conn.status !== 'online') continue
      const meta = metaFor(conn.id)

      if (usesConnectionRootSchema(conn.dbType as V2DbType)) {
        // oracle/dameng：连接 → schema（用户）→ 分组（系统 schema 默认隐藏）
        const schemas = meta.schemas.length ? meta.schemas : [conn.database]
        for (const schema of filterSystem(conn.id, conn.dbType, schemas)) {
          const scope = `${prefix}::${schema}`
          items.push(branch(scope, schema, 1, 'schema'))
          items.push(...objectGroups(conn, scope, 2))
        }
        continue
      }

      if (conn.dbType === 'mysql' || conn.dbType === 'polardb') {
        // MySQL：连接 → 全部库 → 分组（系统库默认隐藏；未加载时回退配置库名不过滤）
        const raw = meta.databases.length ? meta.databases : [conn.database || '默认']
        const databases = meta.databases.length ? filterSystem(conn.id, conn.dbType, raw) : raw
        for (const db of databases) {
          const scope = `${prefix}::${db}`
          items.push(branch(scope, db, 1, 'database'))
          items.push(...objectGroups(conn, scope, 2))
        }
        continue
      }

      // 其余（sqlite）：连接 → 数据库 → 分组
      const database = conn.database
      items.push(branch(`${prefix}::db`, database, 1, 'database'))

      if (usesSchemaTree(conn.dbType as V2DbType)) {
        // PG 系：数据库 → schema → 分组（系统 schema 默认隐藏；未加载时回退 public 不过滤）
        const raw = meta.schemas.length ? meta.schemas : ['public']
        const schemas = meta.schemas.length ? filterSystem(conn.id, conn.dbType, raw) : raw
        for (const schema of schemas) {
          const scope = `${prefix}::${schema}`
          items.push(branch(scope, schema, 2, 'schema'))
          items.push(...objectGroups(conn, scope, 3))
        }
        continue
      }

      if (conn.dbType === 'redis') {
        items.push(...objectGroups(conn, `${prefix}::redis`, 2))
        continue
      }

      // sqlite：数据库下直接挂分组
      items.push(...objectGroups(conn, `${prefix}::objects`, 2))
    }
    return items
  })

  /** 树过滤（关键字命中则保留祖先链） */
  const visibleTreeItems = computed<UiTreeItem[]>(() => {
    const filter = keyword.value.trim().toLowerCase()
    return filterTreeItems(treeItems.value, filter)
  })

  // ──────────────────────────────────────────────────────────────────────
  // 树交互
  // ──────────────────────────────────────────────────────────────────────

  function toggleTree(item: UiTreeItem) {
    // 连接节点：已连接 → 折叠/展开（不重新连接）；未连接 → 连接并在成功后展开
    const conn = connections.value.find((c) => c.id === item.id)
    if (conn) {
      if (conn.status === 'online') {
        const willExpand = !isExpanded(item.id)
        toggleExpanded(item.id)
        if (willExpand) void ensureMeta(conn.id)
      } else if (conn.dbType === 'dameng') {
        connectError.value[conn.id] = '达梦驱动暂未支持（本版本未实现）'
      } else {
        void connect(conn).catch(() => {})
      }
      return
    }
    toggleExpanded(item.id)
    // 展开 schema 节点 → 加载对象
    if (item.kind === 'schema' && item.expandable && !item.expanded) {
      const connId = item.id.split('::')[0]
      const schema = item.id.split('::')[1]
      void ensureObjects(connId, schema)
    }
    // 展开 database 节点（mysql 库 / sqlite main）→ 加载该库对象
    if (item.kind === 'database' && item.expandable && !item.expanded) {
      const connId = item.id.split('::')[0]
      const dbMatch = item.id.match(/::db:(.+)$/)
      const schema = dbMatch ? dbMatch[1] : (item.id.split('::')[1] ?? 'main')
      void ensureObjects(connId, schema)
    }
    // 展开 redis 数据库节点 → 加载键
    if (item.kind === 'database' && item.expandable && !item.expanded) {
      const redisConn = connections.value.find((c) => c.id === item.id.split('::')[0])
      if (redisConn?.dbType === 'redis') void ensureRedisKeys(redisConn.id)
    }
  }

  /** 树选中/右键「查看数据」入口：打开数据页签并按叶子 scope 设置库/schema 上下文 */
  async function selectResource(id: string) {
    selectedResource.value = id
    const connId = id.split('::')[0]
    const conn = connections.value.find((c) => c.id === connId)
    if (conn) activeConnectionId.value = connId
    const leaf = parseLeafId(id)
    if (
      leaf &&
      conn &&
      ['table', 'view', 'materialized_view', 'sequence', 'function'].includes(leaf.kind)
    ) {
      // 数据页签必须落在叶子所在的库/schema，不能用连接默认库（MySQL 默认库可空）
      const { database, schema } = scopeContext(conn, leaf.scope)
      const tabId = `data-${connId}-${leaf.name}`
      openOrFocusTab(tabId, `${leaf.name} · 数据`, 'data', connId, leaf.name)
      tabContexts.value[tabId].database = database
      tabContexts.value[tabId].schema = schema
      void loadTableData(tabId)
      return
    }
    if (leaf?.kind === 'key') {
      const tabId = `redis-${connId}-${leaf.name}`
      openOrFocusTab(tabId, `${leaf.name} · 键`, 'redis', connId, leaf.name)
      void loadRedisKeyInfo(tabId, leaf.name)
    }
  }

  // ──────────────────────────────────────────────────────────────────────
  // 页签
  // ──────────────────────────────────────────────────────────────────────

  function openOrFocusTab(
    id: string,
    label: string,
    kind: V2TabKind,
    connectionId: string,
    table?: string
  ) {
    if (!tabs.value.some((t) => t.id === id)) {
      tabs.value.push({ id, label, kind })
    }
    tabContexts.value[id] ??= {
      connectionId,
      database: activeConnection.value?.database ?? '',
      schema: '',
      table,
    }
    activeTabId.value = id
  }

  /** 打开新的 SQL 编辑器（页签名「SQL编辑器 N」；可指定连接） */
  function openSqlEditor(connectionId?: string) {
    tabSequence += 1
    const id = `q${tabSequence}`
    tabs.value.push({ id, label: `SQL编辑器 ${tabSequence}`, kind: 'query' })
    queryStates.value[id] = makeQueryState()
    tabContexts.value[id] = {
      connectionId: connectionId ?? activeConnectionId.value,
      database: activeConnection.value?.database ?? '',
      schema: '',
    }
    activeTabId.value = id
  }

  /** 打开带预置 SQL 的新编辑器（建表模板等），并指定库/schema 上下文 */
  function openSqlEditorWithSql(
    connectionId: string,
    sql: string,
    database?: string,
    schema?: string
  ) {
    openSqlEditor(connectionId)
    const id = activeTabId.value
    const ctx = tabContexts.value[id]
    if (database !== undefined) ctx.database = database
    if (schema !== undefined) ctx.schema = schema
    queryStates.value[id].sql = sql
  }

  /** 树节点 scope（库名/schema 名）→ 页签上下文（database/schema 字段映射随类型） */
  function scopeContext(
    conn: DbConnectionInfo,
    scope: string
  ): { database: string; schema: string } {
    const type = conn.dbType as V2DbType
    if (usesConnectionRootSchema(type)) return { database: conn.database, schema: scope }
    if (type === 'mysql' || type === 'polardb') return { database: scope, schema: '' }
    if (usesSchemaTree(type)) return { database: conn.database, schema: scope }
    return { database: conn.database || 'main', schema: '' }
  }

  /** 解析树叶子节点 id（`<conn>::<scope>::<kind>:<name>`） */
  function parseLeafId(
    id: string
  ): { connId: string; scope: string; kind: string; name: string } | null {
    const m = id.match(/^(.+?)::(.*)::([a-z_]+):(.+)$/)
    if (!m) return null
    return { connId: m[1], scope: m[2], kind: m[3], name: m[4] }
  }

  /** 打开表/视图结构页签（树右键、Ctrl+点击表名共用入口） */
  function openStructureTab(
    connectionId: string,
    table: string,
    database?: string,
    schema?: string
  ) {
    const id = `structure-${connectionId}-${table}`
    openOrFocusTab(id, `${table} · 结构`, 'structure', connectionId, table)
    if (database !== undefined) tabContexts.value[id].database = database
    if (schema !== undefined) tabContexts.value[id].schema = schema
    void loadColumns(id)
  }

  /** Ctrl+点击编辑器内表名：按当前页签上下文打开表结构 */
  function openStructureForTable(table: string) {
    const ctx = activeTabContext.value
    if (!ctx.connectionId) return
    openStructureTab(ctx.connectionId, table, ctx.database, ctx.schema)
  }

  /** 建表模板 SQL（按类型给最小骨架，在编辑器中由用户补全后执行） */
  function createTableTemplate(conn: DbConnectionInfo, database: string, schema: string): string {
    const type = conn.dbType as V2DbType
    if (type === 'mysql' || type === 'polardb') {
      const db = database ? `\`${database}\`.` : ''
      return `CREATE TABLE ${db}\`new_table\` (\n  \`id\` BIGINT NOT NULL AUTO_INCREMENT COMMENT '主键',\n  \`name\` VARCHAR(255) NOT NULL DEFAULT '',\n  \`created_at\` DATETIME DEFAULT CURRENT_TIMESTAMP,\n  PRIMARY KEY (\`id\`)\n);`
    }
    if (type === 'sqlite') {
      return `CREATE TABLE "new_table" (\n  "id" INTEGER PRIMARY KEY AUTOINCREMENT,\n  "name" TEXT NOT NULL\n);`
    }
    const schemaPrefix = schema ? `"${schema}".` : ''
    return `CREATE TABLE ${schemaPrefix}"new_table" (\n  "id" BIGINT PRIMARY KEY,\n  "name" VARCHAR(255) NOT NULL\n);`
  }

  /** 在指定库/schema 下新建表：打开带模板的 SQL 编辑器（用户确认后自行执行） */
  function openCreateTableEditor(connId: string, scope: string) {
    const conn = connections.value.find((c) => c.id === connId)
    if (!conn) return
    const { database, schema } = scopeContext(conn, scope)
    openSqlEditorWithSql(connId, createTableTemplate(conn, database, schema), database, schema)
  }

  /** 新建数据库（mysql/polardb/PG 系）：执行 CREATE DATABASE 后刷新元数据 */
  async function createDatabase(connId: string, name: string): Promise<boolean> {
    const conn = connections.value.find((c) => c.id === connId)
    if (!conn) return false
    const trimmed = name.trim()
    if (!/^[A-Za-z_][\w$]{0,63}$/.test(trimmed)) {
      showError('库名仅支持字母、数字、下划线与 $，且需以字母或下划线开头')
      return false
    }
    const quoted =
      conn.dbType === 'mysql' || conn.dbType === 'polardb' ? `\`${trimmed}\`` : `"${trimmed}"`
    try {
      const result = await queryIpc.execute(connId, `CREATE DATABASE ${quoted}`, 1)
      if (!result.ok) throw new Error(result.error ?? '创建数据库失败')
      // 库列表失效后重取（对象缓存一并清掉）
      const meta = metaFor(connId)
      meta.loaded = false
      meta.objects = {}
      await ensureMeta(connId)
      return true
    } catch (err) {
      showError(err)
      return false
    }
  }

  /** 刷新树节点：连接=库/schema 列表+对象缓存全清；库/schema/分组=清该 scope 对象缓存重取 */
  async function refreshTreeNode(item: UiTreeItem) {
    const connId = item.id.split('::')[0]
    const meta = metaFor(connId)
    if (item.kind === 'connection') {
      meta.loaded = false
      meta.objects = {}
      meta.redisKeys = []
      await ensureMeta(connId)
      return
    }
    const scope = item.id.split('::')[1]
    if (!scope) return
    if (scope === 'redis') {
      meta.redisKeys = []
      await ensureRedisKeys(connId)
      return
    }
    delete meta.objects[`${connId}::${scope}`]
    await ensureObjects(connId, scope)
  }

  /** 重命名当前页签别名（仅本地；已保存的编辑器下次保存时同步到库） */
  function renameActiveTab(label: string) {
    const tabId = activeTabId.value
    const tab = tabs.value.find((t) => t.id === tabId)
    if (!tab || !label.trim()) return
    tab.label = label.trim()
    const state = queryStates.value[tabId]
    if (state) state.savedTitle = label.trim()
    // 已保存的编辑器：别名立即同步到库
    if (state?.savedId && state.savedTitle) {
      void savedIpc.update(state.savedId, state.savedTitle, state.sql).catch(() => {})
    }
  }

  /**
   * 持久化保存当前 SQL 编辑器：
   * - 已保存过 → 直接 update（无弹窗）
   * - 未保存过 → 需先经 UI 弹窗确认别名（组件层调用本函数时传入）
   * 首次保存后页签更名为别名
   */
  async function saveQueryToDisk(title?: string) {
    const tabId = activeTabId.value
    const state = (queryStates.value[tabId] ??= makeQueryState())
    const sql = state.sql.trim()
    if (!sql) {
      showError('没有可保存的 SQL 内容')
      return
    }
    if (state.savedId) {
      await savedIpc.update(state.savedId, state.savedTitle ?? tabLabel(tabId), sql)
    } else {
      const resolved = (title ?? tabLabel(tabId)).trim() || `SQL编辑器 ${tabSequence}`
      state.savedId = await savedIpc.add(resolved, sql)
      state.savedTitle = resolved
      const tab = tabs.value.find((t) => t.id === tabId)
      if (tab) tab.label = resolved
    }
    state.dirty = false
    await refreshSaved()
  }

  /** 页签显示名 */
  function tabLabel(tabId: string): string {
    return tabs.value.find((t) => t.id === tabId)?.label ?? 'SQL编辑器'
  }

  function closeTab(id: string) {
    tabs.value = tabs.value.filter((t) => t.id !== id)
    delete tabContexts.value[id]
    delete queryStates.value[id]
    delete structureColumns.value[id]
    if (activeTabId.value === id) activeTabId.value = tabs.value[0]?.id ?? ''
  }

  // ──────────────────────────────────────────────────────────────────────
  // 查询执行
  // ──────────────────────────────────────────────────────────────────────

  function patchQueryState(patch: Partial<QueryState>) {
    const id = activeTabId.value
    queryStates.value[id] ??= makeQueryState()
    Object.assign(queryStates.value[id], patch)
  }

  /**
   * 执行 SQL：传入 sqlOverride 则只执行该段（组件层按"选中文本 / 光标所在行"提取）；
   * 不提供"全部执行"（避免误操作，需要全量先全选）。
   */
  async function runQuery(sqlOverride?: string) {
    const tabId = activeTabId.value
    const state = (queryStates.value[tabId] ??= makeQueryState())
    if (state.status === 'running') return
    const conn = activeTabConnection.value
    if (!conn || conn.status !== 'online') {
      patchQueryState({
        status: 'error',
        error: '当前连接已断开，无法执行查询。请先连接。',
        resultTab: 'message',
      })
      return
    }
    const sql = (sqlOverride ?? state.sql).trim()
    if (!sql) {
      patchQueryState({
        status: 'idle',
        error: '没有可执行的 SQL：请选中一段文本，或将光标置于某一行的任意位置。',
        resultTab: 'message',
      })
      return
    }
    const startedAt = Date.now()
    patchQueryState({ status: 'running', error: '', page: 1, columns: [], rows: [], total: 0 })
    try {
      const result = await queryIpc.execute(conn.id, sql, 1000)
      const durationMs = Date.now() - startedAt
      if (result.ok) {
        patchQueryState({
          status: result.isQuery && result.rows.length === 0 ? 'empty' : 'success',
          resultTab: 'data',
          durationMs,
          affected: result.rowsAffected,
          columns: result.columns,
          rows: result.rows,
          total: result.rowsAffected,
          truncated: result.truncated,
          filter: '',
        })
      } else {
        patchQueryState({
          status: 'error',
          error: result.error ?? '查询失败',
          resultTab: 'message',
          durationMs,
        })
      }
      void historyIpc.add(conn.id, sql, result.ok ? 'success' : 'error', durationMs).catch(() => {})
      void refreshHistory()
    } catch (err) {
      patchQueryState({
        status: 'error',
        error: String(err),
        resultTab: 'message',
        durationMs: Date.now() - startedAt,
      })
      void historyIpc.add(conn.id, sql, 'error', Date.now() - startedAt).catch(() => {})
      void refreshHistory()
    }
  }

  async function cancelQuery() {
    const conn = activeTabConnection.value
    if (!conn) return
    try {
      await queryIpc.cancel(conn.id)
      patchQueryState({ status: 'cancelled', resultTab: 'message' })
    } catch (err) {
      showError(err)
    }
  }

  function onFormatSql() {
    const sql = queryState.value.sql
    patchQueryState({ sql: formatSql(sql), dirty: true })
  }

  // ──────────────────────────────────────────────────────────────────────
  // 数据浏览 / 结构 / Redis 键
  // ──────────────────────────────────────────────────────────────────────

  /** 页签上下文 → 后端 schema 入参（mysql/polardb 传库名；PG 系传 schema；sqlite/redis 不用） */
  function ipcScopeArg(conn: DbConnectionInfo, ctx: TabContext): string | undefined {
    if (
      conn.dbType === 'postgresql' ||
      conn.dbType === 'kingbase' ||
      conn.dbType === 'vastbase'
    ) {
      return ctx.schema || 'public'
    }
    if (conn.dbType === 'oracle' || conn.dbType === 'dameng') return ctx.schema || undefined
    return ctx.database || conn.database || undefined
  }

  /** 数据页签：从页签上下文取库/schema 与表名，后端分页 */
  async function loadTableData(tabId: string) {
    const ctx = tabContexts.value[tabId]
    const state = (queryStates.value[tabId] ??= makeQueryState())
    if (!ctx) return
    const table = ctx.table ?? tabTableName(tabId)
    if (!table) return
    const conn = connections.value.find((c) => c.id === ctx.connectionId)
    if (!conn) return
    state.status = 'running'
    try {
      const page = await queryIpc.tableData(
        ctx.connectionId,
        table,
        state.page,
        PAGE_SIZE,
        ipcScopeArg(conn, ctx)
      )
      state.columns = page.columns
      state.rows = page.rows
      state.total = page.total
      state.truncated = false
      state.status = page.error ? 'error' : 'success'
      state.error = page.error ?? ''
      state.resultTab = 'data'
    } catch (err) {
      state.status = 'error'
      state.error = String(err)
      state.resultTab = 'message'
    }
  }

  function setPage(nextPage: number) {
    const state = queryState.value
    state.page = Math.max(1, nextPage)
    if (activeTabKind.value === 'data') {
      void loadTableData(activeTabId.value)
    } else if (activeTabKind.value === 'query') {
      // 查询结果分页：前端按 total 切片（后端已取回一页）
    }
  }

  /** 结构页签：加载列信息（schema 入参与数据页签同规则，否则 MySQL 非默认库查不到列） */
  async function loadColumns(tabId: string) {
    const ctx = tabContexts.value[tabId]
    if (!ctx) return
    const table = ctx.table ?? tabTableName(tabId)
    if (!table) return
    const conn = connections.value.find((c) => c.id === ctx.connectionId)
    if (!conn) return
    try {
      structureColumns.value[tabId] = await queryIpc.columns(
        ctx.connectionId,
        table,
        ipcScopeArg(conn, ctx)
      )
    } catch (err) {
      showError(err)
    }
  }

  /** Redis 键页签：加载键信息 */
  async function loadRedisKeyInfo(tabId: string, key: string) {
    const ctx = tabContexts.value[tabId]
    if (!ctx) return
    const state = (queryStates.value[tabId] ??= makeQueryState())
    try {
      const info = await queryIpc.redisKeyInfo(ctx.connectionId, key)
      state.columns = ['键', '类型', 'TTL', '值']
      state.rows = [[info.key, info.kind, info.ttl === -1 ? '永久' : `${info.ttl}s`, info.value]]
      state.status = 'success'
      state.resultTab = 'data'
    } catch (err) {
      state.status = 'error'
      state.error = String(err)
      state.resultTab = 'message'
    }
  }

  /** 从页签 id 提取表名（data-<conn>-<table> / structure-<conn>-<table>） */
  function tabTableName(tabId: string): string {
    const m = tabId.match(/^data-(?:[^-]+)-(.*)$/) ?? tabId.match(/^structure-(?:[^-]+)-(.*)$/)
    return m?.[1] ?? ''
  }

  // ──────────────────────────────────────────────────────────────────────
  // 历史 / 收藏
  // ──────────────────────────────────────────────────────────────────────

  async function refreshHistory() {
    try {
      history.value = await historyIpc.list()
    } catch (err) {
      showError(err)
    }
  }

  async function refreshSaved() {
    try {
      savedSql.value = await savedIpc.list()
    } catch (err) {
      showError(err)
    }
  }

  function applyHistory(entry: HistoryEntry) {
    if (activeTabKind.value !== 'query' || queryState.value.sql.trim()) openSqlEditor()
    patchQueryState({ sql: entry.sql, dirty: true })
  }

  /** 打开已保存的 SQL 编辑器：同一收藏只保留一个页签（已打开则聚焦）；记住 savedId（Ctrl+S 直接更新） */
  function applySaved(entry: SavedEntry) {
    const existing = tabs.value.find(
      (t) => t.kind === 'query' && queryStates.value[t.id]?.savedId === entry.id
    )
    if (existing) {
      activeTabId.value = existing.id
      return
    }
    if (activeTabKind.value !== 'query' || queryState.value.sql.trim()) openSqlEditor()
    const tabId = activeTabId.value
    const state = (queryStates.value[tabId] ??= makeQueryState())
    Object.assign(state, {
      sql: entry.sql,
      dirty: false,
      savedId: entry.id,
      savedTitle: entry.title,
    })
    const tab = tabs.value.find((t) => t.id === tabId)
    if (tab) tab.label = entry.title
  }

  async function removeSaved(id: number) {
    await savedIpc.remove(id)
    await refreshSaved()
  }

  async function clearHistory() {
    await historyIpc.clear()
    await refreshHistory()
  }

  // ──────────────────────────────────────────────────────────────────────
  // 结果派生
  // ──────────────────────────────────────────────────────────────────────

  const filteredRows = computed(() => {
    const state = queryState.value
    const filter = state.filter.trim().toLowerCase()
    if (!filter) return state.rows
    return state.rows.filter((row) => row.some((cell) => cell.toLowerCase().includes(filter)))
  })

  const totalPages = computed(() => {
    const state = queryState.value
    if (activeTabKind.value === 'data') return Math.max(1, Math.ceil(state.total / PAGE_SIZE))
    return Math.max(1, Math.ceil(filteredRows.value.length / PAGE_SIZE))
  })

  const pageRows = computed(() => {
    const state = queryState.value
    if (activeTabKind.value === 'data') return state.rows
    const start = (state.page - 1) * PAGE_SIZE
    return filteredRows.value.slice(start, start + PAGE_SIZE)
  })

  /** 动态列 → UiDataGridColumn（宽度按内容类型估算） */
  const tableColumns = computed<UiDataGridColumn[]>(() =>
    queryState.value.columns.map((name, index) => ({
      key: `c${index}`,
      label: name,
      width: Math.min(Math.max(name.length * 10 + 32, 72), 240),
      content: 'technical',
    }))
  )

  const resultTabs = computed<UiTabItem[]>(() => {
    const state = queryState.value
    return [
      {
        value: 'data',
        label: '数据',
        badge: state.status === 'success' ? state.total : undefined,
      },
      { value: 'message', label: '消息' },
    ]
  })

  // ──────────────────────────────────────────────────────────────────────
  // 初始化 / 清理
  // ──────────────────────────────────────────────────────────────────────

  const errorHint = ref('')
  let errorTimer: ReturnType<typeof setTimeout> | undefined
  function showError(err: unknown) {
    errorHint.value = err instanceof Error ? err.message : String(err)
    if (errorTimer) clearTimeout(errorTimer)
    errorTimer = setTimeout(() => (errorHint.value = ''), 4000)
  }

  onBeforeUnmount(() => {
    if (errorTimer) clearTimeout(errorTimer)
  })

  return {
    // 连接
    connections,
    activeConnection,
    activeConnectionId,
    connecting,
    connectError,
    connectionOptions,
    connect,
    cancelConnect,
    disconnect,
    saveConnection,
    removeConnection,
    refreshConnections,
    // 元数据
    databaseOptions,
    schemaOptions,
    rowLimitOptions,
    completionTables,
    resolveEditorColumns,
    ensureMeta,
    showSystemSchemas,
    toggleSystemSchemas,
    // 页签
    tabs,
    activeTab,
    activeTabId,
    activeTabKind,
    activeTabContext,
    activeTabConnection,
    tabContexts,
    openSqlEditor,
    openSqlEditorWithSql,
    openStructureTab,
    openStructureForTable,
    openCreateTableEditor,
    createDatabase,
    refreshTreeNode,
    scopeContext,
    parseLeafId,
    closeTab,
    renameActiveTab,
    saveQueryToDisk,
    // 查询
    queryState,
    queryStates,
    patchQueryState,
    runQuery,
    cancelQuery,
    onFormatSql,
    // 树
    treeItems,
    visibleTreeItems,
    keyword,
    selectedResource,
    selectResource,
    toggleTree,
    // 结果
    pageRows,
    filteredRows,
    totalPages,
    setPage,
    resultTabs,
    tableColumns,
    structureColumns,
    loadColumns,
    loadTableData,
    // 历史/收藏
    history,
    savedSql,
    refreshHistory,
    refreshSaved,
    applyHistory,
    applySaved,
    removeSaved,
    clearHistory,
    // 通用
    errorHint,
    showError,
  }
}
