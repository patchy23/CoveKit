/**
 * database 查询工作区域（页签域）：query/data/structure/redis/create-table 页签，
 * 每个页签的 SQL、dirty、结果与请求状态，运行/取消执行，以及结果分页投影
 * （filteredRows/totalPages/pageRows/tableColumns/resultTabs）。
 *
 * 依赖只以端口注入：connection 域的只读快照（connections/activeConnectionId/activeConnection）
 * 与 library 域的历史、收藏读写命令。catalog 域需要的数据只以只读 computed 视图暴露
 * （activeTabConnectionId/activeTabSchema）与命令函数（openDataTab/openRedisKeyTab 等），
 * 不把可写页签状态对象交给别的域。
 */
import { computed, ref } from 'vue'
import type { ComputedRef } from 'vue'
import type { UiDataGridColumn, UiTabItem } from '@/core/ui'
import type { V2QueryStatus, V2Tab, V2TabKind } from '../useDatabaseMeta'
import type {
  DbColumnInfo,
  DbConnectionInfo,
  DbIndexInfo,
  HistoryEntry,
  SavedEntry,
} from '../contracts'
import { adminIpc, queryIpc } from '../ipc'
import { formatSql } from '../sqlFormat'
import { nextRequestId } from '../requestId'

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

/** 页签上下文：连接 + 库 + schema + 对象名（表/键） */
export interface TabContext {
  connectionId: string
  database: string
  schema: string
  /** 对象名（data/structure/redis 页签的目标表或键） */
  table?: string
}

const PAGE_SIZE = 50

/** 页签上下文 → 后端 schema 入参（mysql/polardb 传库名；PG 系传 schema；sqlite/redis 不用） */
export function ipcScopeArg(conn: DbConnectionInfo, ctx: TabContext): string | undefined {
  if (conn.dbType === 'postgresql' || conn.dbType === 'kingbase' || conn.dbType === 'vastbase') {
    return ctx.schema || 'public'
  }
  if (conn.dbType === 'oracle' || conn.dbType === 'dameng') return ctx.schema || undefined
  return ctx.database || conn.database || undefined
}

/** workspace 域的跨域协作端口（由根门面注入） */
export interface QueryWorkspacePorts {
  /** 连接只读快照（connection 域） */
  connections: ComputedRef<readonly DbConnectionInfo[]>
  /** 当前选中连接 id 只读视图（connection 域） */
  activeConnectionId: ComputedRef<string>
  /** 当前选中连接定义只读视图（connection 域，用于取默认库） */
  activeConnection: ComputedRef<DbConnectionInfo | undefined>
  /** 记录一次查询执行并刷新历史列表（library 域；执行生命周期仍归本域） */
  recordHistory: (
    connId: string,
    sql: string,
    status: 'success' | 'error',
    durationMs: number
  ) => void
  /** 新增收藏，返回收藏 id（library 域） */
  addSaved: (title: string, sql: string) => Promise<number>
  /** 更新收藏（library 域） */
  updateSaved: (id: number, title: string, sql: string) => Promise<void>
  /** 重新拉取收藏列表（library 域） */
  refreshSaved: () => Promise<void>
  /** 应用级错误提示（门面持有 errorHint） */
  showError: (err: unknown) => void
}

