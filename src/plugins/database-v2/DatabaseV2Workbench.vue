<script setup lang="ts">
/**
 * database-v2 工作台主容器
 * 风格对齐 patchyBox 其他工具（http-ws / ssh）：
 *   - 顶层 flex 三栏，无卡片壳、无 UiStatusBar、无 UiSplitPane
 *   - 左栏 w-[200px] border-r：搜索 + 连接列表（带类型徽章 + 状态点）
 *   - 中栏 flex-1 flex-col：连接页签（UiTabs）→ SQL 编辑器 + 结果（上下 flex）
 *   - 右栏 w-[220px] border-l：摘要（仅在有活动连接时显示，可整栏收起）
 *   - 图标按钮内嵌 SVG；危险操作走 ContextMenu + ConfirmDialog
 */
import { computed, ref } from 'vue'
import {
  LineNumberTextarea,
  UiAlert,
  UiBadge,
  UiButton,
  UiDataGrid,
  UiEmptyState,
  UiIconButton,
  UiInput,
  UiModal,
  UiPagination,
  UiSearchInput,
  UiSelect,
  UiSpinner,
  UiTable,
  UiTableCell,
  UiTabs,
  UiTree,
} from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import type { UiTabItem } from '@/core/ui'
import {
  useDatabaseV2Mock,
  type V2Connection,
  type V2HistoryEntry,
  type V2SavedEntry,
} from './useDatabaseV2Mock'

const {
  connections,
  connectionOptions,
  tabs,
  activeTabId,
  activeTabKind,
  activeTabContext,
  activeTabConnection,
  tabContexts,
  createQuery,
  closeTab,
  openStructure,
  queryState,
  patchQueryState,
  runQuery,
  cancelQuery,
  keyword,
  visibleTreeItems,
  toggleTree,
  selectedResource,
  selectResource,
  filteredRows,
  pageRows,
  totalPages,
  setPage,
  tableColumns,
  structureColumns,
  history,
  savedSql,
  applyHistory,
  applySaved,
  saveCurrentSql,
  removeSaved,
  showConnectionDialog,
  newConnectionType,
  newConnectionName,
  addConnection,
  databaseOptions,
  schemaOptions,
  removeConnection,
} = useDatabaseV2Mock()

// ──────────────────────────────────────────────────────────────────────────
// 布局状态
// ──────────────────────────────────────────────────────────────────────────
const inspectorOpen = ref(true) // 右侧摘要栏开关

// ──────────────────────────────────────────────────────────────────────────
// 连接类型徽章配色（左栏树连接节点）
// ──────────────────────────────────────────────────────────────────────────
function connectionBadgeClass(connection: V2Connection): string {
  const map: Record<string, string> = {
    mysql: 'bg-[#00758f]/10 text-[#00758f] dark:bg-[#00758f]/20 dark:text-[#5fc4de]',
    postgresql: 'bg-[#336791]/10 text-[#336791] dark:bg-[#336791]/20 dark:text-[#8fb4d9]',
    sqlite: 'bg-[#003b57]/10 text-[#003b57] dark:bg-[#003b57]/20 dark:text-[#7cb3d4]',
    oracle: 'bg-[#c74634]/10 text-[#c74634] dark:bg-[#c74634]/20 dark:text-[#f0937e]',
    sqlserver: 'bg-[#cc2927]/10 text-[#cc2927] dark:bg-[#cc2927]/20 dark:text-[#f08585]',
    redis: 'bg-[#dc382d]/10 text-[#dc382d] dark:bg-[#dc382d]/20 dark:text-[#f1918a]',
    mongodb: 'bg-[#47a248]/10 text-[#47a248] dark:bg-[#47a248]/20 dark:text-[#8fce91]',
  }
  return (
    map[connection.type] ??
    'bg-surface-muted text-secondary dark:bg-surface-muted-dark dark:text-secondary-dark'
  )
}

// ──────────────────────────────────────────────────────────────────────────
// 连接右键菜单（含删除确认）
// ──────────────────────────────────────────────────────────────────────────
const menu = ref<{ x: number; y: number; connection: V2Connection } | null>(null)
const deleteTarget = ref<V2Connection | null>(null)

