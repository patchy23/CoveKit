/**
 * database 对象树与目录域：每连接的库/schema/对象懒加载缓存与加载状态、
 * 树显示状态（关键字、选中、展开、系统库开关）、SQL 编辑器补全元数据，
 * 以及挂在树上的 DDL/管理命令（显式 command，经执行端口请求并失效相关缓存）。
 *
 * 缓存 key 一律带连接 id（`conn::scope`、`conn::table`），不会读到另一个连接的树；
 * 跨域只拿 connection 域只读快照/窄命令、workspace 域只读 computed 视图与页签命令，
 * 不持有其它域的可写状态。
 */
import { computed, ref } from 'vue'
import type { ComputedRef } from 'vue'
import type { UiTreeItem } from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import {
  DB_TYPE_META,
  type V2DbType,
  isSystemSchema,
  usesConnectionRootSchema,
  usesSchemaTree,
} from '../useDatabaseMeta'
import type { DbConnectionInfo, DbGrantInput, DbObjectInfo, DbStepResult } from '../contracts'
import { adminIpc, queryIpc } from '../ipc'
import { nextRequestId } from '../requestId'
import type { TabContext } from '../workspace/useQueryWorkspace'

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

/** 每连接元数据（懒加载缓存；key 带连接 id，跨连接不串读） */
interface ConnMeta {
  databases: string[]
  schemas: string[]
  /** schemaKey（mysql 用 database；pg 用 schema）→ 对象列表 */
  objects: Record<string, DbObjectInfo[]>
  redisKeys: string[]
  loading: boolean
  loaded: boolean
}

/** catalog 域的跨域协作端口（由根门面注入） */
export interface DatabaseCatalogPorts {
  /** 连接只读快照（connection 域） */
  connections: ComputedRef<readonly DbConnectionInfo[]>
  /** 当前页签所属连接 id 只读视图（workspace 域；库/schema 下拉与补全跟随当前页签） */
  activeTabConnectionId: ComputedRef<string>
  /** 当前页签 schema 只读视图（workspace 域） */
  activeTabSchema: ComputedRef<string>
  /** 选中连接（connection 域；连接不存在时不动） */
  activateConnection: (connId: string) => void
  /** 连接命令（connection 域；树双击未连接节点即连接） */
  connect: (conn: DbConnectionInfo) => Promise<void>
  /** 记录连接失败原因（connection 域状态） */
  setConnectError: (connId: string, message: string) => void
  /** 页签上下文 → 后端 schema 入参（workspace 域纯函数） */
  ipcScopeArg: (conn: DbConnectionInfo, ctx: TabContext) => string | undefined
  /** 打开数据页签（workspace 域命令） */
  openDataTab: (connId: string, table: string, database: string, schema: string) => void
  /** 打开 Redis 键页签（workspace 域命令） */
  openRedisKeyTab: (connId: string, key: string) => void
  /** 打开带预置 SQL 的编辑器（workspace 域命令） */
  openSqlEditorWithSql: (
    connectionId: string,
    sql: string,
    database?: string,
    schema?: string
  ) => void
  /** 打开可视化建表页签（workspace 域命令；页签名由本域决定） */
  openCreateTableTab: (
    connectionId: string,
    label: string,
    database: string,
    schema: string
  ) => void
  /** 页签是否存在（workspace 域只读视图） */
  hasTab: (tabId: string) => boolean
  /** 关闭页签（workspace 域命令；表删除/重命名后关闭旧表页签） */
  closeTab: (tabId: string) => void
  /** 应用级错误提示（门面持有 errorHint） */
  showError: (err: unknown) => void
}