export function useQueryWorkspace(ports: QueryWorkspacePorts) {
  // ── 页签 ────────────────────────────────────────────────────────────────
  const tabs = ref<V2Tab[]>([])
  const activeTabId = ref('')
  const tabContexts = ref<Record<string, TabContext>>({})
  const queryStates = ref<Record<string, QueryState>>({})

  /** 在途请求身份：页签 id → 请求 id（结果回填与取消都按它判定归属） */
  const inFlight = new Map<string, string>()
  const structureColumns = ref<Record<string, DbColumnInfo[]>>({})
  /** 结构页签 · 索引子页签数据 */
  const structureIndexes = ref<Record<string, DbIndexInfo[]>>({})
  /** 结构页签 · DDL 子页签（错误信息也写入，前端展示「暂不支持」） */
  const structureDdl = ref<Record<string, string>>({})
  let tabSequence = 0

  // ──────────────────────────────────────────────────────────────────────
  // 派生
  // ──────────────────────────────────────────────────────────────────────

  const activeTab = computed<V2Tab | undefined>(() =>
    tabs.value.find((t) => t.id === activeTabId.value)
  )

  const activeTabKind = computed<V2TabKind>(() => activeTab.value?.kind ?? 'query')

  const activeTabContext = computed<TabContext>(
    () =>
      tabContexts.value[activeTabId.value] ?? {
        connectionId: ports.activeConnectionId.value,
        database: '',
        schema: '',
      }
  )

  const activeTabConnection = computed<DbConnectionInfo | undefined>(() =>
    ports.connections.value.find((c) => c.id === activeTabContext.value.connectionId)
  )

  const queryState = computed<QueryState>(() => {
    queryStates.value[activeTabId.value] ??= makeQueryState()
    return queryStates.value[activeTabId.value]
  })

  /** catalog 只读视图：当前页签所属连接 / schema（库/schema 下拉与编辑器补全） */
  const activeTabConnectionId = computed(() => activeTabContext.value.connectionId)
  const activeTabSchema = computed(() => activeTabContext.value.schema)

  const rowLimitOptions = ['50', '100', '500', '1000'].map((v) => ({ value: v, label: v }))

  // ──────────────────────────────────────────────────────────────────────
  // 页签开闭
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
      database: ports.activeConnection.value?.database ?? '',
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
      connectionId: connectionId ?? ports.activeConnectionId.value,
      database: ports.activeConnection.value?.database ?? '',
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

  /** 表/视图数据页签（树选中「查看数据」入口）：库/schema 取叶子所在 scope，随后加载首页 */
  function openDataTab(connId: string, table: string, database: string, schema: string) {
    const tabId = `data-${connId}-${table}`
    openOrFocusTab(tabId, `${table} · 数据`, 'data', connId, table)
    tabContexts.value[tabId].database = database
    tabContexts.value[tabId].schema = schema
    void loadTableData(tabId)
  }

  /** Redis 键页签（树选中「查看数据」入口）：打开并加载键信息 */
  function openRedisKeyTab(connId: string, key: string) {
    const tabId = `redis-${connId}-${key}`
    openOrFocusTab(tabId, `${key} · 键`, 'redis', connId, key)
    void loadRedisKeyInfo(tabId, key)
  }

  /** 可视化建表页签（mysql 系）：CreateTableTab 按 ctx 渲染列编辑器；库/schema 由 catalog 解析后传入 */
  function openCreateTableTabAt(
    connectionId: string,
    label: string,
    database: string,
    schema: string
  ) {
    tabSequence += 1
    const id = `ct${tabSequence}`
    tabs.value.push({ id, label, kind: 'create-table' })
    tabContexts.value[id] = { connectionId, database, schema }
    activeTabId.value = id
  }

  /** 打开表/视图结构页签（树右键、Ctrl+点击表名共用入口）；列/索引/DDL 一并加载 */
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
    void loadStructureExtras(id)
  }

  /** Ctrl+点击编辑器内表名：按当前页签上下文打开表结构 */
  function openStructureForTable(table: string) {
    const ctx = activeTabContext.value
    if (!ctx.connectionId) return
    openStructureTab(ctx.connectionId, table, ctx.database, ctx.schema)
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

  /** 连接被删除：关闭其下全部页签与页签状态（connection 域经端口调用） */
  function closeTabsForConnection(connectionId: string) {
    tabs.value = tabs.value.filter((t) => tabContexts.value[t.id]?.connectionId !== connectionId)
    Object.keys(tabContexts.value).forEach((tid) => {
      if (tabContexts.value[tid].connectionId === connectionId) {
        delete tabContexts.value[tid]
        delete queryStates.value[tid]
      }
    })
  }

  /** 页签是否存在（catalog 关闭受影响页签前的只读判断） */
  function hasTab(tabId: string): boolean {
    return tabs.value.some((t) => t.id === tabId)
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
      void ports.updateSaved(state.savedId, state.savedTitle, state.sql).catch(() => {})
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
      ports.showError('没有可保存的 SQL 内容')
      return
    }
    if (state.savedId) {
      await ports.updateSaved(state.savedId, state.savedTitle ?? tabLabel(tabId), sql)
    } else {
      const resolved = (title ?? tabLabel(tabId)).trim() || `SQL编辑器 ${tabSequence}`
      state.savedId = await ports.addSaved(resolved, sql)
      state.savedTitle = resolved
      const tab = tabs.value.find((t) => t.id === tabId)
      if (tab) tab.label = resolved
    }
    state.dirty = false
    await ports.refreshSaved()
  }

  // ──────────────────────────────────────────────────────────────────────
  // 查询执行
  // ──────────────────────────────────────────────────────────────────────

  /**
   * 写指定页签的状态对象：用于 `await` 之后的回填。
   * 不能用 `patchQueryState` —— 它写当前活动页签，而用户等待期间可能已切走页签。
   */
  function patchTabState(state: QueryState, patch: Partial<QueryState>) {
    Object.assign(state, patch)
  }

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
    // 请求身份：后端按它登记取消句柄，本函数的回填也按它判定本次结果是否仍然有效
    const requestId = nextRequestId(tabId)
    inFlight.set(tabId, requestId)
    patchQueryState({ status: 'running', error: '', page: 1, columns: [], rows: [], total: 0 })
    try {
      const result = await queryIpc.execute(conn.id, sql, 1000, requestId)
      const durationMs = Date.now() - startedAt
      // 等待期间用户可能切走页签、取消或重新执行：只有本次请求仍是该页签在途请求时才回填
      if (!settleRequest(tabId, requestId)) return
      if (result.ok) {
        patchTabState(state, {
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
        patchTabState(state, {
          status: 'error',
          error: result.error ?? '查询失败',
          resultTab: 'message',
          durationMs,
        })
      }
      ports.recordHistory(conn.id, sql, result.ok ? 'success' : 'error', durationMs)
    } catch (err) {
      // 已取消的请求其失败不再回填（状态停留在「已取消」，不改写成错误）
      if (!settleRequest(tabId, requestId)) return
      patchTabState(state, {
        status: 'error',
        error: String(err),
        resultTab: 'message',
        durationMs: Date.now() - startedAt,
      })
      ports.recordHistory(conn.id, sql, 'error', Date.now() - startedAt)
    }
  }

  /**
   * 结束在途请求：仅当该页签的在途请求仍是本次请求时返回 true（允许回填）；
   * 返回 false 表示已被取消或被新请求取代，本次结果作废。
   */
  function settleRequest(tabId: string, requestId: string): boolean {
    if (inFlight.get(tabId) !== requestId) return false
    inFlight.delete(tabId)
    return true
  }

  async function cancelQuery() {
    const tabId = activeTabId.value
    const conn = activeTabConnection.value
    if (!conn) return
    const requestId = inFlight.get(tabId)
    // 没有在途请求就没有取消对象，不打扰后端（取消与预期缺失不报错）
    if (!requestId) return
    try {
      await queryIpc.cancel(requestId)
    } catch (err) {
      ports.showError(err)
      return
    }
    // 该请求作废：后端迟到的结果不再回填本页签
    inFlight.delete(tabId)
    patchTabState(queryStates.value[tabId] ?? (queryStates.value[tabId] = makeQueryState()), {
      status: 'cancelled',
      resultTab: 'message',
    })
  }

  function onFormatSql() {
    const sql = queryState.value.sql
    patchQueryState({ sql: formatSql(sql), dirty: true })
  }

  // ──────────────────────────────────────────────────────────────────────
  // 数据浏览 / 结构 / Redis 键
  // ──────────────────────────────────────────────────────────────────────

  /** 数据页签：从页签上下文取库/schema 与表名，后端分页 */
  async function loadTableData(tabId: string) {
    const ctx = tabContexts.value[tabId]
    queryStates.value[tabId] ??= makeQueryState()
    // 初始化后从响应式容器重新取值，避免首次异步回填写入原始对象而不触发渲染。
    const state = queryStates.value[tabId]
    if (!ctx) return
    const table = ctx.table ?? tabTableName(tabId)
    if (!table) return
    const conn = ports.connections.value.find((c) => c.id === ctx.connectionId)
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
    const conn = ports.connections.value.find((c) => c.id === ctx.connectionId)
    if (!conn) return
    try {
      structureColumns.value[tabId] = await queryIpc.columns(
        ctx.connectionId,
        table,
        ipcScopeArg(conn, ctx)
      )
    } catch (err) {
      ports.showError(err)
    }
  }

  /** 结构页签：加载索引与 DDL（失败降级为提示文本，不阻塞列信息展示） */
  async function loadStructureExtras(tabId: string) {
    const ctx = tabContexts.value[tabId]
    if (!ctx) return
    const table = ctx.table ?? tabTableName(tabId)
    if (!table) return
    const conn = ports.connections.value.find((c) => c.id === ctx.connectionId)
    if (!conn) return
    const scope = ipcScopeArg(conn, ctx)
    try {
      structureIndexes.value[tabId] = await adminIpc.tableIndexes(ctx.connectionId, table, scope)
    } catch {
      structureIndexes.value[tabId] = []
    }
    try {
      structureDdl.value[tabId] = await adminIpc.tableDdl(ctx.connectionId, table, scope)
    } catch (err) {
      structureDdl.value[tabId] = `-- ${String(err)}`
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
  // 历史 / 收藏 → 编辑器（读取结果的应用，读写本身归 library 域）
  // ──────────────────────────────────────────────────────────────────────

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

  return {
    // 状态
    tabs,
    activeTabId,
    tabContexts,
    queryStates,
    structureColumns,
    structureIndexes,
    structureDdl,
    // 派生
    activeTab,
    activeTabKind,
    activeTabContext,
    activeTabConnection,
    queryState,
    filteredRows,
    totalPages,
    pageRows,
    tableColumns,
    resultTabs,
    rowLimitOptions,
    // catalog 只读视图 + 窄命令
    activeTabConnectionId,
    activeTabSchema,
    ipcScopeArg,
    openDataTab,
    openRedisKeyTab,
    hasTab,
    closeTabsForConnection,
    // 页签命令
    openSqlEditor,
    openSqlEditorWithSql,
    openCreateTableTabAt,
    openStructureTab,
    openStructureForTable,
    closeTab,
    renameActiveTab,
    saveQueryToDisk,
    // 查询命令
    patchQueryState,
    runQuery,
    cancelQuery,
    onFormatSql,
    setPage,
    // 数据加载
    loadTableData,
    loadColumns,
    loadStructureExtras,
    loadRedisKeyInfo,
    // 历史/收藏 → 编辑器
    applyHistory,
    applySaved,
  }
}
