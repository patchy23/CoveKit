<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { useUiStore } from '@/stores/ui'
import { computed, onMounted, ref, watch } from 'vue'
import { save as dialogSave, open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { fileIpc, queryIpc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'
import { nextRequestId } from './requestId'
import { rowsToTsv } from './resultText'
import { splitSqlStatements } from './sqlStatementRanges'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiSelect,
  UiToolbar,
  UiContextMenu,
} from '@/core/ui'
import SqlEditor from './SqlEditor.vue'
import FixtureSql from './FixtureSql.vue'
import QueryResultPane from './QueryResultPane.vue'
import { useSplitPane } from '@/core/ui/useSplitPane'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const { copyText } = useCopy()
const ui = useUiStore()
const { queryState, patchQueryState, activeTabContext, activeTabConnection } = db

const editorRef = ref<InstanceType<typeof SqlEditor> | null>(null)

const editorPaneRef = ref<HTMLElement | null>(null)

/** 编辑器高度（px）：默认按容器 38%，可拖拽；不持久化（分隔条在面板下方，正向） */
const editorSplit = useSplitPane({ initial: 320, min: 120, max: 100000 }, true)

onMounted(() => {
  const parent = editorPaneRef.value?.parentElement
  if (parent) editorSplit.size.value = Math.round(parent.clientHeight * 0.38)
})

/** 拖拽上限：给结果区至少留 140px */
function editorMax(): number {
  const parent = editorPaneRef.value?.parentElement
  return (parent ? parent.clientHeight : 800) - 140
}

/** 窄工具栏内的低频操作收纳。 */
const moreMenu = ref<{ x: number; y: number } | null>(null)
const moreItems = computed(() => [
  { label: '格式化', onClick: db.onFormatSql },
  { label: '查看执行计划', onClick: explain },
  { label: '完整导出当前只读查询…', onClick: exportFull },
  { label: '保存 SQL', onClick: onSave },
  { label: '打开 SQL 文件…', onClick: openSqlFile },
  { label: '另存为 SQL 文件…', onClick: saveSqlFile },
])
const scopeChanging = ref(false)
async function changeConnection(value: string) {
  const connection = db.connections.value.find((item) => item.id === value)
  const tabId = db.activeTabId.value
  const context = activeTabContext.value
  if (
    !connection ||
    scopeChanging.value ||
    queryState.value.status === 'running' ||
    queryState.value.transactionActive
  )
    return
  scopeChanging.value = true
  try {
    await queryIpc.closeWorkspace(context.connectionId, tabId)
    if (db.activeTabId.value !== tabId) return
    Object.assign(context, { connectionId: value, database: connection.database, schema: '' })
  } catch (error) {
    db.showError(error)
  } finally {
    scopeChanging.value = false
  }
}
/** 首次保存确认弹窗。 */
const saveConfirmOpen = ref(false)
const saveTitle = ref('')

/** 编辑器方言与补全元数据（跟随当前连接） */
const editorDialect = computed(() => activeTabConnection.value?.dbType)
const editorTables = computed(() => db.completionTables.value)

const canExecute = computed(
  () =>
    !scopeChanging.value &&
    activeTabConnection.value?.status === 'online' &&
    queryState.value.status !== 'running'
)

const statusText = computed(() => {
  if (activeTabConnection.value?.status !== 'online') return '已断开'
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
      return `${queryState.value.total} 行 · ${queryState.value.durationMs} ms${queryState.value.truncated ? '（截断）' : ''}`
    default:
      return '就绪'
  }
})

/** 提取本次执行范围：有选区执行选中段；否则执行光标所在完整语句 */
function currentExecSql(): string {
  const ed = editorRef.value
  if (!ed) return queryState.value.sql
  return ed.getExecutableSql()
}

function runCurrent() {
  if (!canExecute.value) return
  void db.runQuery(currentExecSql())
}

/** 全部执行：执行整个编辑器内容 */
function explain() {
  if (!canExecute.value) return
  const kind = activeTabConnection.value?.dbType
  if (kind === 'redis' || kind === 'oracle') {
    db.showError('此驱动暂不提供执行计划预览')
    return
  }
  const sql = currentExecSql()
  if (!sql.trim()) return
  if (splitSqlStatements(sql, kind).length !== 1) {
    db.showError('执行计划一次只接受一条语句，请缩小选区')
    return
  }
  void db.runQuery((kind === 'sqlite' ? 'EXPLAIN QUERY PLAN ' : 'EXPLAIN ') + sql)
}
function runAll() {
  if (!canExecute.value) return
  const ed = editorRef.value
  void db.runQuery(ed ? ed.getDoc() : queryState.value.sql)
}

