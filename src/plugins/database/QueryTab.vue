<script setup lang="ts">
/**
 * SQL 编辑器页签：编辑器（工具栏 + 错误条 + 行号编辑器）与结果区（数据/消息/计划）
 * 执行语义：有选中文本执行选中段；无选中执行光标所在行；不提供全部执行（需要全量先全选）。
 * 保存语义：Ctrl+S 持久化（首次弹窗确认别名，已保存直接更新）；页签显示未保存/已保存状态。
 */
import { computed, ref } from 'vue'
import {
  LineNumberTextarea,
  UiAlert,
  UiButton,
  UiEmptyState,
  UiIconButton,
  UiInput,
  UiModal,
  UiPagination,
  UiSelect,
  UiSpinner,
  UiTabs,
  UiDataGrid,
} from '@/core/ui'
import { extractExecSql } from './useDatabase'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const { queryState, patchQueryState, activeTabContext, activeTabConnection } = db

/** 行号编辑器实例（读取选区/光标位置） */
const editorRef = ref<InstanceType<typeof LineNumberTextarea> | null>(null)

/** 首次保存确认弹窗 */
const saveConfirmOpen = ref(false)
const saveTitle = ref('')

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

/** 提取本次执行范围：选中文本 / 光标所在行 */
function currentExecSql(): string {
  const ta = editorRef.value?.textarea
  if (!ta) return queryState.value.sql
  return extractExecSql(ta.value, ta.selectionStart, ta.selectionEnd)
}

function runCurrent() {
  if (!canExecute.value) return
  void db.runQuery(currentExecSql())
}

function onEditorKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    runCurrent()
  } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
    event.preventDefault()
    onSave()
  } else if (event.key === 'Escape' && queryState.value.status === 'running') {
    void db.cancelQuery()
  }
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
    ...Object.fromEntries(queryState.value.columns.map((_, colIndex) => [`c${colIndex}`, row[colIndex] ?? ''])),
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
  const { save } = await import('@tauri-apps/plugin-dialog')
  const path = await save({
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
  <!-- SQL 编辑器（约 38% 高度） -->
  <div class="flex min-h-0 flex-col" style="flex: 0 0 38%">
    <div
      class="flex h-[32px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
    >
      <UiIconButton
        :label="
          queryState.status === 'running'
            ? '运行中…'
            : '运行选中 / 光标所在行（Ctrl+Enter）'
        "
        size="sm"
        :disabled="!canExecute"
        class="text-success-strong dark:text-success-dark"
        :class="queryState.status === 'running' ? '' : 'hover:!bg-success-soft disabled:opacity-40 dark:hover:!bg-success-soft-dark'"
        @click="runCurrent"
      >
        <svg
          v-if="queryState.status === 'running'"
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          class="animate-spin text-text-muted dark:text-text-muted-dark"
          aria-hidden="true"
        >
          <path d="M21 12a9 9 0 1 1-6.219-8.56" />
        </svg>
        <svg
          v-else
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="currentColor"
          aria-hidden="true"
        >
          <path d="M7.5 4.9v14.2c0 .9 1 1.5 1.8 1L20.5 13a1.16 1.16 0 0 0 0-2L9.3 3.9c-.8-.5-1.8.1-1.8 1Z" />
        </svg>
      </UiIconButton>
      <UiIconButton
        label="停止（Esc）"
        size="sm"
        :disabled="queryState.status !== 'running'"
        :class="
          queryState.status === 'running'
            ? 'text-danger-strong hover:!bg-danger-soft dark:text-danger-dark dark:hover:!bg-danger-soft-dark'
            : 'text-text-muted opacity-40 dark:text-text-muted-dark'
        "
        @click="db.cancelQuery"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <rect x="5" y="5" width="14" height="14" rx="2" />
        </svg>
      </UiIconButton>
      <UiIconButton label="执行计划" size="xs" @click="db.runExplain">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 2v6m0 8v6M2 12h6m8 0h6" />
        </svg>
      </UiIconButton>
      <UiIconButton label="格式化" size="xs" @click="db.onFormatSql">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M4 6h16M4 12h16M4 18h10" />
        </svg>
      </UiIconButton>
      <UiIconButton
        :label="queryState.savedId ? '保存（Ctrl+S）' : '保存（Ctrl+S，首次需确认）'"
        size="xs"
        class="text-success-strong dark:text-success-dark"
        @click="onSave"
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2zM17 21v-8H7v8M7 3v5h8" />
        </svg>
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

      <span
        class="ml-auto shrink-0 font-mono text-caption text-text-muted dark:text-text-muted-dark"
        :title="'有选中文本时执行选中段，否则执行光标所在行；需要全量执行请先全选（Ctrl+A）'"
      >
        Ctrl+Enter
      </span>
    </div>

    <UiAlert
      v-if="queryState.error && queryState.resultTab === 'message'"
      class="mx-[8px] mt-[6px]"
      tone="danger"
      title="查询失败"
      size="sm"
      >{{ queryState.error }}</UiAlert
    >

    <LineNumberTextarea
      ref="editorRef"
      :model-value="queryState.sql"
      class="min-h-0 flex-1 rounded-none border-0 font-mono"
      placeholder="-- 有选中执行选中段，否则执行光标所在行；Ctrl+S 保存"
      @update:model-value="(v) => patchQueryState({ sql: v, dirty: true })"
      @keydown="onEditorKeydown"
    />
  </div>

  <!-- 结果区 -->
  <div class="flex min-h-0 flex-1 flex-col border-t border-border dark:border-border-dark">
    <div
      class="flex h-[28px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
    >
      <UiTabs
        :model-value="queryState.resultTab"
        :items="db.resultTabs.value"
        variant="line"
        size="xs"
        @update:model-value="(v) => patchQueryState({ resultTab: String(v) })"
      />
      <span class="ml-auto text-caption text-text-muted dark:text-text-muted-dark">{{ statusText }}</span>
      <UiIconButton label="复制结果" size="xs" @click="copyResult">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="9" y="9" width="13" height="13" rx="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      </UiIconButton>
      <UiIconButton label="导出 CSV" size="xs" @click="exportCsv">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
        </svg>
      </UiIconButton>
    </div>

    <div
      v-if="queryState.status === 'running'"
      class="flex min-h-0 flex-1 flex-col items-center justify-center gap-[8px]"
    >
      <UiSpinner size="md" label="执行中" />
      <p class="text-caption text-secondary dark:text-secondary-dark">正在执行 SQL…</p>
    </div>

    <UiAlert
      v-else-if="queryState.resultTab === 'message'"
      class="m-[8px]"
      :tone="
        queryState.status === 'error'
          ? 'danger'
          : queryState.status === 'cancelled'
            ? 'warning'
            : queryState.status === 'idle'
              ? 'info'
              : 'success'
      "
      :title="
        queryState.status === 'error'
          ? '查询失败'
          : queryState.status === 'cancelled'
            ? '已取消'
            : queryState.status === 'idle'
              ? '没有可执行的 SQL'
              : '执行完成'
      "
      size="sm"
    >
      {{
        queryState.status === 'error'
          ? queryState.error
          : queryState.status === 'cancelled'
            ? '本次查询已停止。'
            : queryState.status === 'idle'
              ? '请选中一段文本，或将光标置于某一行的任意位置后重试。'
              : `返回 ${queryState.total} 行，耗时 ${queryState.durationMs} ms。`
      }}
    </UiAlert>

    <div v-else-if="queryState.resultTab === 'plan'" class="min-h-0 flex-1 overflow-auto p-[8px]">
      <pre class="font-mono text-caption leading-relaxed text-secondary dark:text-secondary-dark">
{{ queryState.plan.join('\n') || '（无执行计划输出）' }}</pre
      >
    </div>

    <UiEmptyState
      v-else-if="queryState.status === 'empty' || gridRows.length === 0"
      :title="queryState.filter ? '无匹配结果' : '暂无结果'"
      :description="queryState.filter ? '过滤后 0 条' : '当前查询未返回数据'"
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
      :columns="db.tableColumns.value"
      :rows="gridRows"
      row-key="__row"
      height="100%"
      @update:model-value="(v) => patchQueryState({ selectedRow: String(v) })"
    />
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
      <span v-if="queryState.truncated" class="text-caption text-warning-strong">结果已截断（仅显示前 1000 行）</span>
      <UiPagination
        :model-value="queryState.page"
        :total-pages="db.totalPages.value"
        size="xs"
        @update:model-value="db.setPage"
      />
    </div>
  </div>

  <!-- 首次保存确认 -->
  <UiModal :open="saveConfirmOpen" title="保存 SQL 编辑器" size="sm" @close="saveConfirmOpen = false">
    <div class="space-y-[8px]">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        首次保存需要确认名称，之后 Ctrl+S 将直接更新该编辑器。
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
