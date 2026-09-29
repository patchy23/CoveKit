/**
 * Database 工作台行为网（AR05）
 *
 * 用途：把 `useDatabase` 组装根（连接 / 查询工作区 / 目录 / 查询库四域 + 兼容门面）的
 * **用户可感知行为**固化成可跑用例。结构重构（`5edcb4a` 的四域拆分）只能用行为等价证明，
 * 返回面 77 个 key 相等不算证据（决策书 §2.3）。
 *
 * 覆盖（对应决策书 §2.3 必需场景）：
 * 1 → 两连接两 tab 结果归属、乱序完成、旧响应晚到不回错页签
 * 2 → 取消的请求身份与「取消一个 tab 不得误伤同连接另一个 tab 的查询」
 * 3 → 取消后重试、重复执行互不覆盖
 * 4 → 关闭一个 tab / 删除一个连接不牵连其它连接与页签
 * 5 → 元数据缓存按连接与 schema 隔离、断开即失效、系统库开关只作用于本连接
 * 6 → 历史与收藏成功路径、失败不伪装成功、列表拉取失败可见
 * 7 → 保存期间切页签：归属仍为发起页签、不抢焦点
 * 8 → 门面提示条可见性（内容与自动清除）与执行前置校验反馈
 *
 * 手法（决策书 §2.1）：mock 本插件 IPC 门面（命令可编程返回、可挂起制造乱序与取消竞态，
 * 不 mock 待测状态所有者本身）；Pinia 每例独立实例；真实挂载最小宿主组件；
 * 断言用户可感知状态与资源责任，不固定无关内部调用顺序。
 */
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h, nextTick } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useUiStore } from '@/stores/ui'
import type { DbConnectionInfo, DbObjectInfo, DbTablePage, QueryResult } from './contracts'
import { useDatabase } from './useDatabase'
import DataTab from './DataTab.vue'
import QueryTab from './QueryTab.vue'
import StructureTab from './StructureTab.vue'
import RedisTab from './RedisTab.vue'

/* ── IPC 门面假实现（vi.hoisted：mock 工厂先于被 mock 模块的导入执行） ── */

const env = vi.hoisted(() => {
  const commands = {
    // 连接
    dbcConnections: vi.fn(),
    dbcConnect: vi.fn(),
    dbcDisconnect: vi.fn(),
    dbcConnectionSave: vi.fn(),
    dbcConnectionDelete: vi.fn(),
    // 查询
    dbcPrepare: vi.fn(),
    dbcExecute: vi.fn(),
    dbcFetch: vi.fn(),
    dbcCloseCursor: vi.fn(async () => undefined),
    exportRows: vi.fn(),
    dialogSave: vi.fn(),
    dbcCancel: vi.fn(),
    // 元数据与结构
    dbcDatabases: vi.fn(),
    dbcSchemas: vi.fn(),
    dbcObjects: vi.fn(),
    dbcColumns: vi.fn(),
    dbcTableData: vi.fn(),
    dbcTableIndexes: vi.fn(),
    dbcTableDdl: vi.fn(),
    dbcRedisKeys: vi.fn(),
    dbcRedisKeyInfo: vi.fn(),
    // 管理命令
    dbcCreateDatabase: vi.fn(),
    dbcDropDatabase: vi.fn(),
    dbcTableAdmin: vi.fn(),
    // 历史与收藏
    dbcDrafts: vi.fn(),
    dbcDraftsSave: vi.fn(),
    dbcHistory: vi.fn(),
    dbcHistoryAdd: vi.fn(),
    dbcHistoryClear: vi.fn(),
    dbcSaved: vi.fn(),
    dbcSavedAdd: vi.fn(),
    dbcSavedUpdate: vi.fn(),
    dbcSavedDelete: vi.fn(),
  }
  return {
    commands,
    reset() {
      for (const fn of Object.values(commands)) fn.mockReset()
    },
  }
})

vi.mock('./ipc', () => ({
  draftIpc: { list: env.commands.dbcDrafts, save: env.commands.dbcDraftsSave },
  connectionIpc: {
    list: env.commands.dbcConnections,
    save: env.commands.dbcConnectionSave,
    remove: env.commands.dbcConnectionDelete,
    connect: env.commands.dbcConnect,
    disconnect: env.commands.dbcDisconnect,
    test: vi.fn(),
    driverStatus: vi.fn(),
  },
  csvIpc: { preview: vi.fn(), import: vi.fn() },
  fileIpc: {
    exportQuery: vi.fn(),
    readSql: vi.fn(),
    writeSql: vi.fn(),
    exportRows: env.commands.exportRows,
  },
  tableIpc: { count: vi.fn(), apply: vi.fn() },
  queryIpc: {
    prepare: env.commands.dbcPrepare,
    closeWorkspace: vi.fn(async () => undefined),
    execute: env.commands.dbcExecute,
    fetch: env.commands.dbcFetch,
    closeCursor: env.commands.dbcCloseCursor,
    cancel: env.commands.dbcCancel,
    databases: env.commands.dbcDatabases,
    schemas: env.commands.dbcSchemas,
    objects: env.commands.dbcObjects,
    columns: env.commands.dbcColumns,
    tableData: env.commands.dbcTableData,
    redisKeys: env.commands.dbcRedisKeys,
    redisKeyInfo: env.commands.dbcRedisKeyInfo,
  },
  adminIpc: {
    charsetOptions: vi.fn(),
    users: vi.fn(),
    createDatabase: env.commands.dbcCreateDatabase,
    dropDatabase: env.commands.dbcDropDatabase,
    tableAdmin: env.commands.dbcTableAdmin,
    tableDdl: env.commands.dbcTableDdl,
    tableIndexes: env.commands.dbcTableIndexes,
  },
  historyIpc: {
    list: env.commands.dbcHistory,
    add: env.commands.dbcHistoryAdd,
    clear: env.commands.dbcHistoryClear,
  },
  savedIpc: {
    list: env.commands.dbcSaved,
    add: env.commands.dbcSavedAdd,
    update: env.commands.dbcSavedUpdate,
    remove: env.commands.dbcSavedDelete,
  },
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: env.commands.dialogSave, open: vi.fn() }))

/* ── 夹具 ── */

function connection(
  id: string,
  label: string,
  over: Partial<DbConnectionInfo> = {}
): DbConnectionInfo {
  return {
    id,
    label,
    dbType: 'mysql',
    env: 'test',
    status: 'online',
    version: '8.0.36',
    latencyMs: 12,
    readonly: false,
    host: '127.0.0.1',
    database: `db-${id}`,
    connectedAt: 1_700_000_000,
    port: 3306,
    username: 'tester',
    ssl: false,
    connectTimeoutMs: 8000,
    ...over,
  }
}

function result(over: Partial<QueryResult> = {}): QueryResult {
  return {
    hasMore: false,
    ok: true,
    columns: ['n'],
    rows: [['1']],
    rowsAffected: 1,
    isQuery: true,
    durationMs: 3,
    truncated: false,
    ...over,
  }
}

function tablePage(over: Partial<DbTablePage> = {}): DbTablePage {
  return {
    columns: ['id'],
    rows: [['1']],
    total: 1,
    page: 1,
    pageSize: 200,
    durationMs: 2,
    ...over,
  }
}

function objectInfo(name: string, kind = 'table'): DbObjectInfo {
  return { kind, name }
}

/** 可手动完成的 Promise（制造乱序完成、晚到响应与取消竞态） */
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined
  let reject: (reason?: unknown) => void = () => undefined
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

/* ── 宿主挂载 ── */

type WorkbenchApi = ReturnType<typeof useDatabase>

/** 记录挂载的宿主，保证每例卸载（composable 的 onBeforeUnmount 责任要真实触发） */
const hosts: Array<() => void> = []

/** 挂载最小宿主组件：四域与门面必须在真实组件里执行，才能覆盖生命周期钩子 */
function mountWorkbench(): WorkbenchApi {
  let api: WorkbenchApi | undefined
  const Host = defineComponent({
    setup() {
      api = useDatabase()
      return () => h('div')
    },
  })
  const wrapper = mount(Host)
  hosts.push(() => wrapper.unmount())
  if (!api) throw new Error('宿主未取得 useDatabase 返回值')
  return api
}

/** 冲刷微任务链（真实计时器下的 await 链） */
async function flush(rounds = 6): Promise<void> {
  for (let index = 0; index < rounds; index += 1) {
    await nextTick()
    await new Promise((resolve) => setTimeout(resolve, 0))
  }
}

/** 切换到某页签（等价于用户点页签，组件层就是写 activeTabId） */
function switchTab(api: WorkbenchApi, tabId: string) {
  api.activeTabId.value = tabId
}

/** 取某个查询页签的状态快照（不存在时抛错，避免断言静默通过） */
function stateOf(api: WorkbenchApi, tabId: string) {
  const state = api.queryStates.value[tabId]
  if (!state) throw new Error(`页签 ${tabId} 无查询状态`)
  return state
}

/** 打开一个带 SQL 的编辑器并固定其连接上下文（返回页签 id） */
function openEditor(api: WorkbenchApi, connId: string, sql: string): string {
  api.openSqlEditor(connId)
  const tabId = api.activeTabId.value
  api.patchQueryState({ sql })
  return tabId
}