function openMenu(event: MouseEvent, connection: V2Connection) {
  event.preventDefault()
  menu.value = {
    x: Math.min(event.clientX, window.innerWidth - 158),
    y: Math.min(event.clientY, window.innerHeight - 180),
    connection,
  }
}

const menuItems = computed<ContextMenuItem[]>(() => {
  if (!menu.value) return []
  const connection = menu.value.connection
  return [
    { label: '新建查询', onClick: () => openQueryOn(connection.id) },
    { label: '编辑连接', onClick: () => showHint('编辑连接（模拟）') },
    { label: '复制连接', onClick: () => showHint('已复制（模拟）') },
    { label: '', separator: true },
    { label: '删除连接', danger: true, onClick: () => (deleteTarget.value = connection) },
  ]
})

function confirmDelete() {
  if (deleteTarget.value) {
    removeConnection(deleteTarget.value.id)
    showHint(`已删除「${deleteTarget.value.label}」`)
    deleteTarget.value = null
  }
}

// ──────────────────────────────────────────────────────────────────────────
// 工作区页签
// ──────────────────────────────────────────────────────────────────────────
const tabItems = computed<UiTabItem[]>(() =>
  tabs.value.map((tab) => ({
    value: tab.id,
    label: tab.label,
    closable: true,
    status:
      tabContexts.value[tab.id]?.connectionId &&
      connections.value.find((c) => c.id === tabContexts.value[tab.id]?.connectionId)?.status ===
        'online'
        ? ('success' as const)
        : ('danger' as const),
  }))
)

function openQueryOn(connectionId: string) {
  createQuery()
  if (tabs.value.length) {
    const newId = tabs.value[tabs.value.length - 1].id
    tabContexts.value[newId] = { ...tabContexts.value[newId], connectionId }
  }
}

/** 树节点点击：连接节点 → 新建查询；表节点 → 打开数据页签 */
function onTreeSelect(id: string) {
  const item = visibleTreeItems.value.find((node) => node.id === id)
  if (!item) return
  if (item.depth === 0) {
    openQueryOn(item.id)
  } else if (item.kind === 'table') {
    selectResource(item.id)
  }
}

/** 树右键：仅连接节点弹菜单 */
function onTreeContext(mouse: MouseEvent, item: { id: string; depth: number }) {
  if (item.depth === 0) {
    const connection = connections.value.find((c) => c.id === item.id)
    if (connection) openMenu(mouse, connection)
  }
}

// ──────────────────────────────────────────────────────────────────────────
// 编辑器 / 查询动作
// ──────────────────────────────────────────────────────────────────────────
const canExecute = computed(
  () => activeTabConnection.value.status === 'online' && queryState.value.status !== 'running'
)

function onRunQuery() {
  runQuery()
}

function onFormatSql() {
  const sql = queryState.value.sql
  patchQueryState({
    sql: sql
      .replace(/\s+/g, ' ')
      .replace(
        / (FROM|WHERE|ORDER BY|GROUP BY|LIMIT|JOIN|LEFT JOIN|RIGHT JOIN|INNER JOIN) /g,
        '\n$1 '
      )
      .trim(),
    dirty: true,
  })
  showHint('已格式化')
}

function onSaveSql() {
  saveCurrentSql('')
  showHint('已收藏')
}

function onEditorKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    if (canExecute.value) onRunQuery()
  } else if (event.key === 'Escape' && queryState.value.status === 'running') {
    cancelQuery()
  }
}

// ──────────────────────────────────────────────────────────────────────────
// 结果区动作
// ──────────────────────────────────────────────────────────────────────────
const resultTabs = computed<UiTabItem[]>(() => [
  {
    value: 'data',
    label: '数据',
    badge: queryState.value.status === 'success' ? filteredRows.value.length : undefined,
  },
  { value: 'message', label: '消息' },
  { value: 'plan', label: '计划' },
])