/** 保存：已保存直接更新；未保存弹窗确认别名 */
function onSave() {
  const state = queryState.value
  if (state.filePath) {
    const snapshot = state.sql
    void fileIpc
      .writeSql(state.filePath, snapshot)
      .then(() => {
        state.dirty = state.sql !== snapshot
        ui.toast('SQL 文件已保存')
      })
      .catch(db.showError)
    return
  }
  if (!state.sql.trim()) {
    db.showError('没有可保存的 SQL 内容')
    return
  }
  if (state.savedId) {
    void db.saveQueryToDisk().catch((err) => db.showError(err))
  } else {
    saveTitle.value = db.activeTab.value?.label ?? 'SQL编辑器'
    saveConfirmOpen.value = true
  }
}

function confirmSave() {
  saveConfirmOpen.value = false
  void db.saveQueryToDisk(saveTitle.value).catch((err) => db.showError(err))
}

/** 动态行对象（UiDataGrid 按 key 渲染；__row 作 row-key） */
type GridRow = { __row: string } & Record<string, string | null>
const gridRows = computed<GridRow[]>(() =>
  db.pageRows.value.map((row) => ({
    __row: String(queryState.value.rows.indexOf(row)),
    ...Object.fromEntries(
      queryState.value.columns.map((_, colIndex) => [
        `c${colIndex}`,
        queryState.value.values[queryState.value.rows.indexOf(row)]?.[colIndex]?.kind === 'null'
          ? null
          : (row[colIndex] ?? ''),
      ])
    ),
  }))
)

/** 复制结果到剪贴板（TSV 制表符分隔） */
async function copyResult() {
  const source = queryState.value
  const text = rowsToTsv(
    db.filteredRows.value.map(
      (row) =>
        source.values[source.rows.indexOf(row)] ?? row.map((value) => ({ kind: 'text', value }))
    )
  )
  await copyText(text, '已复制筛选结果')
}