/** 取某次执行的真实入参（mock 直接替身 `queryIpc.execute`，故为位置参数） */
function executeArg(sql: string) {
  const call = env.commands.dbcExecute.mock.calls.find((args) => args[1] === sql)
  if (!call) throw new Error(`未发出 SQL 为 ${sql} 的执行请求`)
  return {
    connId: call[0] as string,
    sql: call[1] as string,
    maxRows: call[2] as number | undefined,
    requestId: call[3] as string | undefined,
  }
}

beforeEach(() => {
  setActivePinia(createPinia())
  env.reset()
  env.commands.dbcConnections.mockResolvedValue([
    connection('conn-a', 'A 库'),
    connection('conn-b', 'B 库'),
  ])
  env.commands.dbcDatabases.mockResolvedValue([])
  env.commands.dbcSchemas.mockResolvedValue([])
  env.commands.dbcObjects.mockResolvedValue([])
  env.commands.dbcColumns.mockResolvedValue([])
  env.commands.dbcHistory.mockResolvedValue([])
  env.commands.dbcSaved.mockResolvedValue([])
  env.commands.dbcPrepare.mockResolvedValue({
    requiresConfirmation: false,
    confirmationToken: null,
    target: '测试',
    summary: '',
  })
  env.commands.dbcHistoryAdd.mockResolvedValue(undefined)
  env.commands.dbcCloseCursor.mockResolvedValue(undefined)
  env.commands.dbcCancel.mockResolvedValue(undefined)
  env.commands.dbcConnect.mockImplementation(async (id: string) => connection(id, id))
  env.commands.dbcDisconnect.mockResolvedValue(undefined)
  env.commands.dbcConnectionDelete.mockResolvedValue(undefined)
})

afterEach(() => {
  vi.useRealTimers()
  while (hosts.length) hosts.pop()?.()
  vi.unstubAllGlobals()
})

/* ────────────────────────────────────────────────────────────────────── */
describe('滚动继续读取', () => {
  it('卸载工具主动清空结果模型，外部仍持状态引用也不保留正文', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    env.commands.dbcExecute.mockResolvedValue(
      result({ rows: [['large result']], values: [[{ kind: 'text', value: 'large result' }]] })
    )
    await api.runQuery()
    const retained = api.queryState.value
    hosts.pop()!()
    expect(retained.rows).toEqual([])
    expect(retained.values).toEqual([])
    expect(retained.statements).toEqual([])
    expect(api.queryStates.value).toEqual({})
  })
  it('共享预算保留旧页草稿，游标只在有限窗口内继续前读', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const text = 'x'.repeat(1024 * 1024)
    const protectedTabs: string[] = []
    for (let index = 0; index < 2; index++) {
      const id = openEditor(api, 'conn-a', `SELECT ${index}`)
      env.commands.dbcExecute.mockResolvedValue(result({ rows: [[text], [text]] }))
      await api.runQuery()
      api.patchQueryState({ gridEdits: { 0: { n: { kind: 'text', value: '修改' } } } })
      protectedTabs.push(id)
    }
    const current = openEditor(api, 'conn-a', 'SELECT 3')
    env.commands.dbcExecute.mockResolvedValue(
      result({ rows: [['first-1'], ['first-2']], hasMore: true, cursorId: 'bounded' })
    )
    await api.runQuery()
    env.commands.dbcFetch.mockResolvedValue(
      result({ rows: [['next-1'], ['next-2']], hasMore: false })
    )
    await api.loadMore()
    expect(stateOf(api, current).rows).toEqual([['first-1'], ['first-2'], ['next-1'], ['next-2']])
    expect(stateOf(api, current).loadLimit).toBe('')
    for (const id of protectedTabs) {
      expect(stateOf(api, id).rows).toHaveLength(2)
      expect(stateOf(api, id).gridEdits?.[0].n.value).toBe('修改')
    }
    expect(env.commands.dbcFetch).toHaveBeenCalledWith('conn-a', current, 'bounded', 2)
  })
  it('导出已加载结果时明确提示游标仍有未加载数据', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    api.patchQueryState({
      status: 'success',
      columns: ['n'],
      rows: [['1']],
      values: [[{ kind: 'integer', value: '1' }]],
      total: 1,
      hasMore: true,
      truncated: false,
    })
    const toast = vi.spyOn(useUiStore(), 'toast')
    env.commands.dialogSave.mockResolvedValue('result.csv')
    env.commands.exportRows.mockResolvedValue(1)
    const panel = mount(QueryTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    await flush()
    await panel.get('button[aria-label="导出筛选结果 CSV"]').trigger('click')
    await flush()
    expect(env.commands.exportRows).toHaveBeenCalledWith(
      'result.csv',
      ['n'],
      [[{ kind: 'integer', value: '1' }]]
    )
    expect(toast).toHaveBeenCalledWith(expect.stringContaining('原结果未完整'))
  })
  it('SQL 单飞追加，失败保留已加载结果并按同一偏移重试', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const id = openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    env.commands.dbcExecute.mockResolvedValue(result({ hasMore: true, cursorId: 'cursor-a' }))
    await api.runQuery()
    expect(env.commands.dbcExecute.mock.calls[0][2]).toBe(100)
    const next = deferred<QueryResult>()
    env.commands.dbcFetch.mockReturnValueOnce(next.promise)
    const reading = api.loadMore()
    await api.loadMore()
    expect(env.commands.dbcFetch).toHaveBeenCalledTimes(1)
    next.reject(new Error('暂时无法读取'))
    await reading
    expect(stateOf(api, id).rows).toEqual([['1']])
    expect(stateOf(api, id).loadMoreError).toContain('暂时无法读取')
    env.commands.dbcFetch.mockResolvedValue(result({ rows: [['2']], hasMore: false }))
    await api.loadMore()
    expect(env.commands.dbcFetch).toHaveBeenLastCalledWith('conn-a', id, 'cursor-a', 1)
    expect(stateOf(api, id).rows).toEqual([['1'], ['2']])
    expect(api.pageRows.value).toEqual([['1'], ['2']])
    expect(stateOf(api, id).hasMore).toBe(false)
  })

  it('重新执行前先关闭游标，关闭失败保留旧结果并阻止执行', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const id = openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    env.commands.dbcExecute.mockResolvedValue(result({ hasMore: true, cursorId: 'cursor-a' }))
    await api.runQuery()
    env.commands.dbcCloseCursor.mockRejectedValueOnce(new Error('DB_CURSOR_SESSION_RESET'))
    await api.runQuery('SELECT 2')
    expect(env.commands.dbcExecute).toHaveBeenCalledTimes(1)
    expect(stateOf(api, id).rows).toEqual([['1']])
    expect(stateOf(api, id).error).toContain('DB_CURSOR_SESSION_RESET')
    env.commands.dbcExecute.mockResolvedValue(result({ rows: [['new']] }))
    await api.runQuery('SELECT 2')
    expect(stateOf(api, id).rows).toEqual([['new']])
  })

  it('重新执行后的迟到追加不能混入新结果', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const id = openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    env.commands.dbcExecute.mockResolvedValue(result({ hasMore: true, cursorId: 'old' }))
    await api.runQuery()
    const next = deferred<QueryResult>()
    env.commands.dbcFetch.mockReturnValueOnce(next.promise)
    const reading = api.loadMore()
    env.commands.dbcExecute.mockResolvedValue(result({ rows: [['new']] }))
    await api.runQuery('SELECT 2')
    next.resolve(result({ rows: [['old-late']] }))
    await reading
    expect(env.commands.dbcCloseCursor).toHaveBeenCalledWith('conn-a', id, 'old')
    expect(stateOf(api, id).rows).toEqual([['new']])
  })

  it('表浏览目标页替换当前页，草稿阻止翻页，刷新取消旧页并丢弃迟到结果', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcTableData.mockResolvedValue(
      tablePage({ hasMore: true, values: [[{ kind: 'integer', value: '1' }]] })
    )
    api.selectResource('conn-a::db1::table:users')
    const id = api.activeTabId.value
    await flush()
    api.patchQueryState({ gridEdits: { 0: { id: { kind: 'integer', value: '9' } } } })
    await api.loadMore()
    expect(env.commands.dbcTableData).toHaveBeenCalledTimes(1)
    api.patchQueryState({ gridEdits: {} })
    const next = deferred<DbTablePage>()
    env.commands.dbcTableData.mockReturnValueOnce(next.promise)
    const reading = api.loadMore()
    const nextPage = env.commands.dbcTableData.mock.calls.at(-1)!
    expect(nextPage.slice(2, 4)).toEqual([2, 100])
    expect(nextPage[7]).toBeUndefined()
    env.commands.dbcTableData.mockResolvedValue(tablePage({ rows: [['fresh']], hasMore: false }))
    await api.loadTableData(id)
    next.resolve(tablePage({ rows: [['stale']], hasMore: false }))
    await reading
    expect(env.commands.dbcCancel).toHaveBeenCalledWith(nextPage[8])
    expect(stateOf(api, id).rows).toEqual([['fresh']])
    expect(stateOf(api, id).page).toBe(1)
  })

  it('切换表页期间保留网格实例，返回后替换为目标页', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcTableData.mockResolvedValue(tablePage({ hasMore: true }))
    api.selectResource('conn-a::db1::table:users')
    await flush()
    const panel = mount(DataTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    const grid = panel.findComponent({ name: 'ResultCanvasGrid' }).element
    const next = deferred<DbTablePage>()
    env.commands.dbcTableData.mockReturnValueOnce(next.promise)
    const reading = api.loadMore()
    await nextTick()
    expect(panel.findComponent({ name: 'ResultCanvasGrid' }).element).toBe(grid)
    expect(panel.text()).toContain('正在读取下一批')
    next.resolve(tablePage({ rows: [['2']], hasMore: false }))
    await reading
    await nextTick()
    expect(panel.findComponent({ name: 'ResultCanvasGrid' }).element).toBe(grid)
    expect(api.queryState.value.rows).toEqual([['2']])
  })

  it('游标跨过旧累计行数后仍可继续读取下一页', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const id = openEditor(api, 'conn-a', 'SELECT n FROM numbers')
    const rows = Array.from({ length: 100 }, (_, index) => [String(index)])
    env.commands.dbcExecute.mockResolvedValue(
      result({
        rows,
        hasMore: true,
        cursorId: 'limit',
        pageInfo: { mode: 'cursor', page: 1, pageSize: 100 },
      })
    )
    await api.runQuery()
    const nextRows = Array.from({ length: 100 }, (_, index) => [String(index + 100)])
    env.commands.dbcFetch.mockResolvedValue(
      result({ rows: nextRows, hasMore: true, cursorId: 'limit' })
    )
    await api.loadMore()
    expect(stateOf(api, id).rows).toHaveLength(200)
    expect(stateOf(api, id).rows[199]).toEqual(['199'])
    expect(stateOf(api, id).cursorOffset).toBe(200)
    expect(stateOf(api, id).loadLimit).toBe('')
    expect(env.commands.dbcFetch).toHaveBeenCalledWith('conn-a', id, 'limit', 100)
    expect(env.commands.dbcCloseCursor).not.toHaveBeenCalledWith('conn-a', id, 'limit')
  })

  it('安全 SQL 可直接读取超过旧累计阈值的目标页，并沿用执行时 SQL 快照', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const id = openEditor(api, 'conn-a', 'SELECT n FROM numbers ORDER BY n')
    env.commands.dbcExecute.mockResolvedValueOnce(
      result({
        rows: [['first-page']],
        hasMore: true,
        pageInfo: { mode: 'server', page: 1, pageSize: 100 },
      })
    )
    await api.runQuery()
    api.patchQueryState({ sql: 'SELECT changed FROM elsewhere', dirty: true })
    env.commands.dbcExecute.mockResolvedValueOnce(
      result({
        rows: [['target-page']],
        hasMore: true,
        pageInfo: { mode: 'server', page: 1001, pageSize: 100 },
      })
    )

    expect(await api.goToPage(id, 1001)).toBe(true)
    const targetRequest = env.commands.dbcExecute.mock.calls.at(-1)!
    expect(targetRequest[1]).toBe('SELECT n FROM numbers ORDER BY n')
    expect(targetRequest.slice(7, 9)).toEqual([1001, 100])
    expect(stateOf(api, id).rows).toEqual([['target-page']])
    expect(stateOf(api, id).gridPage).toBe(1001)
  })
})