const statusText = computed(() => {
  if (activeTabConnection.value.status === 'offline') return '已断开'
  switch (queryState.value.status) {
    case 'running':
      return '执行中…'
    case 'error':
      return '失败'
    case 'cancelled':
      return '已取消'
    case 'empty':
      return '无结果'
    case 'success':
      return `${filteredRows.value.length} 行 · ${queryState.value.durationMs} ms`
    default:
      return '就绪'
  }
})

// ──────────────────────────────────────────────────────────────────────────
// 摘要面板（右栏）动作
// ──────────────────────────────────────────────────────────────────────────
const inspectorSection = ref<'overview' | 'history' | 'saved'>('overview')

function onApplyHistory(entry: V2HistoryEntry) {
  applyHistory(entry)
  showHint('已恢复到当前查询')
}

function onApplySaved(entry: V2SavedEntry) {
  applySaved(entry)
  showHint(`已加载「${entry.title}」`)
}

// ──────────────────────────────────────────────────────────────────────────
// 提示（替代 toast，2.4s 自动消失）
// ──────────────────────────────────────────────────────────────────────────
const hint = ref('')
let hintTimer: ReturnType<typeof setTimeout> | undefined
function showHint(text: string) {
  hint.value = text
  if (hintTimer) clearTimeout(hintTimer)
  hintTimer = setTimeout(() => (hint.value = ''), 2400)
}
</script>