export function useDatabaseCatalog(ports: DatabaseCatalogPorts) {
  // ── 元数据缓存 ──────────────────────────────────────────────────────────
  const metas = ref<Record<string, ConnMeta>>({})

  // ── 树显示状态 ──────────────────────────────────────────────────────────
  const keyword = ref('')
  const selectedResource = ref('')
  const expandedIds = ref<Set<string>>(new Set())
  /** 每连接「显示系统库」开关（默认隐藏；右键菜单切换） */
  const showSystemSchemas = ref<Record<string, boolean>>({})

  // ──────────────────────────────────────────────────────────────────────
  // 系统库过滤 / 展开状态
  // ──────────────────────────────────────────────────────────────────────

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

  function isExpanded(id: string): boolean {
    return expandedIds.value.has(id)
  }

  function toggleExpanded(id: string) {
    const next = new Set(expandedIds.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    expandedIds.value = next
  }

  /** 连接成功（connection 域端口）：展开该连接节点并预取库/schema 列表 */
  function prefetchConnection(connId: string) {
    const expanded = new Set(expandedIds.value)
    expanded.add(connId)
    expandedIds.value = expanded
    void ensureMeta(connId)
  }

  /** 连接断开/删除（connection 域端口）：失效该连接的元数据缓存 */
  function invalidateConnectionMeta(connId: string) {
    delete metas.value[connId]
  }

  // ──────────────────────────────────────────────────────────────────────
  // 目录派生视图（库/schema 下拉、补全元数据）
  // ──────────────────────────────────────────────────────────────────────

  const databaseOptions = computed(() => {
    const connId = ports.activeTabConnectionId.value
    const meta = metas.value[connId]
    if (!meta || meta.databases.length === 0) return []
    const conn = ports.connections.value.find((c) => c.id === connId)
    const names = conn ? filterSystem(connId, conn.dbType, meta.databases) : meta.databases
    return names.map((name) => ({ value: name, label: name }))
  })

  const schemaOptions = computed(() => {
    const connId = ports.activeTabConnectionId.value
    const meta = metas.value[connId]
    if (!meta || meta.schemas.length === 0) return []
    const conn = ports.connections.value.find((c) => c.id === connId)
    const names = conn ? filterSystem(connId, conn.dbType, meta.schemas) : meta.schemas
    return names.map((name) => ({ value: name, label: name }))
  })

  /** SQL 编辑器补全元数据：当前连接已加载的表/视图（列暂不缓存，先补表名） */
  const completionTables = computed<{ name: string; columns: { name: string }[] }[]>(() => {
    const meta = metas.value[ports.activeTabConnectionId.value]
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
    const connId = ports.activeTabConnectionId.value
    const cacheKey = `${connId}::${table.toLowerCase()}`
    const cached = editorColumnCache.get(cacheKey)
    if (cached) return cached
    try {
      const cols = await queryIpc.columns(connId, table, ports.activeTabSchema.value || undefined)
      const names = cols.map((c) => c.name)
      editorColumnCache.set(cacheKey, names)
      return names
    } catch {
      editorColumnCache.set(cacheKey, [])
      return []
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
    const conn = ports.connections.value.find((c) => c.id === connId)
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
      ports.showError(err)
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
      ports.showError(err)
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
      ports.showError(err)
    }
  }

  // ──────────────────────────────────────────────────────────────────────
  // 对象树（层级随数据库类型，与 mock 语义一致；叶子懒加载）
  // ──────────────────────────────────────────────────────────────────────

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
    for (const conn of ports.connections.value) {
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
    const conn = ports.connections.value.find((c) => c.id === item.id)
    if (conn) {
      if (conn.status === 'online') {
        const willExpand = !isExpanded(item.id)
        toggleExpanded(item.id)
        if (willExpand) void ensureMeta(conn.id)
      } else if (conn.dbType === 'dameng') {
        ports.setConnectError(conn.id, '达梦驱动暂未支持（本版本未实现）')
      } else {
        void ports.connect(conn).catch(() => {})
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
      const redisConn = ports.connections.value.find((c) => c.id === item.id.split('::')[0])
      if (redisConn?.dbType === 'redis') void ensureRedisKeys(redisConn.id)
    }
  }

  /** 解析树叶子节点 id（`<conn>::<scope>::<kind>:<name>`） */
  function parseLeafId(
    id: string
  ): { connId: string; scope: string; kind: string; name: string } | null {
    const m = id.match(/^(.+?)::(.*)::([a-z_]+):(.+)$/)
    if (!m) return null
    return { connId: m[1], scope: m[2], kind: m[3], name: m[4] }
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

  /** 树选中/右键「查看数据」入口：打开数据页签并按叶子 scope 设置库/schema 上下文 */
  async function selectResource(id: string) {
    selectedResource.value = id
    const connId = id.split('::')[0]
    const conn = ports.connections.value.find((c) => c.id === connId)
    if (conn) ports.activateConnection(connId)
    const leaf = parseLeafId(id)
    if (
      leaf &&
      conn &&
      ['table', 'view', 'materialized_view', 'sequence', 'function'].includes(leaf.kind)
    ) {
      // 数据页签必须落在叶子所在的库/schema，不能用连接默认库（MySQL 默认库可空）
      const { database, schema } = scopeContext(conn, leaf.scope)
      ports.openDataTab(connId, leaf.name, database, schema)
      return
    }
    if (leaf?.kind === 'key') {
      ports.openRedisKeyTab(connId, leaf.name)
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

  // ──────────────────────────────────────────────────────────────────────
  // 树上的建表 / 建库 / 维护命令
  // ──────────────────────────────────────────────────────────────────────

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

  /** 在指定库/schema 下新建表：打开带模板的 SQL 编辑器（非 mysql 系类型的回退路径） */
  function openCreateTableEditor(connId: string, scope: string) {
    const conn = ports.connections.value.find((c) => c.id === connId)
    if (!conn) return
    const { database, schema } = scopeContext(conn, scope)
    ports.openSqlEditorWithSql(
      connId,
      createTableTemplate(conn, database, schema),
      database,
      schema
    )
  }

  /** 可视化建表页签（mysql 系）：CreateTableTab 按 ctx 渲染列编辑器 */
  function openCreateTableTab(connId: string, scope: string) {
    const conn = ports.connections.value.find((c) => c.id === connId)
    if (!conn) return
    const { database, schema } = scopeContext(conn, scope)
    ports.openCreateTableTab(
      connId,
      `新建表 · ${scope || conn.database || '未选库'}`,
      database,
      schema
    )
  }

  /** 新建数据库（字符集/排序规则/授权可选）：分步执行，逐步结果由调用方展示 */
  async function createDatabaseFull(
    connId: string,
    name: string,
    charset?: string,
    collation?: string,
    grants?: DbGrantInput[]
  ): Promise<DbStepResult[] | null> {
    const trimmed = name.trim()
    if (!/^[A-Za-z_][\w$]{0,63}$/.test(trimmed)) {
      ports.showError('库名仅支持字母、数字、下划线与 $，且需以字母或下划线开头')
      return null
    }
    try {
      const steps = await adminIpc.createDatabase(connId, trimmed, charset, collation, grants)
      if (steps.every((s) => s.ok)) {
        // 库列表失效后重取（对象缓存一并清掉）
        const meta = metaFor(connId)
        meta.loaded = false
        meta.objects = {}
        await ensureMeta(connId)
      }
      return steps
    } catch (err) {
      ports.showError(err)
      return null
    }
  }

  /** 删除数据库（前端确认后调用）：执行后刷新元数据 */
  async function dropDatabase(connId: string, name: string): Promise<boolean> {
    try {
      const sql = await adminIpc.dropDatabase(connId, name)
      useUiStore().toast(`已删除数据库 ${name}`)
      void sql
      const meta = metaFor(connId)
      meta.loaded = false
      meta.objects = {}
      await ensureMeta(connId)
      return true
    } catch (err) {
      ports.showError(err)
      return false
    }
  }

  /** 执行 DDL（可视化建表等）：成功 toast + 刷新指定 scope 对象缓存 */
  async function executeDdl(connId: string, sql: string, scope?: string): Promise<boolean> {
    try {
      const result = await queryIpc.execute(connId, sql, 1, nextRequestId('ddl'))
      if (!result.ok) throw new Error(result.error ?? '执行失败')
      useUiStore().toast('执行成功')
      if (scope) {
        await refreshTreeNode({
          id: `${connId}::${scope}`,
          kind: 'database',
          label: scope,
          depth: 1,
          expandable: true,
          expanded: true,
        })
      }
      return true
    } catch (err) {
      ports.showError(err)
      return false
    }
  }

  /** 表维护（重命名/清空/删除）：执行后刷新该 scope 对象缓存，并关闭受影响页签 */
  async function tableAdminAction(
    connId: string,
    scope: string,
    table: string,
    action: 'rename' | 'truncate' | 'drop',
    newName?: string,
    kind?: string
  ): Promise<boolean> {
    const conn = ports.connections.value.find((c) => c.id === connId)
    if (!conn) return false
    const { database, schema } = scopeContext(conn, scope)
    try {
      const sql = await adminIpc.tableAdmin(connId, table, action, {
        schema: ports.ipcScopeArg(conn, { connectionId: connId, database, schema }),
        newName,
        kind,
      })
      useUiStore().toast(action === 'rename' ? `已重命名为 ${newName}` : '执行成功')
      void sql
      await refreshTreeNode({
        id: `${connId}::${scope}`,
        kind: 'database',
        label: scope,
        depth: 1,
        expandable: true,
        expanded: true,
      })
      if (action === 'drop' || action === 'rename') {
        // 关闭指向旧表的页签（数据/结构）
        for (const suffix of [`data-${connId}-${table}`, `structure-${connId}-${table}`]) {
          if (ports.hasTab(suffix)) ports.closeTab(suffix)
        }
      }
      return true
    } catch (err) {
      ports.showError(err)
      return false
    }
  }

  return {
    // 状态
    metas,
    keyword,
    selectedResource,
    showSystemSchemas,
    // 派生
    databaseOptions,
    schemaOptions,
    completionTables,
    treeItems,
    visibleTreeItems,
    // 元数据加载
    ensureMeta,
    ensureObjects,
    ensureRedisKeys,
    resolveEditorColumns,
    // connection 域端口
    prefetchConnection,
    invalidateConnectionMeta,
    // 树交互
    toggleSystemSchemas,
    toggleTree,
    selectResource,
    refreshTreeNode,
    scopeContext,
    parseLeafId,
    createTableTemplate,
    // 建表/建库/维护命令
    openCreateTableEditor,
    openCreateTableTab,
    createDatabaseFull,
    dropDatabase,
    executeDdl,
    tableAdminAction,
  }
}