describe('查询结果归属（决策书 §2.3 场景 1/3）', () => {
  it('大 SQL 格式化在改写或切页时取消，不覆盖其它页签', async () => {
    const workers: {
      onmessage?: (event: { data: unknown }) => void
      postMessage: ReturnType<typeof vi.fn>
      terminate: ReturnType<typeof vi.fn>
    }[] = []
    vi.stubGlobal(
      'Worker',
      class {
        postMessage = vi.fn()
        terminate = vi.fn()
        constructor() {
          workers.push(this)
        }
      }
    )
    const api = mountWorkbench()
    await api.refreshConnections()
    const large = `select '${'x'.repeat(70000)}';`
    openEditor(api, 'conn-a', large)
    const first = api.onFormatSql()
    api.patchQueryState({ sql: 'SELECT edited' })
    await first
    expect(workers[0].terminate).toHaveBeenCalledOnce()
    expect(api.queryState.value.sql).toBe('SELECT edited')
    api.patchQueryState({ sql: large })
    const next = api.onFormatSql()
    openEditor(api, 'conn-b', 'SELECT other')
    await next
    const request = workers[1].postMessage.mock.lastCall![0]
    workers[1].onmessage?.({ data: { id: request.id, result: 'STALE' } })
    await nextTick()
    expect(workers[1].terminate).toHaveBeenCalledOnce()
    expect(api.queryState.value.sql).toBe('SELECT other')
  })
  it('查询结果提供全部已加载行，过滤后无匹配时显示空态', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT id FROM users')
    api.patchQueryState({
      status: 'success',
      columns: ['id'],
      rows: Array.from({ length: 61 }, (_, i) => [`user-${String(i + 1).padStart(3, '0')}`]),
      total: 61,
    })
    const panel = mount(QueryTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    await flush()
    expect(panel.text()).toContain('user-001')
    expect(api.pageRows.value).toHaveLength(61)
    expect(panel.findComponent({ name: 'ResultCanvasGrid' }).props('rows')[50].c0).toBe('user-051')
    expect(
      panel
        .findAll('button')
        .find((button) => button.text().trim() === '下一页')
        ?.attributes('disabled')
    ).toBeDefined()
    await panel.get('input[placeholder="筛选已加载结果…"]').setValue('user-001')
    await flush()
    expect(api.queryState.value.page).toBe(1)
    expect(panel.text()).toContain('user-001')
    expect(panel.text()).not.toContain('user-051')
    await panel.get('input[placeholder="筛选已加载结果…"]').setValue('不存在')
    await flush()
    expect(panel.text()).toContain('无匹配结果')
  })

  it('数据页的结构入口加载 DDL，生成查询保留表名与库范围', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcTableData.mockResolvedValue(tablePage())
    env.commands.dbcColumns.mockResolvedValue([])
    env.commands.dbcTableIndexes.mockResolvedValue([])
    env.commands.dbcTableDdl.mockResolvedValue('CREATE TABLE `user-records` (`id` INT)')
    void api.selectResource('conn-a::db1::table:user-records')
    await flush()
    const data = mount(DataTab, { props: { db: api } })
    hosts.push(() => data.unmount())
    await data.get('button[aria-label="查看结构"]').trigger('click')
    await flush()
    expect(api.structureDdl.value[api.activeTabId.value]).toContain('CREATE TABLE')
    const structure = mount(StructureTab, { props: { db: api } })
    hosts.push(() => structure.unmount())
    expect(structure.text()).toContain('user-records')
    await structure
      .findAll('button')
      .find((button) => button.text() === '生成查询')!
      .trigger('click')
    expect(api.queryState.value.sql).toBe('SELECT * FROM `db1`.`user-records` LIMIT 100;')
    expect(api.activeTabContext.value.database).toBe('db1')
  })

  it('无需展开树即可加载当前库表名，切库和列缓存均按库隔离', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcObjects.mockImplementation(async (_id: string, scope: string) => [
      objectInfo(`${scope}_table`),
    ])
    env.commands.dbcColumns.mockImplementation(
      async (_id: string, _table: string, scope: string) => [{ name: `${scope}_column` }]
    )
    api.openSqlEditorWithSql('conn-a', '', 'db1', '')
    await flush()
    expect(api.completionTables.value.map((table) => table.name)).toEqual(['db1_table'])
    expect(await api.resolveEditorColumns('users')).toEqual(['db1_column'])
    api.activeTabContext.value.database = 'db2'
    await flush()
    expect(api.completionTables.value.map((table) => table.name)).toEqual(['db2_table'])
    expect(await api.resolveEditorColumns('users')).toEqual(['db2_column'])
    expect(env.commands.dbcColumns).toHaveBeenCalledWith('conn-a', 'users', 'db2', 'db2')
    api.activeTabContext.value.database = 'db1'
    await flush()
    expect(await api.resolveEditorColumns('users')).toEqual(['db1_column'])
    expect(env.commands.dbcColumns).toHaveBeenCalledTimes(2)
  })

  it('列补全读取失败不会缓存空结果，刷新目录会重新读取列', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    api.openSqlEditorWithSql('conn-a', '', 'db1', '')
    env.commands.dbcColumns
      .mockRejectedValueOnce(new Error('列读取失败'))
      .mockResolvedValue([{ name: 'id' }])
    expect(await api.resolveEditorColumns('users')).toEqual([])
    expect(api.errorHint.value).toContain('列读取失败')
    expect(await api.resolveEditorColumns('users')).toEqual(['id'])
    await api.refreshTreeNode({ id: 'conn-a::db1', label: 'db1', kind: 'database', depth: 1 })
    env.commands.dbcColumns.mockResolvedValue([{ name: 'new_column' }])
    expect(await api.resolveEditorColumns('users')).toEqual(['new_column'])
  })

  it('刷新连接会重载当前库候选，刷新前的慢响应不覆盖新候选', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const oldObjects = deferred<DbObjectInfo[]>()
    env.commands.dbcObjects
      .mockReturnValueOnce(oldObjects.promise)
      .mockResolvedValue([objectInfo('fresh_table')])
    api.openSqlEditorWithSql('conn-a', '', 'db1', '')
    await flush()
    await api.refreshTreeNode({ id: 'conn-a', label: 'A 库', kind: 'connection', depth: 0 })
    expect(api.completionTables.value.map((table) => table.name)).toEqual(['fresh_table'])
    oldObjects.resolve([objectInfo('stale_table')])
    await flush()
    expect(api.completionTables.value.map((table) => table.name)).toEqual(['fresh_table'])
  })

  it('PostgreSQL 的列补全使用页签 schema，断开重连后丢弃旧列缓存', async () => {
    env.commands.dbcConnections.mockResolvedValue([
      connection('pg', 'PG', { dbType: 'postgresql', database: 'app' }),
    ])
    env.commands.dbcConnect.mockResolvedValue(
      connection('pg', 'PG', { dbType: 'postgresql', database: 'app' })
    )
    const api = mountWorkbench()
    await api.refreshConnections()
    api.openSqlEditorWithSql('pg', '', 'app', 'sales')
    env.commands.dbcColumns.mockResolvedValue([{ name: 'sales_id' }])
    expect(await api.resolveEditorColumns('users')).toEqual(['sales_id'])
    expect(env.commands.dbcColumns).toHaveBeenLastCalledWith('pg', 'users', 'sales', 'app')
    api.activeTabContext.value.schema = 'public'
    env.commands.dbcColumns.mockResolvedValue([{ name: 'public_id' }])
    expect(await api.resolveEditorColumns('users')).toEqual(['public_id'])
    expect(env.commands.dbcColumns).toHaveBeenLastCalledWith('pg', 'users', 'public', 'app')
    await api.disconnect(api.connections.value[0])
    await api.connect(api.connections.value[0])
    env.commands.dbcColumns.mockResolvedValue([{ name: 'updated_id' }])
    expect(await api.resolveEditorColumns('users')).toEqual(['updated_id'])
  })
  it('保留查询和活动语句各自返回的列类型', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabId = openEditor(api, 'conn-a', 'SELECT id FROM users; SELECT created_at FROM users')
    const first = result({ columns: ['id'], columnTypes: ['bigint'], rows: [['1']] })
    const second = result({
      columns: ['created_at'],
      columnTypes: ['timestamp with time zone'],
      rows: [['2026-09-29T00:00:00Z']],
    })
    env.commands.dbcExecute.mockResolvedValue(
      result({
        columns: second.columns,
        columnTypes: second.columnTypes,
        rows: second.rows,
        statements: [first, second],
      })
    )

    await api.runQuery()
    expect(stateOf(api, tabId).columnTypes).toEqual(['timestamp with time zone'])
    api.selectStatement(0)
    expect(stateOf(api, tabId).columnTypes).toEqual(['bigint'])
    api.selectStatement(1)
    expect(stateOf(api, tabId).columnTypes).toEqual(['timestamp with time zone'])
  })

  it('两个连接的两个页签并发执行，结果各自落在发起页签', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    const tabB = openEditor(api, 'conn-b', 'SELECT b')

    const runA = deferred<QueryResult>()
    const runB = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementation(async (_connId: string, sql: string) =>
      sql === 'SELECT a' ? runA.promise : runB.promise
    )

    switchTab(api, tabA)
    const pendingA = api.runQuery()
    switchTab(api, tabB)
    const pendingB = api.runQuery()
    expect(stateOf(api, tabA).status).toBe('running')
    expect(stateOf(api, tabB).status).toBe('running')

    // 乱序完成：后发的先回
    runB.resolve(result({ columns: ['b'], rows: [['b1']], rowsAffected: 1 }))
    await flush()
    runA.resolve(result({ columns: ['a'], rows: [['a1']], rowsAffected: 1 }))
    await Promise.all([pendingA, pendingB])
    await flush()

    expect(stateOf(api, tabA).rows).toEqual([['a1']])
    expect(stateOf(api, tabB).rows).toEqual([['b1']])
    expect(stateOf(api, tabA).status).toBe('success')
    expect(stateOf(api, tabB).status).toBe('success')
  })

  it('切走页签后旧响应晚到，不写入当前页签', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    const tabB = openEditor(api, 'conn-b', 'SELECT b')

    const runA = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementation(async (_connId: string, sql: string) => {
      if (sql === 'SELECT a') return runA.promise
      return result({ columns: ['b'], rows: [['b1']] })
    })

    switchTab(api, tabA)
    const pendingA = api.runQuery()
    // 用户在结果返回前切到另一个页签
    switchTab(api, tabB)
    runA.resolve(result({ columns: ['a'], rows: [['a1']] }))
    await pendingA
    await flush()

    expect(stateOf(api, tabA).rows).toEqual([['a1']])
    expect(stateOf(api, tabA).status).toBe('success')
    // 当前页签没有被别的页签的结果污染
    expect(stateOf(api, tabB).rows).toEqual([])
    expect(stateOf(api, tabB).status).toBe('idle')
  })

  it('失败的查询只写发起页签，并在该页签可见错误', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT bad')
    const tabB = openEditor(api, 'conn-b', 'SELECT good')

    const runA = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementation(async (_connId: string, sql: string) => {
      if (sql === 'SELECT bad') return runA.promise
      return result()
    })

    switchTab(api, tabA)
    const pendingA = api.runQuery()
    switchTab(api, tabB)
    runA.resolve({ ...result(), ok: false, error: '表不存在' })
    await pendingA
    await flush()

    expect(stateOf(api, tabA).status).toBe('error')
    expect(stateOf(api, tabA).error).toBe('表不存在')
    expect(stateOf(api, tabA).resultTab).toBe('message')
    expect(stateOf(api, tabB).status).toBe('idle')
  })

  it('取消只作用于当前页签的请求，同连接另一页签的查询仍完成', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    // 两个页签共用同一连接：后端取消必须按请求身份区分，不能按连接一刀切
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    const tabB = openEditor(api, 'conn-a', 'SELECT b')

    const runA = deferred<QueryResult>()
    const runB = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementation(async (_connId: string, sql: string) =>
      sql === 'SELECT a' ? runA.promise : runB.promise
    )
    env.commands.dbcCancel.mockResolvedValue(undefined)

    switchTab(api, tabA)
    const pendingA = api.runQuery()
    switchTab(api, tabB)
    const pendingB = api.runQuery()
    await flush()

    await api.cancelQuery()
    await flush()

    // 取消请求携带的必须是 B 页签自己的请求身份，而不是连接（否则会连带取消 A）
    const argA = executeArg('SELECT a')
    const argB = executeArg('SELECT b')
    // 两次执行必须各自带请求身份，取消命中的是发起页签那一个
    expect(argB.requestId).toBeTruthy()
    expect(argA.requestId).not.toBe(argB.requestId)
    // 取消必须带**本次页签请求**的身份（不是连接、也不是另一个页签的请求）
    const cancelCalls = env.commands.dbcCancel.mock.calls
    const cancelArg = cancelCalls[cancelCalls.length - 1]?.[0] as string | undefined
    expect(cancelArg).toBe(argB.requestId)
    expect(cancelArg).not.toBe(argA.requestId)
    expect(stateOf(api, tabB).status).toBe('running')
    expect(stateOf(api, tabB).cancelRequested).toBe(true)

    // B 的取消挂起期间，A 的结果照常回填自己的页签
    runA.resolve(result({ columns: ['a'], rows: [['a1']] }))
    await pendingA
    await flush()
    expect(stateOf(api, tabA).status).toBe('success')
    expect(stateOf(api, tabA).rows).toEqual([['a1']])

    runB.reject(new Error('查询已取消'))
    await pendingB.catch(() => undefined)
    await flush()
    expect(stateOf(api, tabB).status).toBe('error')
  })

  it('取消后重试：新结果写入同一页签，页签不停留在 running', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')

    const first = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementationOnce(async () => first.promise)
    env.commands.dbcCancel.mockResolvedValue(undefined)

    const pendingFirst = api.runQuery()
    await flush()
    await api.cancelQuery()
    await flush()
    expect(stateOf(api, tabA).status).toBe('running')
    await api.runQuery()
    expect(env.commands.dbcExecute).toHaveBeenCalledTimes(1)
    first.reject(new Error('查询已取消'))
    await pendingFirst
    await flush()
    expect(stateOf(api, tabA).status).toBe('error')

    env.commands.dbcExecute.mockImplementationOnce(async () =>
      result({ columns: ['a'], rows: [['retry-ok']] })
    )
    await api.runQuery()
    await flush()

    expect(stateOf(api, tabA).status).toBe('success')
    expect(stateOf(api, tabA).rows).toEqual([['retry-ok']])

    // 被取消的首请求迟到返回，也不得覆盖重试结果
    first.resolve(result({ columns: ['a'], rows: [['stale']] }))
    await pendingFirst
    await flush()
    expect(stateOf(api, tabA).rows).toEqual([['retry-ok']])
  })

  it('running 中重复点执行不会发出第二次请求', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT a')

    const first = deferred<QueryResult>()
    env.commands.dbcExecute.mockImplementation(async () => first.promise)
    const pending = api.runQuery()
    await flush()
    await api.runQuery()
    expect(env.commands.dbcExecute).toHaveBeenCalledTimes(1)
    first.resolve(result())
    await pending
  })

  it('未连接与空 SQL 的执行请求被拦下并给出可见提示', async () => {
    const api = mountWorkbench()
    env.commands.dbcConnections.mockResolvedValue([
      connection('conn-a', 'A 库', { status: 'offline' }),
    ])
    await api.refreshConnections()
    const tabId = openEditor(api, 'conn-a', 'SELECT a')

    await api.runQuery()
    expect(env.commands.dbcExecute).not.toHaveBeenCalled()
    expect(stateOf(api, tabId).resultTab).toBe('message')
    expect(stateOf(api, tabId).error).toContain('当前连接已断开')

    env.commands.dbcConnections.mockResolvedValue([connection('conn-a', 'A 库')])
    await api.refreshConnections()
    openEditor(api, 'conn-a', '   ')
    await api.runQuery()
    expect(env.commands.dbcExecute).not.toHaveBeenCalled()
    expect(stateOf(api, api.activeTabId.value).resultTab).toBe('message')
    expect(stateOf(api, api.activeTabId.value).error).toContain('没有可执行的 SQL')
  })
})