<template>
  <div class="flex h-full min-h-0 w-full">
    <!-- ═══════════ 左栏：连接列表 ═══════════ -->
    <aside class="flex w-[200px] shrink-0 flex-col border-r border-border dark:border-border-dark">
      <!-- 左栏 header -->
      <div class="shrink-0 space-y-[8px] px-[10px] py-[10px]">
        <UiSearchInput v-model="keyword" size="sm" placeholder="搜索连接…" />
        <UiButton variant="secondary" size="sm" block @click="showConnectionDialog = true">
          + 新建连接
        </UiButton>
      </div>

      <!-- 对象树（连接 → 数据库 → schema → 表/视图/函数） -->
      <div class="min-h-0 flex-1 overflow-y-auto px-[4px] pb-[8px]">
        <UiTree
          v-model="selectedResource"
          :items="visibleTreeItems"
          :row-height="24"
          @update:model-value="onTreeSelect"
          @toggle="toggleTree"
          @context="onTreeContext"
        >
          <!-- 连接节点：类型徽章 + 状态点 -->
          <template #icon="{ item }">
            <span
              v-if="item.depth === 0"
              class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded font-mono text-[9px] font-bold"
              :class="
                connectionBadgeClass(connections.find((c) => c.id === item.id) ?? connections[0])
              "
              >{{
                (connections.find((c) => c.id === item.id) ?? connections[0]).type
                  .slice(0, 1)
                  .toUpperCase()
              }}</span
            >
            <span
              v-else-if="item.kind === 'table'"
              class="w-[16px] shrink-0 text-center text-caption text-text-muted dark:text-text-muted-dark"
              >▦</span
            >
            <span
              v-else-if="item.kind === 'view'"
              class="w-[16px] shrink-0 text-center text-caption text-text-muted dark:text-text-muted-dark"
              >◫</span
            >
            <span
              v-else
              class="w-[16px] shrink-0 text-center text-caption text-text-muted dark:text-text-muted-dark"
            >
              {{ item.kind === 'database' ? '◈' : item.kind === 'schema' ? '◇' : '▸' }}
            </span>
          </template>
          <template #suffix="{ item }">
            <span
              v-if="item.depth === 0"
              class="h-[6px] w-[6px] rounded-full"
              :class="
                item.badge === '断开'
                  ? 'bg-danger-strong dark:bg-danger-dark'
                  : 'bg-success-strong dark:bg-success-dark'
              "
            />
          </template>
        </UiTree>

        <p
          v-if="!visibleTreeItems.length"
          class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          {{ keyword ? '无匹配对象' : '暂无连接，点击上方新建' }}
        </p>
      </div>
    </aside>

    <!-- ═══════════ 中栏：查询工作区 ═══════════ -->
    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- 连接页签 -->
      <UiTabs
        v-if="tabItems.length"
        :model-value="activeTabId"
        :items="tabItems"
        variant="line"
        size="sm"
        @update:model-value="activeTabId = $event"
        @close="closeTab"
      />

      <!-- 空态 -->
      <div
        v-if="!tabs.length"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="mb-[8px] text-h2">从左侧选择一个连接开始</p>
          <p class="text-body-sm">或点击「+ 新建连接」添加数据库</p>
        </div>
      </div>

      <!-- 查询页签 -->
      <template v-else-if="activeTabKind === 'query'">
        <!-- SQL 编辑器（约 40% 高度） -->
        <div class="flex min-h-0 flex-col" style="flex: 0 0 38%">
          <!-- 编辑器工具栏 -->
          <div
            class="flex h-[32px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
          >
            <UiButton
              variant="primary"
              size="xs"
              :disabled="!canExecute"
              :loading="queryState.status === 'running'"
              @click="onRunQuery"
            >
              ▶ 运行
            </UiButton>
            <UiButton
              v-if="queryState.status === 'running'"
              variant="danger"
              size="xs"
              @click="cancelQuery"
              >■ 停止</UiButton
            >
            <UiIconButton label="执行计划" size="xs" @click="showHint('执行计划（模拟）')">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M12 2v6m0 8v6M2 12h6m8 0h6" />
              </svg>
            </UiIconButton>
            <UiIconButton label="格式化" size="xs" @click="onFormatSql">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M4 6h16M4 12h16M4 18h10" />
              </svg>
            </UiIconButton>
            <UiIconButton label="收藏 SQL" size="xs" @click="onSaveSql">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" />
              </svg>
            </UiIconButton>

            <span class="mx-[4px] h-[14px] w-px bg-border dark:bg-border-dark" />

            <!-- 上下文选择器 -->
            <UiSelect
              :model-value="activeTabContext.connectionId"
              :options="connectionOptions"
              size="xs"
              class="w-[140px]"
              title="当前连接"
              @update:model-value="
                (v) => {
                  tabContexts[activeTabId].connectionId = String(v)
                }
              "
            />
            <UiSelect
              :model-value="activeTabContext.database"
              :options="databaseOptions"
              size="xs"
              class="w-[110px]"
              title="数据库"
              @update:model-value="
                (v) => {
                  tabContexts[activeTabId].database = String(v)
                }
              "
            />
            <UiSelect
              :model-value="activeTabContext.schema"
              :options="schemaOptions"
              size="xs"
              class="w-[90px]"
              title="Schema"
              @update:model-value="
                (v) => {
                  tabContexts[activeTabId].schema = String(v)
                }
              "
            />

            <span class="ml-auto font-mono text-caption text-text-muted dark:text-text-muted-dark">
              Ctrl+Enter
            </span>
          </div>

          <!-- 错误提示 -->
          <UiAlert
            v-if="queryState.error"
            class="mx-[8px] mt-[6px]"
            tone="danger"
            title="查询失败"
            size="sm"
            >{{ queryState.error }}</UiAlert
          >

          <!-- SQL 编辑器 -->
          <LineNumberTextarea
            :model-value="queryState.sql"
            class="min-h-0 flex-1 rounded-none border-0 font-mono"
            @update:model-value="(v) => patchQueryState({ sql: v, dirty: true })"
            @keydown="onEditorKeydown"
          />
        </div>

        <!-- 结果区 -->
        <div class="flex min-h-0 flex-1 flex-col border-t border-border dark:border-border-dark">
          <!-- 结果工具栏 -->
          <div
            class="flex h-[28px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
          >
            <UiTabs
              :model-value="queryState.resultTab"
              :items="resultTabs"
              variant="line"
              size="xs"
              @update:model-value="(v) => patchQueryState({ resultTab: String(v) })"
            />
            <span class="ml-auto text-caption text-text-muted dark:text-text-muted-dark">{{
              statusText
            }}</span>
            <UiIconButton label="复制结果" size="xs" @click="showHint('已复制（模拟）')">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <rect x="9" y="9" width="13" height="13" rx="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            </UiIconButton>
            <UiIconButton label="导出" size="xs" @click="showHint('导出（模拟）')">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
              </svg>
            </UiIconButton>
            <UiIconButton
              :label="inspectorOpen ? '隐藏摘要' : '显示摘要'"
              size="xs"
              :variant="inspectorOpen ? 'secondary' : 'ghost'"
              @click="inspectorOpen = !inspectorOpen"
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <path d="M15 3v18" />
              </svg>
            </UiIconButton>
          </div>

          <!-- 结果主体 -->
          <div
            v-if="queryState.status === 'running'"
            class="flex min-h-0 flex-1 flex-col items-center justify-center gap-[8px]"
          >
            <UiSpinner size="md" label="执行中" />
            <p class="text-caption text-secondary dark:text-secondary-dark">已运行 0.7 秒</p>
          </div>

          <UiAlert
            v-else-if="queryState.resultTab === 'message'"
            class="m-[8px]"
            :tone="
              queryState.status === 'error'
                ? 'danger'
                : queryState.status === 'cancelled'
                  ? 'warning'
                  : 'success'
            "
            :title="
              queryState.status === 'error'
                ? '查询失败'
                : queryState.status === 'cancelled'
                  ? '已取消'
                  : '执行完成'
            "
            size="sm"
          >
            {{
              queryState.status === 'error'
                ? queryState.error
                : queryState.status === 'cancelled'
                  ? '本次查询已停止。'
                  : `返回 ${filteredRows.length} 行，耗时 ${queryState.durationMs} ms。`
            }}
          </UiAlert>

          <div
            v-else-if="queryState.resultTab === 'plan'"
            class="min-h-0 flex-1 overflow-auto p-[8px]"
          >
            <pre
              class="font-mono text-caption leading-relaxed text-secondary dark:text-secondary-dark"
            >
