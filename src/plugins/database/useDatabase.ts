/**
 * database 数据层兼容门面（真实 IPC）
 *
 * 本文件只做两件事：
 * 1) 应用级协调：错误提示条（errorHint/showError）与卸载清理；
 * 2) 装配四个职责域（connection / workspace / catalog / library）并把它们的公开 API
 *    拼回既有返回面，现有组件与调用方的 import 路径与用法保持不变。
 *
 * 域实现：
 * - connection/useDatabaseConnections.ts：连接定义、连接/断开、当前选择与请求取消标记
 * - workspace/useQueryWorkspace.ts：页签、每页签 SQL/dirty/结果/请求状态、执行与结果投影
 * - catalog/useDatabaseCatalog.ts：库/schema/对象树缓存、加载状态、补全元数据与树上 DDL 命令
 * - library/useQueryLibrary.ts：历史/收藏的读写（不含查询执行生命周期）
 *
 * 搬迁的纯函数（filterTreeItems、extractExecSql、withTimeout）在此再导出，
 * 既有单测与组件的 import 路径不变。
 */
import { onBeforeUnmount, ref } from 'vue'
import { useDataRefresh } from '@/core/dataTransfer/useDataRefresh'
import { useDatabaseConnections } from './connection/useDatabaseConnections'
import { useQueryWorkspace } from './workspace/useQueryWorkspace'
import { useDatabaseCatalog } from './catalog/useDatabaseCatalog'
import { useQueryLibrary } from './library/useQueryLibrary'

export { filterTreeItems } from './catalog/useDatabaseCatalog'
export { extractExecSql } from './workspace/useQueryWorkspace'
export { withTimeout } from './connection/useDatabaseConnections'
export type { QueryState, TabContext } from './workspace/useQueryWorkspace'

type CatalogDomain = ReturnType<typeof useDatabaseCatalog>
type WorkspaceDomain = ReturnType<typeof useQueryWorkspace>