/* ────────────────────────────────────────────────────────────────────── */
describe('页签与连接的连带影响（决策书 §2.3 场景 4）', () => {
  it('关闭一个页签不影响另一个页签的状态与页签名', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    const tabB = openEditor(api, 'conn-b', 'SELECT b')
    env.commands.dbcExecute.mockResolvedValue(result({ rows: [['b-kept']] }))
    switchTab(api, tabB)
    await api.runQuery()
    await flush()

    api.closeTab(tabA)
    if (api.closeConfirmation.value) api.confirmClose(true)
    await flush()

    expect(api.tabs.value.map((t) => t.id)).toEqual([tabB])
    expect(stateOf(api, tabB).rows).toEqual([['b-kept']])
    expect(api.activeTabId.value).toBe(tabB)
  })

  it('删除连接只关闭该连接的页签并失效其元数据，其它连接不受影响', async () => {
    const api = mountWorkbench()
    env.commands.dbcDatabases.mockResolvedValue(['db1'])
    env.commands.dbcSchemas.mockResolvedValue(['db1'])
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    const tabB = openEditor(api, 'conn-b', 'SELECT b')

    // 两个连接都加载过元数据
    switchTab(api, tabA)
    await api.ensureMeta('conn-a')
    switchTab(api, tabB)
    await api.ensureMeta('conn-b')
    await flush()
    const callsBefore = env.commands.dbcDatabases.mock.calls.length

    env.commands.dbcConnections.mockResolvedValue([connection('conn-b', 'B 库')])
    await api.removeConnection('conn-a')
    await flush()

    expect(api.tabs.value.map((t) => t.id)).toEqual([tabB])
    expect(api.queryStates.value[tabA]).toBeUndefined()
    expect(api.connections.value.map((c) => c.id)).toEqual(['conn-b'])

    // A 的元数据缓存已失效：回到 A（若仍存在）会重新拉取；B 命中缓存不再拉取
    switchTab(api, tabB)
    await api.ensureMeta('conn-b')
    await flush()
    expect(env.commands.dbcDatabases.mock.calls.length).toBe(callsBefore)
  })

  it('最后一个连接可删除，工作台回到空状态', async () => {
    const api = mountWorkbench()
    env.commands.dbcConnections.mockResolvedValue([connection('conn-a', 'A 库')])
    await api.refreshConnections()
    env.commands.dbcConnections.mockResolvedValue([])
    await api.removeConnection('conn-a')
    expect(env.commands.dbcConnectionDelete).toHaveBeenCalledWith('conn-a')
    expect(api.connections.value).toEqual([])
    expect(api.activeConnectionId.value).toBe('')
  })

  it('删除连接失败时保留连接与其页签（不伪装成功）', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    env.commands.dbcConnectionDelete.mockRejectedValue(new Error('连接被占用'))

    await expect(api.removeConnection('conn-a')).rejects.toThrow('连接被占用')
    await flush()

    expect(api.connections.value.map((c) => c.id)).toContain('conn-a')
    expect(api.tabs.value.map((t) => t.id)).toEqual([tabA])
  })
})

