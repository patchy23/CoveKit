<script setup lang="ts">
/**
 * 数据浏览页签：可暂存修改的表格浏览（后端分页）+ 刷新 + 查看结构
 */
import { computed } from 'vue'
import EditableResultGrid from './EditableResultGrid.vue'
import { hasGridChanges } from './workspace/useQueryWorkspace'
import TableTools from './TableTools.vue'
import {
  UiBadge,
  UiButton,
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
</script>

<template>
  <UiToolbar density="compact" bordered>
    <UiBadge tone="info" size="xs">{{
      db.activeTabConnection.value?.readonly ? '只读' : '表数据'
    }}</UiBadge>
    <UiIconButton
      label="刷新"
      size="xs"
      :disabled="state.status === 'running' || hasGridChanges(state) || state.gridSaving"
      @click="refresh"
    >
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

  <div :inert="hasGridChanges(state) || state.gridSaving"><TableTools :db="db" /></div>
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

  <EditableResultGrid v-else :db="db" :state="state" class="min-h-0 flex-1" :rows="gridRows" />
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
</template>
