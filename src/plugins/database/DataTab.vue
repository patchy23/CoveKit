<script setup lang="ts">
/**
 * 数据浏览页签：只读表格浏览（后端分页）+ 刷新 + 查看结构
 */
import { computed } from 'vue'
import {
  UiBadge,
  UiButton,
  UiDataGrid,
  UiEmptyState,
  UiIcon,
  UiIconButton,
  UiPagination,
} from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const state = computed(() => db.queryState.value)

/** 动态行对象（列名为 c0/c1…） */
type GridRow = { __row: string } & Record<string, string>
const gridRows = computed<GridRow[]>(() =>
  state.value.rows.map((row, index) => ({
    __row: String(index),
    ...Object.fromEntries(
      state.value.columns.map((_, colIndex) => [`c${colIndex}`, row[colIndex] ?? ''])
    ),
  }))
)

function refresh() {
  db.loadTableData(db.activeTabId.value)
}

function toStructure() {
  const tabId = db.activeTabId.value
  const table = tabId.replace(/^data-/, 'structure-')
  const ctx = db.tabContexts.value[tabId]
  if (!ctx) return
  if (!db.tabs.value.some((t) => t.id === table)) {
    db.tabs.value.push({
      id: table,
      label: `${table.split('structure-')[1]} · 结构`,
      kind: 'structure',
    })
    db.tabContexts.value[table] = { ...ctx }
  }
  db.activeTabId.value = table
  db.loadColumns(table)
}
</script>

<template>
  <div
    class="flex h-[32px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
  >
    <UiBadge tone="info" size="xs">只读浏览</UiBadge>
    <UiIconButton label="刷新" size="xs" @click="refresh">
      <UiIcon name="refresh" :size="12" />
    </UiIconButton>
    <UiIconButton label="查看结构" size="xs" @click="toStructure">
      <UiIcon name="grid" :size="12" />
    </UiIconButton>
    <span class="ml-auto font-mono text-caption text-secondary dark:text-secondary-dark">
      {{ db.activeTabConnection.value?.label ?? '' }} · 共 {{ state.total }} 行
    </span>
  </div>

  <UiEmptyState v-if="state.status === 'error'" :title="'加载失败'" :description="state.error">
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
    @update:model-value="(v) => db.patchQueryState({ selectedRow: String(v) })"
  />
  <div
    class="flex h-[28px] shrink-0 items-center justify-end gap-[8px] border-t border-border px-[8px] dark:border-border-dark"
  >
    <span class="mr-auto text-caption text-text-muted dark:text-text-muted-dark">
      {{ state.truncated ? '结果已截断' : `耗时 ${state.durationMs} ms` }}
    </span>
    <UiPagination
      :model-value="state.page"
      :total-pages="db.totalPages.value"
      size="xs"
      @update:model-value="db.setPage"
    />
  </div>
</template>