/* ────────────────────────────────────────────────────────────────────── */
describe('元数据缓存隔离与失效（决策书 §2.3 场景 5）', () => {
  it('库/schema 缓存按连接隔离：同名 schema 不串读', async () => {
    const api = mountWorkbench()
    env.commands.dbcDatabases.mockImplementation(async (connId: string) => [
      connId === 'conn-a' ? 'shared' : 'shared',
    ])
    env.commands.dbcSchemas.mockResolvedValue(['shared'])
    env.commands.dbcObjects.mockImplementation(async (connId: string) => [
      objectInfo(connId === 'conn-a' ? 'a_table' : 'b_table'),
    ])
    await api.refreshConnections()
    await api.ensureMeta('conn-a')
    await api.ensureMeta('conn-b')
    await flush()

    // 分别展开两个连接下同名 schema：各自拿到本连接的对象
    api.toggleTree({
      id: 'conn-a::shared',
      label: 'shared',
      depth: 1,
      kind: 'database',
      expandable: true,
      expanded: false,
    })
    await flush()
    api.toggleTree({
      id: 'conn-b::shared',
      label: 'shared',
      depth: 1,
      kind: 'database',
      expandable: true,
      expanded: false,
    })
    await flush()

    const labels = api.treeItems.value.map((item) => item.label)
    expect(labels).toContain('a_table')
    expect(labels).toContain('b_table')

    // 第二次展开命中缓存，不再请求后端
    const objectsCalls = env.commands.dbcObjects.mock.calls.length
    api.toggleTree({
      id: 'conn-a::shared',
      label: 'shared',
      depth: 1,
      kind: 'database',
      expandable: false,
      expanded: true,
    })
    await flush()
    expect(env.commands.dbcObjects.mock.calls.length).toBe(objectsCalls)
  })

  it('断开连接失效该连接元数据缓存，其它连接缓存保留', async () => {
    const api = mountWorkbench()
    env.commands.dbcDatabases.mockResolvedValue(['db1'])
    env.commands.dbcSchemas.mockResolvedValue(['db1'])
    await api.refreshConnections()
    await api.ensureMeta('conn-a')
    await api.ensureMeta('conn-b')
    await flush()
    expect(env.commands.dbcDatabases).toHaveBeenCalledTimes(2)

    await api.disconnect(api.connections.value.find((c) => c.id === 'conn-a')!)
    expect(api.connections.value.find((c) => c.id === 'conn-a')?.status).toBe('offline')

    await api.ensureMeta('conn-a')
    await flush()
    expect(env.commands.dbcDatabases).toHaveBeenCalledTimes(3)

    await api.ensureMeta('conn-b')
    await flush()
    expect(env.commands.dbcDatabases).toHaveBeenCalledTimes(3)
  })

  it('库/schema 下拉跟随当前页签的连接，系统库开关只影响本连接', async () => {
    const api = mountWorkbench()
    env.commands.dbcDatabases.mockImplementation(async (connId: string) =>
      connId === 'conn-a' ? ['information_schema', 'biz'] : ['mysql', 'biz_b']
    )
    env.commands.dbcSchemas.mockResolvedValue([])
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    await api.ensureMeta('conn-a')
    await api.ensureMeta('conn-b')
    await flush()

    switchTab(api, tabA)
    expect(api.databaseOptions.value.map((o) => o.value)).toEqual(['biz'])

    // 打开本连接的系统库开关后可见；另一连接仍隐藏其系统库
    api.toggleSystemSchemas('conn-a')
    await flush()
    expect(api.databaseOptions.value.map((o) => o.value)).toContain('information_schema')

    const tabB = openEditor(api, 'conn-b', 'SELECT b')
    switchTab(api, tabB)
    expect(api.databaseOptions.value.map((o) => o.value)).toEqual(['biz_b'])
  })

  it('元数据拉取失败经提示条可见，且不阻塞后续重试', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcDatabases
      .mockRejectedValueOnce(new Error('连接已断开'))
      .mockResolvedValue(['db1'])
    env.commands.dbcSchemas.mockResolvedValue(['db1'])

    await api.ensureMeta('conn-a')
    await flush()
    expect(api.errorHint.value).toContain('连接已断开')

    api.showError('')
    env.commands.dbcDatabases.mockReset()
    env.commands.dbcDatabases.mockResolvedValue(['db1'])
    await api.ensureMeta('conn-a')
    await flush()
    const tab = openEditor(api, 'conn-a', 'SELECT 1')
    switchTab(api, tab)
    expect(api.databaseOptions.value.map((o) => o.value)).toEqual(['db1'])
  })
})