export function useDatabase() {
  // ── 应用级协调：错误提示条（各域经 showError 端口写入，4 秒后自动清除） ──
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

  // ── 职责域装配 ─────────────────────────────────────────────────────────
  // 连接域对 catalog/workspace 的端口延迟绑定：端口只在用户交互（连接成功、删除连接）
  // 时才被调用，届时下面两个域已构造完成 —— 既避开循环依赖，也不互传可写状态对象。
  let catalog: CatalogDomain | null = null
  let workspace: WorkspaceDomain | null = null

  const library = useQueryLibrary({ showError })
  const connection = useDatabaseConnections({
    prefetchCatalog: (connId) => catalog?.prefetchConnection(connId),
    invalidateCatalogMeta: (connId) => catalog?.invalidateConnectionMeta(connId),
    closeConnectionTabs: (connectionId) => workspace?.closeTabsForConnection(connectionId),
    showError,
  })
  useDataRefresh('database.', () =>
    Promise.all([connection.refreshConnections(), library.refreshSaved(), library.refreshHistory()])
  )
  const ws = useQueryWorkspace({
    connections: connection.connectionList,
    activeConnectionId: connection.activeConnectionIdView,
    activeConnection: connection.activeConnection,
    recordHistory: library.recordHistory,
    addSaved: library.addSaved,
    updateSaved: library.updateSaved,
    refreshSaved: library.refreshSaved,
    showError,
  })
  workspace = ws
  const cat = useDatabaseCatalog({
    connections: connection.connectionList,
    activeTabConnectionId: ws.activeTabConnectionId,
    activeTabSchema: ws.activeTabSchema,
    activeTabDatabase: ws.activeTabDatabase,
    activateConnection: connection.activateConnection,
    connect: connection.connect,
    setConnectError: connection.setConnectError,
    ipcScopeArg: ws.ipcScopeArg,
    openDataTab: ws.openDataTab,
    openRedisKeyTab: ws.openRedisKeyTab,
    openSqlEditorWithSql: ws.openSqlEditorWithSql,
    openCreateTableTab: ws.openCreateTableTabAt,
    hasTab: ws.hasTab,
    closeTab: ws.closeTab,
    showError,
  })
  catalog = cat

  return {
    // 连接（connection 域）
    connections: connection.connections,
    activeConnection: connection.activeConnection,
    activeConnectionId: connection.activeConnectionId,
    connecting: connection.connecting,
    connectError: connection.connectError,
    connectionOptions: connection.connectionOptions,
    connect: connection.connect,
    cancelConnect: connection.cancelConnect,
    disconnect: connection.disconnect,
    saveConnection: connection.saveConnection,
    removeConnection: connection.removeConnection,
    refreshConnections: connection.refreshConnections,
    // 元数据（catalog 域）
    databaseOptions: cat.databaseOptions,
    schemaOptions: cat.schemaOptions,
    rowLimitOptions: ws.rowLimitOptions,
    completionTables: cat.completionTables,
    resolveEditorColumns: cat.resolveEditorColumns,
    ensureMeta: cat.ensureMeta,
    showSystemSchemas: cat.showSystemSchemas,
    toggleSystemSchemas: cat.toggleSystemSchemas,
    // 页签（workspace 域，catalog 域补充树上入口）
    tabs: ws.tabs,
    activeTab: ws.activeTab,
    activeTabId: ws.activeTabId,
    activeTabKind: ws.activeTabKind,
    activeTabContext: ws.activeTabContext,
    activeTabConnection: ws.activeTabConnection,
    tabContexts: ws.tabContexts,
    openSqlEditor: ws.openSqlEditor,
    openSqlEditorWithSql: ws.openSqlEditorWithSql,
    openStructureTab: ws.openStructureTab,
    openStructureForTable: ws.openStructureForTable,
    openCreateTableEditor: cat.openCreateTableEditor,
    openCreateTableTab: cat.openCreateTableTab,
    createDatabaseFull: cat.createDatabaseFull,
    dropDatabase: cat.dropDatabase,
    tableAdminAction: cat.tableAdminAction,
    executeDdl: cat.executeDdl,
    refreshTreeNode: cat.refreshTreeNode,
    scopeContext: cat.scopeContext,
    parseLeafId: cat.parseLeafId,
    closeTab: ws.closeTab,
    renameActiveTab: ws.renameActiveTab,
    saveQueryToDisk: ws.saveQueryToDisk,
    // 查询（workspace 域）
    queryState: ws.queryState,
    queryStates: ws.queryStates,
    patchQueryState: ws.patchQueryState,
    runQuery: ws.runQuery,
    cancelQuery: ws.cancelQuery,
    onFormatSql: ws.onFormatSql,
    // 树（catalog 域）
    treeItems: cat.treeItems,
    visibleTreeItems: cat.visibleTreeItems,
    keyword: cat.keyword,
    selectedResource: cat.selectedResource,
    selectResource: cat.selectResource,
    toggleTree: cat.toggleTree,
    // 结果（workspace 域）
    pageRows: ws.pageRows,
    filteredRows: ws.filteredRows,
    totalPages: ws.totalPages,
    setPage: ws.setPage,
    resultTabs: ws.resultTabs,
    tableColumns: ws.tableColumns,
    structureColumns: ws.structureColumns,
    structureIndexes: ws.structureIndexes,
    structureDdl: ws.structureDdl,
    loadColumns: ws.loadColumns,
    loadStructureExtras: ws.loadStructureExtras,
    loadTableData: ws.loadTableData,
    // 历史/收藏（library 域读写；应用到编辑器是 workspace 域命令）
    history: library.history,
    savedSql: library.savedSql,
    refreshHistory: library.refreshHistory,
    refreshSaved: library.refreshSaved,
    applyHistory: ws.applyHistory,
    applySaved: ws.applySaved,
    removeSaved: library.removeSaved,
    clearHistory: library.clearHistory,
    // 通用
    errorHint,
    showError,
  }
}