Index Scan using users_pkey
  Filter: status = 'active'
  Rows Removed by Filter: 12
  Execution Time: 36 ms</pre>
          </div>

          <UiEmptyState
            v-else-if="queryState.status === 'empty' || filteredRows.length === 0"
            :title="queryState.filter ? '无匹配结果' : '暂无结果'"
            :description="queryState.filter ? `过滤后 0 条` : '当前查询未返回数据'"
          >
            <UiButton
              v-if="queryState.filter"
              size="sm"
              variant="secondary"
              @click="patchQueryState({ filter: '', page: 1 })"
              >清除过滤</UiButton
            >
          </UiEmptyState>

          <UiDataGrid
            v-else
            :model-value="queryState.selectedRow"
            class="min-h-0 flex-1"
            :columns="tableColumns"
            :rows="pageRows"
            row-key="id"
            height="100%"
            @update:model-value="(v) => patchQueryState({ selectedRow: String(v) })"
          >
            <template #cell-status="{ value }">
              <UiBadge :tone="value === 'active' ? 'success' : 'warning'" size="xs">{{
                value
              }}</UiBadge>
            </template>
          </UiDataGrid>

          <!-- 分页栏 -->
          <div
            class="flex h-[28px] shrink-0 items-center justify-between gap-[8px] border-t border-border px-[8px] dark:border-border-dark"
          >
            <UiInput
              :model-value="queryState.filter"
              class="w-[160px]"
              size="xs"
              placeholder="过滤结果…"
              @update:model-value="(v) => patchQueryState({ filter: String(v), page: 1 })"
            />
            <UiPagination
              :model-value="queryState.page"
              :total-pages="totalPages"
              size="xs"
              @update:model-value="setPage"
            />
          </div>
        </div>
      </template>

      <!-- 数据浏览页签 -->
      <template v-else-if="activeTabKind === 'data'">
        <div
          class="flex h-[32px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
        >
          <UiBadge tone="info" size="xs">只读浏览</UiBadge>
          <UiIconButton label="刷新" size="xs" @click="showHint('刷新数据（模拟）')">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path d="M23 4v6h-6M1 20v-6h6" />
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
            </svg>
          </UiIconButton>
          <UiIconButton label="查看结构" size="xs" @click="openStructure">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
            </svg>
          </UiIconButton>
          <span class="ml-auto font-mono text-caption text-secondary dark:text-secondary-dark">
            {{ activeTabConnection.label }} · {{ activeTabContext.database }}.{{
              activeTabContext.schema
            }}.users
          </span>
        </div>
        <UiDataGrid
          :model-value="queryState.selectedRow"
          class="min-h-0 flex-1"
          :columns="tableColumns"
          :rows="pageRows"
          row-key="id"
          height="100%"
          @update:model-value="(v) => patchQueryState({ selectedRow: String(v) })"
        >
          <template #cell-status="{ value }">
            <UiBadge :tone="value === 'active' ? 'success' : 'warning'" size="xs">{{
              value
            }}</UiBadge>
          </template>
        </UiDataGrid>
        <div
          class="flex h-[28px] shrink-0 items-center justify-end border-t border-border px-[8px] dark:border-border-dark"
        >
          <UiPagination
            :model-value="queryState.page"
            :total-pages="totalPages"
            size="xs"
            @update:model-value="setPage"
          />
        </div>
      </template>

      <!-- 结构页签 -->
      <template v-else>
        <div class="min-h-0 flex-1 overflow-auto p-[12px]">
          <div class="mb-[12px] flex items-center justify-between">
            <div>
              <h2 class="text-card-title font-semibold text-primary dark:text-primary-dark">
                users · 表结构
              </h2>
              <p class="mt-[2px] font-mono text-caption text-secondary dark:text-secondary-dark">
                {{ activeTabConnection.label }} · {{ activeTabContext.database }}.{{
                  activeTabContext.schema
                }}.users
              </p>
            </div>
            <UiButton
              size="xs"
              variant="primary"
              @click="
                createQuery()
                patchQueryState({ sql: 'SELECT * FROM users LIMIT 100;' })
              "
              >生成查询</UiButton
            >
          </div>
          <UiTable density="compact" :hoverable="true" :striped="true">
            <thead>
              <tr>
                <UiTableCell as="th">字段</UiTableCell>
                <UiTableCell as="th">类型</UiTableCell>
                <UiTableCell as="th">可空</UiTableCell>
                <UiTableCell as="th">默认值</UiTableCell>
                <UiTableCell as="th">键</UiTableCell>
                <UiTableCell as="th">注释</UiTableCell>
              </tr>
            </thead>
            <tbody>
              <tr v-for="column in structureColumns" :key="column.name">
                <UiTableCell content="technical">{{ column.name }}</UiTableCell>
                <UiTableCell content="technical">{{ column.type }}</UiTableCell>
                <UiTableCell>{{ column.nullable }}</UiTableCell>
                <UiTableCell content="technical">{{ column.defaultValue }}</UiTableCell>
                <UiTableCell>
                  <UiBadge v-if="column.key !== '—'" tone="info" size="xs">{{
                    column.key
                  }}</UiBadge>
                  <span v-else class="text-text-muted">—</span>
                </UiTableCell>
                <UiTableCell>{{ column.comment ?? '' }}</UiTableCell>
              </tr>
            </tbody>
          </UiTable>
        </div>
      </template>
    </main>

    <!-- ═══════════ 右栏：摘要面板 ═══════════ -->
    <aside
      v-if="inspectorOpen && tabs.length"
      class="flex w-[220px] shrink-0 flex-col border-l border-border dark:border-border-dark"
    >
      <!-- 摘要 header -->
      <div
        class="flex h-[28px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
      >
        <span class="text-caption font-semibold text-primary dark:text-primary-dark">摘要</span>
        <UiBadge tone="neutral" size="xs">{{ activeTabConnection.type.toUpperCase() }}</UiBadge>
        <UiIconButton label="收起" size="xs" class="ml-auto" @click="inspectorOpen = false">
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </UiIconButton>
      </div>

      <!-- 区块切换 -->
      <UiTabs
        v-model="inspectorSection"
        :items="[
          { value: 'overview', label: '概览' },
          { value: 'history', label: '历史', badge: history.length },
          { value: 'saved', label: '收藏', badge: savedSql.length },
        ]"
        variant="line"
        size="xs"
        class="shrink-0 border-b border-border dark:border-border-dark"
      />

      <!-- 摘要内容 -->
      <div class="min-h-0 flex-1 overflow-auto p-[8px]">
        <!-- 概览 -->
        <div v-if="inspectorSection === 'overview'" class="space-y-[8px]">
          <section>
            <div class="mb-[4px] text-label-caps text-text-muted dark:text-text-muted-dark">
              连接
            </div>
            <div class="space-y-[3px] rounded border border-border p-[6px] dark:border-border-dark">
              <div class="truncate text-body-sm font-medium text-primary dark:text-primary-dark">
                {{ activeTabConnection.label }}
              </div>
              <div class="truncate font-mono text-caption text-secondary dark:text-secondary-dark">
                {{ activeTabConnection.host }}
              </div>
              <div class="flex items-center gap-[6px] text-caption">
                <span class="text-text-muted dark:text-text-muted-dark">版本</span>
                <span class="font-mono text-primary dark:text-primary-dark">{{
                  activeTabConnection.version
                }}</span>
                <span class="ml-auto text-text-muted dark:text-text-muted-dark"
                  >{{ activeTabConnection.latency }} ms</span
                >
              </div>
              <div class="flex items-center gap-[6px] text-caption">
                <span class="text-text-muted dark:text-text-muted-dark">权限</span>
                <UiBadge :tone="activeTabConnection.readonly ? 'warning' : 'success'" size="xs">
                  {{ activeTabConnection.readonly ? '只读' : '可写' }}
                </UiBadge>
              </div>
            </div>
          </section>

          <section>
            <div class="mb-[4px] text-label-caps text-text-muted dark:text-text-muted-dark">
              当前表
            </div>
            <div class="space-y-[3px] rounded border border-border p-[6px] dark:border-border-dark">
              <div class="font-mono text-body-sm font-medium text-primary dark:text-primary-dark">
                users
              </div>
              <div class="font-mono text-caption text-secondary dark:text-secondary-dark">
                {{ activeTabContext.database }}.{{ activeTabContext.schema }}.users
              </div>
              <div class="flex items-center gap-[6px] text-caption">
                <span class="text-text-muted dark:text-text-muted-dark">行数</span>
                <span class="font-mono text-primary dark:text-primary-dark">48,260</span>
                <span class="ml-auto text-text-muted dark:text-text-muted-dark">18.4 MB</span>
              </div>
            </div>
          </section>

          <section>
            <div class="mb-[4px] flex items-center justify-between">
              <span class="text-label-caps text-text-muted dark:text-text-muted-dark"
                >字段 · {{ structureColumns.length }}</span
              >
            </div>
            <div class="overflow-hidden rounded border border-border dark:border-border-dark">
              <UiTable density="compact" :framed="false">
                <tbody>
                  <tr v-for="column in structureColumns" :key="column.name">
                    <UiTableCell content="technical">{{ column.name }}</UiTableCell>
                    <UiTableCell content="technical">{{ column.type }}</UiTableCell>
                    <UiTableCell align="right">
                      <UiBadge v-if="column.key !== '—'" tone="info" size="xs">{{
                        column.key
                      }}</UiBadge>
                    </UiTableCell>
                  </tr>
                </tbody>
              </UiTable>
            </div>
          </section>
        </div>

        <!-- 历史 -->
        <div v-else-if="inspectorSection === 'history'" class="space-y-[2px]">
          <div
            v-for="entry in history"
            :key="entry.id"
            class="flex w-full cursor-pointer flex-col gap-[2px] rounded px-[6px] py-[4px] text-left hover:bg-border dark:hover:bg-border-dark"
            @click="onApplyHistory(entry)"
          >
            <div class="flex items-center gap-[4px]">
              <span
                class="h-[5px] w-[5px] rounded-full"
                :class="entry.status === 'success' ? 'bg-success-strong' : 'bg-danger-strong'"
              />
              <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
                entry.at
              }}</span>
              <span class="ml-auto font-mono text-caption text-text-muted dark:text-text-muted-dark"
                >{{ entry.durationMs }} ms</span
              >
            </div>
            <div class="line-clamp-2 font-mono text-caption text-primary dark:text-primary-dark">
              {{ entry.sql }}
            </div>
          </div>
        </div>

        <!-- 收藏 -->
        <div v-else class="space-y-[2px]">
          <div
            v-for="entry in savedSql"
            :key="entry.id"
            class="group flex items-start gap-[4px] rounded px-[6px] py-[4px] hover:bg-border dark:hover:bg-border-dark"
          >
            <div class="min-w-0 flex-1 cursor-pointer text-left" @click="onApplySaved(entry)">
              <div class="text-caption font-semibold text-primary dark:text-primary-dark">
                {{ entry.title }}
              </div>
              <div
                class="line-clamp-2 font-mono text-caption text-secondary dark:text-secondary-dark"
              >
                {{ entry.sql }}
              </div>
            </div>
            <UiIconButton
              label="删除"
              size="xs"
              class="shrink-0 opacity-0 group-hover:opacity-100"
              @click.stop="removeSaved(entry.id)"
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </UiIconButton>
          </div>
        </div>
      </div>
    </aside>

    <!-- ═══════════ 提示条 ═══════════ -->
    <div
      v-if="hint"
      class="pointer-events-none absolute bottom-[20px] left-1/2 -translate-x-1/2 rounded-md border border-border bg-surface px-[10px] py-[4px] text-caption text-primary shadow-md dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
    >
      {{ hint }}
    </div>

    <!-- ═══════════ 对话框 ═══════════ -->
    <UiModal
      :open="showConnectionDialog"
      title="新建连接"
      size="lg"
      @close="showConnectionDialog = false"
    >
      <div class="space-y-[12px]">
        <div>
          <label
            class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
            >连接名称</label
          >
          <UiInput
            :model-value="newConnectionName"
            size="sm"
            placeholder="例如：开发 · PG 主库"
            @update:model-value="(v) => (newConnectionName = String(v))"
          />
        </div>
        <div class="grid grid-cols-[1fr_96px] gap-[8px]">
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >主机</label
            >
            <UiInput size="sm" placeholder="127.0.0.1" />
          </div>
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >端口</label
            >
            <UiInput size="sm" placeholder="5432" />
          </div>
        </div>
        <div class="grid grid-cols-2 gap-[8px]">
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >用户名</label
            >
            <UiInput size="sm" placeholder="patchy" />
          </div>
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >密码</label
            >
            <UiInput size="sm" type="password" placeholder="••••••" />
          </div>
        </div>
        <div>
          <label
            class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
            >默认数据库</label
          >
          <UiInput size="sm" placeholder="patchybox" />
        </div>
        <div>
          <label
            class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
            >数据库类型</label
          >
          <div class="grid grid-cols-4 gap-[6px]">
            <UiButton
              v-for="item in [
                { value: 'mysql', label: 'MySQL' },
                { value: 'postgresql', label: 'PostgreSQL' },
                { value: 'sqlite', label: 'SQLite' },
                { value: 'oracle', label: 'Oracle' },
                { value: 'sqlserver', label: 'SQL Server' },
                { value: 'redis', label: 'Redis' },
                { value: 'mongodb', label: 'MongoDB' },
              ]"
              :key="item.value"
              size="xs"
              :variant="newConnectionType === item.value ? 'primary' : 'secondary'"
              block
              @click="newConnectionType = item.value as typeof newConnectionType"
            >
              {{ item.label }}
            </UiButton>
          </div>
        </div>
      </div>
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="showConnectionDialog = false">取消</UiButton>
        <UiButton size="sm" variant="primary" @click="addConnection">保存并连接</UiButton>
      </template>
    </UiModal>

    <!-- 删除确认 -->
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除连接"
      :message="`确定删除「${deleteTarget?.label ?? ''}」吗？该连接下的所有查询页签将被关闭。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="confirmDelete"
    />

    <!-- 右键菜单 -->
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
  </div>
</template>