/* ────────────────────────────────────────────────────────────────────── */
describe('历史与收藏（决策书 §2.3 场景 6）', () => {
  it('执行成功/失败都写入历史并刷新列表', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    env.commands.dbcExecute.mockResolvedValue(result({ rows: [['a1']] }))
    env.commands.dbcHistory.mockResolvedValue([
      { id: 1, connId: 'conn-a', sql: 'SELECT a', status: 'success', durationMs: 3, at: 'now' },
    ])

    await api.runQuery()
    await flush()

    expect(env.commands.dbcHistoryAdd).toHaveBeenCalledWith(
      'conn-a',
      'SELECT a',
      'success',
      expect.any(Number),
      { database: 'db-conn-a', schema: '' }
    )
    expect(api.history.value.map((h) => h.id)).toEqual([1])
    expect(stateOf(api, tabA).status).toBe('success')

    env.commands.dbcExecute.mockResolvedValue({ ...result(), ok: false, error: '语法错误' })
    await api.runQuery()
    await flush()
    expect(env.commands.dbcHistoryAdd).toHaveBeenLastCalledWith(
      'conn-a',
      'SELECT a',
      'error',
      expect.any(Number),
      { database: 'db-conn-a', schema: '' }
    )
  })

  it('历史写入失败不影响执行结果呈现，历史列表拉取失败经提示条可见', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT a')
    env.commands.dbcExecute.mockResolvedValue(result({ rows: [['a1']] }))
    env.commands.dbcHistoryAdd.mockRejectedValue(new Error('磁盘只读'))
    env.commands.dbcHistory.mockRejectedValueOnce(new Error('历史库打不开'))

    await api.runQuery()
    await flush()

    expect(stateOf(api, api.activeTabId.value).status).toBe('success')
    expect(api.errorHint.value).toContain('磁盘只读')
    await api.refreshHistory()
    expect(api.errorHint.value).toContain('历史库打不开')
  })

  it('收藏保存成功：落库、页签更名为别名、dirty 复位', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    env.commands.dbcSavedAdd.mockResolvedValue(7)
    env.commands.dbcSaved.mockResolvedValue([
      { id: 7, title: '我的查询', sql: 'SELECT a', at: 'now' },
    ])

    await api.saveQueryToDisk('我的查询')
    await flush()

    expect(env.commands.dbcSavedAdd).toHaveBeenCalledWith('我的查询', 'SELECT a')
    expect(stateOf(api, tabA).savedId).toBe(7)
    expect(stateOf(api, tabA).dirty).toBe(false)
    expect(api.tabs.value.find((t) => t.id === tabA)?.label).toBe('我的查询')
    expect(api.savedSql.value.map((s) => s.id)).toEqual([7])
  })

  it('无内容保存被拦下并提示，不发出落库请求', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', '   ')

    await api.saveQueryToDisk('空')

    expect(env.commands.dbcSavedAdd).not.toHaveBeenCalled()
    expect(api.errorHint.value).toContain('没有可保存的 SQL 内容')
  })

  it('收藏保存失败：向上抛出且不留下假的已保存状态', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    api.patchQueryState({ dirty: true })
    env.commands.dbcSavedAdd.mockRejectedValue(new Error('库已锁定'))

    await expect(api.saveQueryToDisk('失败保存')).rejects.toThrow('库已锁定')
    await flush()

    expect(stateOf(api, tabA).savedId).toBeUndefined()
    expect(stateOf(api, tabA).dirty).toBe(true)
    expect(api.tabs.value.find((t) => t.id === tabA)?.label).not.toBe('失败保存')
  })

  it('删除收藏与清空历史后列表同步刷新', async () => {
    const api = mountWorkbench()
    env.commands.dbcSavedDelete.mockResolvedValue(undefined)
    env.commands.dbcHistoryClear.mockResolvedValue(undefined)
    env.commands.dbcSaved
      .mockResolvedValueOnce([{ id: 7, title: 'x', sql: 'SELECT 1', at: 'now' }])
      .mockResolvedValue([])
    env.commands.dbcHistory
      .mockResolvedValueOnce([
        { id: 1, connId: 'conn-a', sql: 'SELECT 1', status: 'success', durationMs: 1, at: 'now' },
      ])
      .mockResolvedValue([])

    await api.refreshSaved()
    await api.refreshHistory()
    expect(api.savedSql.value.map((s) => s.id)).toEqual([7])

    await api.removeSaved(7)
    await flush()
    expect(env.commands.dbcSavedDelete).toHaveBeenCalledWith(7)
    expect(api.savedSql.value).toEqual([])

    await api.clearHistory()
    await flush()
    expect(env.commands.dbcHistoryClear).toHaveBeenCalled()
    expect(api.history.value).toEqual([])
  })
})

/* ────────────────────────────────────────────────────────────────────── */
describe('保存期间切页签的归属（决策书 §2.3 场景 7）', () => {
  it('保存过程中切换到别的页签，保存结果仍写回发起页签且不抢焦点', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    api.patchQueryState({ dirty: true })

    const saving = deferred<number>()
    env.commands.dbcSavedAdd.mockImplementation(async () => saving.promise)
    env.commands.dbcSaved.mockResolvedValue([])

    const pending = api.saveQueryToDisk('保存中')
    await flush()
    // 落库还没回来，用户切到另一个页签
    const tabB = openEditor(api, 'conn-b', 'SELECT b')
    expect(api.activeTabId.value).toBe(tabB)

    saving.resolve(11)
    await pending
    await flush()

    expect(stateOf(api, tabA).savedId).toBe(11)
    expect(stateOf(api, tabA).dirty).toBe(false)
    expect(api.tabs.value.find((t) => t.id === tabA)?.label).toBe('保存中')
    // 另一个页签不被牵连：既没被改名，也没被标记为已保存
    expect(api.tabs.value.find((t) => t.id === tabB)?.label).toBe('SQL编辑器 2')
    expect(stateOf(api, tabB).savedId).toBeUndefined()
    expect(api.activeTabId.value).toBe(tabB)
  })

  it('已保存页签重命名同步到库；重命名空别名被忽略', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tabA = openEditor(api, 'conn-a', 'SELECT a')
    env.commands.dbcSavedAdd.mockResolvedValue(5)
    env.commands.dbcSaved.mockResolvedValue([])
    env.commands.dbcSavedUpdate.mockResolvedValue(undefined)

    await api.saveQueryToDisk('原名')
    await flush()
    api.renameActiveTab('新名')
    await flush()
    expect(env.commands.dbcSavedUpdate).toHaveBeenCalledWith(5, '新名', 'SELECT a')

    const before = env.commands.dbcSavedUpdate.mock.calls.length
    api.renameActiveTab('   ')
    await flush()
    expect(env.commands.dbcSavedUpdate.mock.calls.length).toBe(before)
    expect(api.tabs.value.find((t) => t.id === tabA)?.label).toBe('新名')
  })

  it('从历史/收藏应用到编辑器：已有内容的编辑器另开页签，收藏页签唯一', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    openEditor(api, 'conn-a', 'SELECT 原有')

    api.applyHistory({
      id: 1,
      connId: 'conn-a',
      sql: 'SELECT 历史',
      status: 'error',
      durationMs: 1,
      at: 'now',
    })
    await flush()
    expect(api.tabs.value).toHaveLength(2)
    expect(stateOf(api, api.activeTabId.value).sql).toBe('SELECT 历史')
    expect(stateOf(api, api.activeTabId.value).dirty).toBe(true)

    api.applySaved({ id: 9, title: '收藏 9', sql: 'SELECT 收藏', at: 'now' })
    await flush()
    const savedTab = api.activeTabId.value
    expect(api.tabs.value.find((t) => t.id === savedTab)?.label).toBe('收藏 9')
    expect(stateOf(api, savedTab).savedId).toBe(9)

    // 同一收藏再次应用只聚焦已打开的页签，不重复开
    const tabCount = api.tabs.value.length
    api.applySaved({ id: 9, title: '收藏 9', sql: 'SELECT 收藏', at: 'now' })
    await flush()
    expect(api.tabs.value).toHaveLength(tabCount)
    expect(api.activeTabId.value).toBe(savedTab)
  })
})

