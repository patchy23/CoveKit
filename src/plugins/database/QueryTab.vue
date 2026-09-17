<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
import { computed, onMounted, ref } from 'vue'
import { save as dialogSave } from '@tauri-apps/plugin-dialog'
import { UiButton, UiIcon, UiIconButton, UiInput, UiModal, UiSelect } from '@/core/ui'
import SqlEditor from './SqlEditor.vue'
import QueryResultPane from './QueryResultPane.vue'
import { useSplitPane } from './useSplitPane'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
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

/** 首次保存确认弹窗 */
const saveConfirmOpen = ref(false)
const saveTitle = ref('')

/** 编辑器方言与补全元数据（跟随当前连接） */
const editorDialect = computed(() => activeTabConnection.value?.dbType)
const editorTables = computed(() => db.completionTables.value)

const canExecute = computed(
  () => activeTabConnection.value?.status === 'online' && queryState.value.status !== 'running'
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
function runAll() {
  if (!canExecute.value) return
  const ed = editorRef.value
  void db.runQuery(ed ? ed.getDoc() : queryState.value.sql)
}

/** 保存：已保存直接更新；未保存弹窗确认别名 */
function onSave() {
  const state = queryState.value
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
type GridRow = { __row: string } & Record<string, string>
const gridRows = computed<GridRow[]>(() =>
  queryState.value.rows.map((row, index) => ({
    __row: String(index),
    ...Object.fromEntries(
      queryState.value.columns.map((_, colIndex) => [`c${colIndex}`, row[colIndex] ?? ''])
    ),
  }))
)

/** 复制结果到剪贴板（TSV 制表符分隔） */
async function copyResult() {
  const text = gridRows.value
    .map((row) => queryState.value.columns.map((_, i) => String(row[`c${i}`] ?? '')).join('\t'))
    .join('\n')
  if (text) {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
  }
}

/** 导出 CSV（对话框选路径 → dbc_export_csv 落盘） */
async function exportCsv() {
  const rows = gridRows.value.map((row) =>
    queryState.value.columns
      .map((_, i) => {
        const cell = String(row[`c${i}`] ?? '')
        return /[",\n]/.test(cell) ? `"${cell.replace(/"/g, '""')}"` : cell
      })
      .join(',')
  )
  const csv = [queryState.value.columns.join(','), ...rows].join('\n')
  const path = await dialogSave({
    defaultPath: 'result.csv',
    filters: [{ name: 'CSV', extensions: ['csv'] }],
  })
  if (path) {
    const { invokeCommand } = await import('@/core/ipc/ipc')
    await invokeCommand('dbc_export_csv', { path, text: csv })
  }
}
</script>

<template>
  <div
    ref="editorPaneRef"
    class="flex min-h-0 flex-col"
    :style="{ flex: `0 0 ${editorSplit.size.value}px` }"
  >
    <div
      class="flex h-[32px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
    >
      <UiIconButton
        :label="queryState.status === 'running' ? '运行中…' : '运行选中 / 光标所在语句'"
        size="sm"
        :disabled="!canExecute"
        class="text-success-strong dark:text-success-dark"
        @click="runCurrent"
      >
        <UiIcon
          v-if="queryState.status === 'running'"
          name="loading"
          :size="16"
          :stroke-width="2.5"
          class="shrink-0 animate-spin"
        />
        <UiIcon v-else name="play" :size="16" class="shrink-0" />
      </UiIconButton>
      <UiIconButton
        label="停止"
        size="sm"
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
        size="sm"
        class="text-success-strong dark:text-success-dark"
        @click="runAll"
      >
        <UiIcon name="play-all" :size="16" class="shrink-0" />
      </UiIconButton>
      <UiIconButton label="格式化" size="sm" @click="db.onFormatSql">
        <UiIcon name="format" :size="16" class="shrink-0" />
      </UiIconButton>
      <UiIconButton
        :label="queryState.savedId ? '保存' : '保存（首次需确认别名）'"
        size="sm"
        class="text-success-strong dark:text-success-dark"
        @click="onSave"
      >
        <UiIcon name="save" :size="16" class="shrink-0" />
      </UiIconButton>

      <span class="mx-[4px] h-[14px] w-px bg-border dark:bg-border-dark" />

      <UiSelect
        :model-value="activeTabContext.connectionId"
        :options="db.connectionOptions.value"
        size="xs"
        class="w-[140px]"
        title="当前连接"
        @update:model-value="(v) => (activeTabContext.connectionId = String(v))"
      />
      <UiSelect
        v-if="db.databaseOptions.value.length"
        :model-value="activeTabContext.database"
        :options="db.databaseOptions.value"
        size="xs"
        class="w-[110px]"
        title="数据库"
        @update:model-value="(v) => (activeTabContext.database = String(v))"
      />
      <UiSelect
        v-if="db.schemaOptions.value.length"
        :model-value="activeTabContext.schema"
        :options="db.schemaOptions.value"
        size="xs"
        class="w-[90px]"
        title="Schema"
        @update:model-value="(v) => (activeTabContext.schema = String(v))"
      />
    </div>

    <SqlEditor
      ref="editorRef"
      :model-value="queryState.sql"
      :dialect="editorDialect"
      :tables="editorTables"
      :resolve-columns="db.resolveEditorColumns"
      :on-run-statement="(sql) => db.runQuery(sql)"
      :on-table-click="(t) => db.openStructureForTable(t)"
      class="min-h-0 flex-1"
      placeholder="-- 有选中执行选中段，否则执行光标所在语句"
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
        size="sm"
        placeholder="编辑器名称（别名）"
        @keydown.enter="confirmSave"
      />
    </div>
    <template #footer>
      <UiButton size="sm" variant="ghost" @click="saveConfirmOpen = false">取消</UiButton>
      <UiButton size="sm" variant="primary" @click="confirmSave">保存</UiButton>
    </template>
  </UiModal>
</template>