/** 导出 CSV（对话框选路径 → dbc_export_csv 落盘） */
async function exportCsv() {
  try {
    const source = queryState.value
    const rows = db.filteredRows.value.map(
      (row) =>
        source.values[source.rows.indexOf(row)] ?? row.map((value) => ({ kind: 'text', value }))
    )
    const path = await dialogSave({
      defaultPath: 'result.csv',
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (path) {
      await fileIpc.exportRows(path, source.columns, rows)
      ui.toast(
        `已导出已加载结果中的 ${rows.length} 行${source.truncated ? '，原结果未完整' : ''}；NULL 编码为 \\N`
      )
    }
  } catch (err) {
    db.showError(err)
  }
}
const exportRequest = ref('')
const exportLifecycle = useToolLifecycle('database', { owner: 'database.export' })
watch(
  exportRequest,
  (value) => {
    exportLifecycle.running.value = !!value
  },
  { flush: 'sync' }
)
const exportCancelling = ref(false)
const exportTarget = ref('')
async function exportFull() {
  if (exportRequest.value) return
  const context = { ...activeTabContext.value }
  const sql = currentExecSql()
  if (!sql.trim()) {
    db.showError('请选择一条只读查询')
    return
  }
  try {
    const path = await dialogSave({
      defaultPath: 'full-result.csv',
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (!path) return
    exportRequest.value = nextRequestId('full-export')
    exportCancelling.value = false
    exportTarget.value = [context.database, context.schema].filter(Boolean).join(' / ')
    const count = await fileIpc.exportQuery(
      context.connectionId,
      { database: context.database, schema: context.schema },
      sql,
      path,
      exportRequest.value
    )
    ui.toast('完整导出 ' + count + ' 行；NULL 编码为 \\N')
  } catch (error) {
    db.showError(error)
  } finally {
    exportRequest.value = ''
    exportCancelling.value = false
  }
}
async function cancelExport() {
  if (!exportRequest.value || exportCancelling.value) return
  exportCancelling.value = true
  try {
    await queryIpc.cancel(exportRequest.value)
  } catch (error) {
    db.showError(error)
    exportCancelling.value = false
  }
}
async function openSqlFile() {
  try {
    const path = await dialogOpen({
      multiple: false,
      filters: [{ name: 'SQL', extensions: ['sql'] }],
    })
    if (typeof path !== 'string') return
    const sql = await fileIpc.readSql(path)
    db.openSqlEditorWithSql(
      activeTabContext.value.connectionId,
      sql,
      activeTabContext.value.database,
      activeTabContext.value.schema
    )
    patchQueryState({ filePath: path, dirty: false })
    db.renameActiveTab(path.split(/[\\/]/).pop() ?? 'SQL')
  } catch (error) {
    db.showError(error)
  }
}
async function saveSqlFile() {
  const state = queryState.value
  try {
    const path = await dialogSave({
      defaultPath: state.filePath ?? 'query.sql',
      filters: [{ name: 'SQL', extensions: ['sql'] }],
    })
    if (!path) return
    const snapshot = state.sql
    await fileIpc.writeSql(path, snapshot)
    state.filePath = path
    state.dirty = state.sql !== snapshot
    ui.toast('SQL 文件已保存')
  } catch (error) {
    db.showError(error)
  }
}
</script>

<template>
  <div
    ref="editorPaneRef"
    class="flex min-h-0 flex-col"
    :style="{ flex: `0 0 ${editorSplit.size.value}px` }"
  >
    <UiToolbar density="compact" bordered class="@container/querybar">
      <UiIconButton
        :label="queryState.status === 'running' ? '运行中…' : '运行选中 / 光标所在语句'"
        size="xs"
        :disabled="!canExecute"
        class="text-success-strong dark:text-success-dark"
        @click="runCurrent"
      >
        <UiIcon
          v-if="queryState.status === 'running'"
          name="loading"
          :size="14"
          :stroke-width="2.5"
          class="shrink-0 animate-spin"
        />
        <UiIcon v-else name="play" :size="14" class="shrink-0" />
      </UiIconButton>
      <UiIconButton
        label="停止"
        size="xs"
        :disabled="queryState.status !== 'running'"
        class="text-danger-strong dark:text-danger-dark"
        @click="db.cancelQuery"
      >
        <!-- 定稿图形：12x12 居中描边方块（lucide Square 为 18x18，比例不同，保持自定义） -->
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
          style="width: 16px; height: 16px; flex: none"
        >
          <rect x="6" y="6" width="12" height="12" rx="2" />
        </svg>
      </UiIconButton>
      <UiIconButton
        label="全部执行"
        :disabled="!canExecute"
        size="xs"
        class="text-success-strong dark:text-success-dark"
        @click="runAll"
      >
        <UiIcon name="play-all" :size="14" class="shrink-0" />
      </UiIconButton>
      <UiIconButton
        label="格式化"
        size="xs"
        class="hidden @[560px]/querybar:inline-flex"
        @click="db.onFormatSql"
      >
        <UiIcon name="format" :size="14" class="shrink-0" />
      </UiIconButton>
      <UiIconButton
        :label="queryState.savedId ? '保存' : '保存（首次需确认别名）'"
        size="xs"
        class="hidden text-success-strong dark:text-success-dark @[560px]/querybar:inline-flex"
        @click="onSave"
      >
        <UiIcon name="save" :size="14" class="shrink-0" />
      </UiIconButton>

      <FixtureSql :db="db" />
      <UiIconButton
        label="更多查询操作"
        size="xs"
        @click="(event) => (moreMenu = { x: event.clientX, y: event.clientY + 4 })"
      >
        <UiIcon name="chevron-down" :size="12" />
      </UiIconButton>
      <UiButton
        v-if="
          ['mysql', 'polardb', 'postgresql', 'sqlite'].includes(
            activeTabConnection?.dbType ?? ''
          ) && !queryState.transactionActive
        "
        size="xs"
        variant="ghost"
        :disabled="!canExecute"
        @click="db.runQuery('BEGIN')"
        >事务</UiButton
      >
      <template v-if="queryState.transactionActive">
        <UiButton size="xs" variant="ghost" :disabled="!canExecute" @click="db.runQuery('COMMIT')"
          >提交</UiButton
        >
        <UiButton size="xs" variant="ghost" :disabled="!canExecute" @click="db.runQuery('ROLLBACK')"
          >回滚</UiButton
        >
      </template>
      <span class="mx-[4px] h-[14px] w-px bg-border dark:bg-border-dark" />

      <UiSelect
        :model-value="activeTabContext.connectionId"
        :options="db.connectionOptions.value"
        :disabled="scopeChanging || queryState.status === 'running' || queryState.transactionActive"
        size="xs"
        class="min-w-0 flex-1 basis-[140px] max-w-[180px]"
        title="当前连接"
        @update:model-value="changeConnection"
      />
      <UiSelect
        v-if="db.databaseOptions.value.length"
        :model-value="activeTabContext.database"
        :options="db.databaseOptions.value"
        :disabled="scopeChanging || queryState.status === 'running' || queryState.transactionActive"
        size="xs"
        class="min-w-0 flex-1 basis-[110px] max-w-[160px]"
        title="数据库"
        @update:model-value="(v) => (activeTabContext.database = String(v))"
      />
      <UiSelect
        v-if="db.schemaOptions.value.length"
        :model-value="activeTabContext.schema"
        :options="db.schemaOptions.value"
        :disabled="scopeChanging || queryState.status === 'running' || queryState.transactionActive"
        size="xs"
        class="min-w-0 flex-1 basis-[90px] max-w-[140px]"
        title="Schema"
        @update:model-value="(v) => (activeTabContext.schema = String(v))"
      />
    </UiToolbar>

    <SqlEditor
      ref="editorRef"
      :selection="queryState.selection"
      :document-key="db.activeTabId.value"
      :document-keys="db.tabs.value.filter((tab) => tab.kind === 'query').map((tab) => tab.id)"
      :model-value="queryState.sql"
      :dialect="editorDialect"
      :tables="editorTables"
      :resolve-columns="db.resolveEditorColumns"
      :on-run-statement="(sql) => db.runQuery(sql)"
      :on-table-click="(t) => db.openStructureForTable(t)"
      class="min-h-0 flex-1"
      placeholder="-- 有选中执行选中段，否则执行光标所在语句"
      @selection="(selection) => patchQueryState({ selection })"
      @save="onSave"
      @update:model-value="(v) => patchQueryState({ sql: v, dirty: true })"
    />
  </div>

  <UiTooltip content="拖拽调整编辑器高度">
    <div
      class="h-[5px] shrink-0 cursor-row-resize border-t border-border bg-surface-muted transition-colors hover:bg-tertiary/40 dark:border-border-dark dark:bg-surface-muted-dark"
      @mousedown="(e) => editorSplit.onPointerDown(e, editorMax)"
    />
  </UiTooltip>

  <QueryResultPane
    :db="db"
    :query-state="queryState"
    :rows="gridRows"
    :status-text="statusText"
    @copy="copyResult"
    @export="exportCsv"
    @patch="patchQueryState"
  />

  <UiContextMenu
    v-if="moreMenu"
    :x="moreMenu.x"
    :y="moreMenu.y"
    :items="moreItems"
    size="sm"
    @close="moreMenu = null"
  />

  <!-- 首次保存确认 -->
  <UiModal
    :open="saveConfirmOpen"
    title="保存 SQL 编辑器"
    size="sm"
    @close="saveConfirmOpen = false"
  >
    <div class="space-y-[8px]">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        首次保存需要确认名称，之后保存将直接更新该编辑器。
      </p>
      <UiInput
        v-model="saveTitle"
        size="xs"
        placeholder="编辑器名称（别名）"
        @keydown.enter="confirmSave"
      />
    </div>
    <template #footer>
      <UiButton size="xs" variant="ghost" @click="saveConfirmOpen = false">取消</UiButton>
      <UiButton size="xs" variant="primary" @click="confirmSave">保存</UiButton>
    </template>
  </UiModal>
  <UiModal
    :open="!!exportRequest"
    title="完整导出"
    :description="exportTarget"
    size="sm"
    @close="cancelExport"
  >
    <p class="text-body-sm">
      正在使用独立只读会话重新执行所选查询并流式写入 CSV，不受已加载结果行数限制。
    </p>
    <p class="mt-sm text-caption">
      {{
        exportCancelling
          ? '已请求取消，等待执行通道结束。'
          : '导出完成后替换目标文件；取消或失败不替换原文件。'
      }}
    </p>
    <template #footer
      ><UiButton size="xs" variant="secondary" :disabled="exportCancelling" @click="cancelExport"
        >取消导出</UiButton
      ></template
    >
  </UiModal>
</template>
