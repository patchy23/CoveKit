<script setup lang="ts">
/**
 * 数据浏览页签：只读表格浏览（后端分页）+ 刷新 + 查看结构
 */
import { computed, ref } from 'vue'
import CellValueDialog from './CellValueDialog.vue'
import type { DbValue } from './contracts'
import TableTools from './TableTools.vue'
import {
  UiBadge,
  UiButton,
  UiDataGrid,
  UiEmptyState,
  UiIcon,
  UiIconButton,
  UiPagination,
  UiSpinner,
  UiToolbar,
} from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const state = computed(() => db.queryState.value)

/** 动态行对象（列名为 c0/c1…） */
type GridRow = { __row: string } & Record<string, string | null>
const gridRows = computed<GridRow[]>(() =>
  state.value.rows.map((row, index) => ({
    __row: String(index),
    ...Object.fromEntries(
      state.value.columns.map((_, colIndex) => [
        `c${colIndex}`,
        state.value.values[index]?.[colIndex]?.kind === 'null' ? null : (row[colIndex] ?? ''),
      ])
    ),
  }))
)

function refresh() {
  db.loadTableData(db.activeTabId.value)
}

function toStructure() {
  const ctx = db.activeTabContext.value
  if (!ctx.table) return
  db.openStructureTab(ctx.connectionId, ctx.table, ctx.database, ctx.schema)
}
const cellDetail = ref<{ name: string; value: DbValue } | null>(null)
function showCell(event: { row: Record<string, unknown>; column: { key: string; label: string } }) {
  const index = Number(event.column.key.slice(1))
  const row = Number(event.row.__row)
  cellDetail.value = {
    name: event.column.label,
    value: state.value.values[row]?.[index] ?? {
      kind: 'text',
      value: String(event.row[event.column.key] ?? ''),
    },
  }
}
</script>

<template>
  <UiToolbar density="compact" bordered>
    <UiBadge tone="info" size="xs">{{
      db.activeTabConnection.value?.readonly ? '只读' : '表数据'
    }}</UiBadge>
    <UiIconButton label="刷新" size="xs" :disabled="state.status === 'running'" @click="refresh">
      <UiIcon name="refresh" :size="12" />
    </UiIconButton>
    <UiIconButton label="查看结构" size="xs" @click="toStructure">
      <UiIcon name="grid" :size="12" />
    </UiIconButton>
    <template #trailing
      ><span
        class="max-w-[240px] truncate font-mono text-caption text-secondary dark:text-secondary-dark"
      >
        {{ db.activeTabConnection.value?.label ?? '' }} · 当前页 {{ state.rows.length }} 行{{
          state.hasMore ? '，还有下一页' : ''
        }}
      </span></template
    >
  </UiToolbar>

  <TableTools :db="db" />
  <div
    v-if="state.status === 'running'"
    role="status"
    class="flex min-h-0 flex-1 items-center justify-center gap-[6px] text-caption text-secondary dark:text-secondary-dark"
  >
    <UiSpinner size="xs" />正在加载表数据…
  </div>
  <UiEmptyState v-else-if="state.status === 'error'" :title="'加载失败'" :description="state.error">
    <UiButton size="sm" variant="secondary" @click="refresh">重试</UiButton>
  </UiEmptyState>

  <UiDataGrid
    v-else
    :model-value="state.selectedRow"
    class="min-h-0 flex-1"
    :columns="db.tableColumns.value"
    :rows="gridRows"
    row-key="__row"
    height="100%"
    @cell="showCell"
    @update:model-value="(v) => db.patchQueryState({ selectedRow: String(v) })"
  />
  <UiToolbar density="compact" class="border-t border-border px-[6px] dark:border-border-dark">
    <span class="text-caption text-text-muted dark:text-text-muted-dark">
      {{
        state.status === 'running'
          ? '加载中…'
          : state.status === 'error'
            ? '加载失败'
            : `耗时 ${state.durationMs} ms`
      }}
    </span>
    <template #trailing
      ><UiPagination
        :model-value="state.page"
        :total-pages="db.totalPages.value"
        size="xs"
        @update:model-value="db.setPage"
    /></template>
  </UiToolbar>
  <CellValueDialog :detail="cellDetail" @close="cellDetail = null" />
</template>