/* ────────────────────────────────────────────────────────────────────── */
describe('门面提示与树命令（决策书 §2.1 反馈可见性）', () => {
  it('错误正文保持可见直到手动关闭，新错误替换旧消息', async () => {
    const api = mountWorkbench()
    vi.useFakeTimers()
    api.showError(new Error('连接超时'))
    expect(api.errorHint.value).toBe('连接超时')

    api.showError('第二条')
    expect(api.errorHint.value).toBe('第二条')
    await vi.advanceTimersByTimeAsync(3999)
    expect(api.errorHint.value).toBe('第二条')
    await vi.advanceTimersByTimeAsync(2)
    expect(api.errorHint.value).toBe('第二条')
    api.showError('')
    expect(api.errorHint.value).toBe('')
  })

  it('树上执行 DDL 成功给 toast 并刷新该 scope，失败只提示不 toast', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const ui = useUiStore()
    const toast = vi.spyOn(ui, 'toast')
    env.commands.dbcExecute.mockResolvedValue(result())
    env.commands.dbcObjects.mockResolvedValue([])

    await api.executeDdl('conn-a', 'CREATE TABLE t (id INT)', 'db1')
    await flush()
    expect(toast).toHaveBeenCalledWith('执行成功')

    toast.mockClear()
    env.commands.dbcExecute.mockResolvedValue({ ...result(), ok: false, error: '语法错误' })
    const ok = await api.executeDdl('conn-a', 'CREATE TABLE', 'db1')
    await flush()
    expect(ok).toBe(false)
    expect(toast).not.toHaveBeenCalled()
    expect(api.errorHint.value).toContain('语法错误')
  })

  it('表删除/重命名后关闭指向旧表的页签，其它页签不受影响', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    env.commands.dbcObjects.mockResolvedValue([objectInfo('users')])
    env.commands.dbcTableAdmin.mockResolvedValue('ALTER TABLE users RENAME TO users2')
    env.commands.dbcDatabases.mockResolvedValue(['db1'])
    env.commands.dbcSchemas.mockResolvedValue(['db1'])
    await api.ensureMeta('conn-a')

    // 打开旧表的数据页签（等价于树上选中该表）与另一个普通编辑器页签
    api.selectResource('conn-a::db1::table:users')
    const dataTab = api.activeTabId.value
    await flush()
    const editorTab = openEditor(api, 'conn-a', 'SELECT 1')
    expect(api.tabs.value.map((t) => t.id)).toContain(dataTab)

    await api.tableAdminAction('conn-a', 'db1', 'users', 'rename', 'users2')
    await flush()

    expect(api.tabs.value.map((t) => t.id)).toEqual([editorTab])
    expect(api.activeTabId.value).toBe(editorTab)
  })

  it('首次查看表数据时，异步返回后直接渲染列与数据，无需刷新', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const pending = deferred<DbTablePage>()
    env.commands.dbcTableData.mockReturnValue(pending.promise)

    void api.selectResource('conn-a::db1::table:users')
    const panel = mount(DataTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    await nextTick()
    expect(panel.text()).not.toContain('首次加载用户')

    pending.resolve(tablePage({ columns: ['用户名'], rows: [['首次加载用户']], total: 1 }))
    await flush()

    expect(panel.text()).toContain('用户名')
    expect(panel.text()).toContain('首次加载用户')
    expect(panel.text()).toContain('已加载 1 行')
    expect(env.commands.dbcTableData).toHaveBeenCalledTimes(1)
    expect(env.commands.dbcTableData).toHaveBeenCalledWith(
      'conn-a',
      'users',
      1,
      100,
      'db1',
      'db1',
      undefined,
      undefined,
      expect.any(String)
    )
  })

  it('首次查看表数据失败时立即显示错误，重试后显示数据', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const pending = deferred<DbTablePage>()
    env.commands.dbcTableData.mockReturnValueOnce(pending.promise)

    void api.selectResource('conn-a::db1::table:users')
    const panel = mount(DataTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    await nextTick()
    pending.reject(new Error('测试连接读取失败'))
    await flush()

    expect(panel.text()).toContain('加载失败')
    expect(panel.text()).toContain('测试连接读取失败')
    env.commands.dbcTableData.mockResolvedValue(
      tablePage({ columns: ['用户名'], rows: [['重试成功用户']], total: 1 })
    )
    const retry = panel.findAll('button').find((button) => button.text() === '重试')
    expect(retry).toBeDefined()
    await retry!.trigger('click')
    await flush()

    expect(panel.text()).toContain('重试成功用户')
    expect(panel.text()).not.toContain('加载失败')
  })

  it('数据表按服务端目标页替换结果，滚动不取数且保留页内行索引', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const rows = Array.from({ length: 500 }, (_, index) => [`row-${index}`])
    env.commands.dbcTableData.mockImplementation(
      async (_connId: string, _table: string, page: number, size: number) =>
        tablePage({
          rows: rows.slice((page - 1) * size, page * size),
          total: rows.length,
          page,
          pageSize: size,
          hasMore: page * size < rows.length,
        })
    )
    api.selectResource('conn-a::db1::table:users')
    await flush()
    const tabId = api.activeTabId.value
    const panel = mount(DataTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    const grid = () => panel.findComponent({ name: 'EditableResultGrid' })

    expect(grid().props('rows')).toHaveLength(100)
    expect(grid().props('rows')[0].__row).toBe('0')
    await panel.findComponent({ name: 'ResultCanvasGrid' }).trigger('scroll')
    await flush()
    expect(env.commands.dbcTableData).toHaveBeenCalledTimes(1)

    await panel
      .findAll('button')
      .find((button) => button.text().includes('下一页'))!
      .trigger('click')
    await flush()
    expect(stateOf(api, tabId).gridPage).toBe(2)

    const pageSize = panel.findComponent({ name: 'UiSelect' })
    await pageSize.vm.$emit('update:modelValue', '200')
    await flush()
    expect(stateOf(api, tabId).gridPage).toBe(2)
    expect(grid().props('rows')).toHaveLength(200)
    expect(grid().props('rows')[0]).toMatchObject({ __row: '0', __label: '201' })

    await panel
      .findAll('button')
      .find((button) => button.text().includes('下一页'))!
      .trigger('click')
    await flush()
    expect(stateOf(api, tabId).gridPage).toBe(3)
    expect(grid().props('rows')).toHaveLength(100)
    expect(grid().props('rows')[0]).toMatchObject({ __row: '0', __label: '401' })

    await pageSize.vm.$emit('update:modelValue', '1000')
    await flush()
    expect(stateOf(api, tabId).gridPage).toBe(3)
    expect(stateOf(api, tabId).status).toBe('empty')
    expect(grid().props('rows')).toHaveLength(0)
    expect(panel.text()).toContain('第 3 页 · 已加载 0 行')
    expect(env.commands.dbcTableData.mock.calls.map((call) => call.slice(2, 4))).toEqual([
      [1, 100],
      [2, 100],
      [2, 200],
      [3, 200],
      [3, 1000],
    ])
  })

  it('数据页签按自己的页签状态加载分页数据，切换页签不影响已加载结果', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const pageA = deferred<DbTablePage>()
    env.commands.dbcTableData.mockImplementation(async (_connId: string, table: string) =>
      table === 'users' ? pageA.promise : tablePage({ rows: [['orders-1']], total: 1 })
    )

    api.selectResource('conn-a::db1::table:users')
    const tabA = api.activeTabId.value
    api.selectResource('conn-a::db1::table:orders')
    const tabB = api.activeTabId.value
    await flush()

    pageA.resolve(tablePage({ rows: [['users-1']], total: 1 }))
    await flush()

    expect(stateOf(api, tabA).rows).toEqual([['users-1']])
    expect(stateOf(api, tabA).status).toBe('success')
    expect(stateOf(api, tabB).rows).toEqual([['orders-1']])
  })
})

describe('执行准备和持久化竞态', () => {
  it('预检期间取消后不再向数据库发送 SQL', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tab = openEditor(api, 'conn-a', 'SELECT 1')
    const preparing = deferred<{ requiresConfirmation: boolean; target: string; summary: string }>()
    env.commands.dbcPrepare.mockReturnValue(preparing.promise)
    const pending = api.runQuery()
    await flush()
    await api.cancelQuery()
    preparing.resolve({ requiresConfirmation: false, target: '测试', summary: '' })
    await pending
    expect(env.commands.dbcExecute).not.toHaveBeenCalled()
    expect(stateOf(api, tab).status).toBe('idle')
    expect(stateOf(api, tab).error).toContain('尚未执行')
  })

  it('收藏保存等待期间继续编辑仍保留未保存标记', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const tab = openEditor(api, 'conn-a', 'SELECT 1')
    const saving = deferred<number>()
    env.commands.dbcSavedAdd.mockReturnValue(saving.promise)
    const pending = api.saveQueryToDisk('查询')
    api.patchQueryState({ sql: 'SELECT 2', dirty: true })
    saving.resolve(17)
    await pending
    expect(stateOf(api, tab).sql).toBe('SELECT 2')
    expect(stateOf(api, tab).dirty).toBe(true)
  })

  it('连接取消清理完成前不允许重连，迟到成功不复活连接', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const target = api.connections.value[0]
    const opening = deferred<DbConnectionInfo>()
    env.commands.dbcConnect.mockReturnValue(opening.promise)
    const pending = api.connect(target)
    api.cancelConnect(target.id)
    await api.connect(target)
    expect(env.commands.dbcConnect).toHaveBeenCalledTimes(1)
    opening.resolve(target)
    await pending
    expect(env.commands.dbcDisconnect).toHaveBeenCalledWith(target.id)
    expect(api.connecting.value[target.id]).toBe(false)
  })
})

describe('SQL 草稿恢复', () => {
  it('慢保存期间合并中间 SQL，关闭等待最终快照保存完成', async () => {
    env.commands.dbcDrafts.mockResolvedValue([])
    const first = deferred<void>()
    const last = deferred<void>()
    env.commands.dbcDraftsSave.mockReturnValueOnce(first.promise).mockReturnValueOnce(last.promise)
    const api = mountWorkbench()
    await api.restoreDrafts()
    openEditor(api, 'conn-a', 'SELECT 0')
    const pending = [api.flushDrafts()]
    for (let i = 1; i <= 100; i++) {
      api.patchQueryState({ sql: `SELECT ${i}`, dirty: true })
      pending.push(api.flushDrafts())
    }
    expect(env.commands.dbcDraftsSave).toHaveBeenCalledTimes(1)
    first.resolve()
    await flush()
    expect(env.commands.dbcDraftsSave).toHaveBeenCalledTimes(2)
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [expect.objectContaining({ sql: 'SELECT 100', dirty: true })],
      [api.activeTabId.value]
    )
    let finished = false
    const closing = Promise.all(pending).then(() => {
      finished = true
    })
    await flush()
    expect(finished).toBe(false)
    last.resolve()
    await closing
    expect(finished).toBe(true)
  })

  it('合并保存失败传给全部等待者，重试保存仍保留的最新草稿', async () => {
    env.commands.dbcDrafts.mockResolvedValue([])
    const failed = deferred<void>()
    env.commands.dbcDraftsSave.mockReturnValueOnce(failed.promise).mockResolvedValue(undefined)
    const api = mountWorkbench()
    await api.restoreDrafts()
    openEditor(api, 'conn-a', 'SELECT 1')
    const first = api.flushDrafts()
    api.patchQueryState({ sql: 'SELECT 2', dirty: true })
    const second = api.flushDrafts()
    const results = Promise.allSettled([first, second])
    failed.reject(new Error('磁盘已满'))
    expect((await results).map((result) => result.status)).toEqual(['rejected', 'rejected'])
    expect(api.queryState.value.sql).toBe('SELECT 2')
    expect(api.queryState.value.dirty).toBe(true)
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [expect.objectContaining({ sql: 'SELECT 2', dirty: true })],
      [api.activeTabId.value]
    )
  })

  it('恢复目标和选区，保持离线且不会执行 SQL 或恢复事务', async () => {
    env.commands.dbcDrafts.mockResolvedValue([
      {
        label: '待处理',
        sql: 'DELETE FROM items',
        connectionId: 'removed',
        database: 'db',
        schema: 'custom',
        from: 2,
        to: 7,
        dirty: true,
        active: true,
      },
    ])
    env.commands.dbcDraftsSave.mockResolvedValue(undefined)
    const api = mountWorkbench()
    await api.restoreDrafts()
    expect(api.queryState.value.sql).toBe('DELETE FROM items')
    expect(api.queryState.value.selection).toEqual({ from: 2, to: 7 })
    expect(api.activeTabContext.value).toMatchObject({
      connectionId: 'removed',
      database: 'db',
      schema: 'custom',
    })
    expect(api.queryState.value.transactionActive).toBeFalsy()
    expect(env.commands.dbcExecute).not.toHaveBeenCalled()
    expect(env.commands.dbcConnect).not.toHaveBeenCalled()
    api.patchQueryState({ sql: 'SELECT 2' })
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [expect.objectContaining({ sql: 'SELECT 2' })],
      [api.activeTabId.value]
    )
    await api.closeTab(api.activeTabId.value, true)
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith([], [])
  })
  it('只传修改文档，未变正文不因另一个页签编辑而重复发送', async () => {
    env.commands.dbcDrafts.mockResolvedValue([])
    env.commands.dbcDraftsSave.mockResolvedValue(undefined)
    const api = mountWorkbench()
    await api.restoreDrafts()
    openEditor(api, 'conn-a', 'SELECT large_text_1')
    const first = api.activeTabId.value
    openEditor(api, 'conn-a', 'SELECT large_text_2')
    const second = api.activeTabId.value
    await api.flushDrafts()
    api.patchQueryState({ sql: 'SELECT changed' })
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [expect.objectContaining({ id: second, sql: 'SELECT changed' })],
      [first, second]
    )
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith([], [first, second])
  })
  it('只移动选区或切换页签不传 SQL，失败后的重试保留变化', async () => {
    env.commands.dbcDrafts.mockResolvedValue([])
    env.commands.dbcDraftsSave.mockResolvedValue(undefined)
    const api = mountWorkbench()
    await api.restoreDrafts()
    openEditor(api, 'conn-a', 'x'.repeat(70000))
    const first = api.activeTabId.value
    openEditor(api, 'conn-a', 'SELECT 2')
    const second = api.activeTabId.value
    await api.flushDrafts()
    api.activeTabId.value = first
    api.patchQueryState({ selection: { from: 120, to: 130 } })
    env.commands.dbcDraftsSave.mockRejectedValueOnce(new Error('磁盘已满'))
    await expect(api.flushDrafts()).rejects.toThrow('磁盘已满')
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [],
      [first, second],
      [
        { id: first, from: 120, to: 130, active: true },
        { id: second, from: 0, to: 0, active: false },
      ]
    )
    api.patchQueryState({ sql: 'SELECT changed' })
    await api.flushDrafts()
    expect(env.commands.dbcDraftsSave).toHaveBeenLastCalledWith(
      [expect.objectContaining({ id: first, sql: 'SELECT changed', from: 120, to: 130 })],
      [first, second]
    )
  })
  it('恢复失败不覆盖唯一草稿，保存失败传给关闭流程', async () => {
    env.commands.dbcDrafts.mockRejectedValue(new Error('磁盘读取失败'))
    const api = mountWorkbench()
    await api.restoreDrafts()
    openEditor(api, 'conn-a', 'SELECT 3')
    await expect(api.flushDrafts()).rejects.toThrow('旧草稿读取失败')
    expect(env.commands.dbcDraftsSave).not.toHaveBeenCalled()
    expect(api.errorHint.value).toContain('磁盘读取失败')
    env.commands.dbcDrafts.mockResolvedValue([])
    await api.restoreDrafts()
    env.commands.dbcDraftsSave.mockRejectedValue(new Error('磁盘已满'))
    await expect(api.flushDrafts()).rejects.toThrow('磁盘已满')
  })
})

describe('Redis 与结构页异步归属', () => {
  it('Redis 首次返回直接渲染，刷新晚到不覆盖最新内容，过期键有明确提示', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const first = deferred<{ key: string; kind: string; ttl: number; value: string }>()
    env.commands.dbcRedisKeyInfo.mockReturnValueOnce(first.promise)
    await api.selectResource('conn-a::db1::key:sample')
    const id = api.activeTabId.value
    const panel = mount(RedisTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    first.resolve({ key: 'sample', kind: 'string', ttl: -1, value: '首次内容' })
    await flush()
    expect(panel.text()).toContain('首次内容')
    const old = deferred<{ key: string; kind: string; ttl: number; value: string }>()
    env.commands.dbcRedisKeyInfo
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce({ key: 'sample', kind: 'none', ttl: -2, value: '' })
    const slow = api.loadRedisKeyInfo(id, 'sample')
    await api.loadRedisKeyInfo(id, 'sample')
    old.resolve({ key: 'sample', kind: 'string', ttl: -1, value: '旧内容' })
    await slow
    await flush()
    expect(panel.text()).toContain('键不存在或已过期')
    expect(panel.text()).not.toContain('旧内容')
  })

  it('同名表结构按库与 schema 分开，关闭后重开的旧列请求不得回写', async () => {
    const api = mountWorkbench()
    await api.refreshConnections()
    const old = deferred<never[]>()
    env.commands.dbcColumns.mockReturnValueOnce(old.promise).mockResolvedValue([])
    env.commands.dbcTableIndexes.mockResolvedValue([])
    env.commands.dbcTableDdl.mockResolvedValue('CREATE TABLE sample (id INT)')
    api.openStructureTab('conn-a', 'sample', 'one', 'a')
    const one = api.activeTabId.value
    api.openStructureTab('conn-a', 'sample', 'two', 'a')
    expect(api.activeTabId.value).not.toBe(one)
    api.closeTab(one, true)
    env.commands.dbcColumns.mockRejectedValueOnce(new Error('列读取失败'))
    api.openStructureTab('conn-a', 'sample', 'one', 'a')
    await flush()
    old.resolve([])
    await flush()
    expect(api.structureColumns.value[one]).toBeUndefined()
    expect(api.structureErrors.value[one].columns).toContain('列读取失败')
    const panel = mount(StructureTab, { props: { db: api } })
    hosts.push(() => panel.unmount())
    expect(panel.text()).toContain('列读取失败')
  })
})

it('网格草稿保留在原页签，并阻止刷新、重新执行和无确认关闭', async () => {
  const api = mountWorkbench()
  await api.refreshConnections()
  const id = openEditor(api, 'conn-a', 'SELECT * FROM users')
  api.patchTabQueryState(id, { gridEdits: { '0': { name: { kind: 'text', value: '待保存' } } } })
  await api.runQuery()
  expect(env.commands.dbcExecute).not.toHaveBeenCalled()
  api.closeTab(id)
  expect(api.closeConfirmation.value?.dirty).toBe(true)
  expect(api.tabs.value.some((tab) => tab.id === id)).toBe(true)
  const other = openEditor(api, 'conn-b', 'SELECT 1')
  api.patchTabQueryState(id, { gridMessage: '原页反馈' })
  expect(api.activeTabId.value).toBe(other)
  expect(api.queryStates.value[id].gridEdits?.[0].name.value).toBe('待保存')
  expect(api.queryStates.value[other].gridMessage).toBeUndefined()
})
