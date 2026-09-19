<script setup lang="ts">
import {
  UiAlert,
  UiButton,
  UiDataGrid,
  UiEmptyState,
  UiIcon,
  UiIconButton,
  UiInput,
  UiPagination,
  UiSpinner,
  UiTabs,
  UiToolbar,
} from '@/core/ui'
import type { QueryState, useDatabase } from './useDatabase'

defineProps<{
  db: ReturnType<typeof useDatabase>
  queryState: QueryState
  rows: Array<{ __row: string } & Record<string, string>>
  statusText: string
}>()
const emit = defineEmits<{
  copy: []
  export: []
  patch: [value: Partial<QueryState>]
}>()
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col border-t border-border dark:border-border-dark">
    <UiToolbar density="compact" bordered>
      <UiTabs
        :model-value="queryState.resultTab"
        :items="db.resultTabs.value"
        variant="line"
        size="xs"
        @update:model-value="emit('patch', { resultTab: String($event) })" />
      <template #trailing
        ><span class="text-caption text-text-muted dark:text-text-muted-dark">{{
          statusText
        }}</span>
        <UiIconButton
          label="复制筛选结果"
          size="xs"
          :disabled="!db.filteredRows.value.length || queryState.status === 'running'"
          @click="emit('copy')"
          ><UiIcon name="copy" :size="12"
        /></UiIconButton>
        <UiIconButton
          label="导出筛选结果 CSV"
          size="xs"
          :disabled="!db.filteredRows.value.length || queryState.status === 'running'"
          @click="emit('export')"
          ><UiIcon name="download" :size="12"
        /></UiIconButton> </template
    ></UiToolbar>
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
    <UiEmptyState
      v-else-if="queryState.status === 'empty' || rows.length === 0"
      :title="queryState.filter ? '无匹配结果' : '暂无结果'"
      :description="queryState.filter ? '过滤后 0 条' : '当前查询未返回数据'"
    >
      <UiButton
        v-if="queryState.filter"
        size="sm"
        variant="secondary"
        @click="emit('patch', { filter: '', page: 1 })"
        >清除过滤</UiButton
      >
    </UiEmptyState>
    <UiDataGrid
      v-else
      :model-value="queryState.selectedRow"
      class="min-h-0 flex-1"
      :columns="db.tableColumns.value"
      :rows="rows"
      row-key="__row"
      height="100%"
      @update:model-value="emit('patch', { selectedRow: String($event) })"
    />
    <UiToolbar density="compact" class="border-t border-border px-[6px] dark:border-border-dark">
      <UiInput
        :model-value="queryState.filter"
        class="min-w-0 w-[160px]"
        size="xs"
        placeholder="过滤结果…"
        @update:model-value="emit('patch', { filter: String($event), page: 1 })"
      />
      <span v-if="queryState.truncated" class="text-caption text-warning-strong"
        >结果已截断（仅显示前 1000 行）</span
      >
      <template #trailing
        ><UiPagination
          :model-value="queryState.page"
          :total-pages="db.totalPages.value"
          size="xs"
          @update:model-value="db.setPage"
      /></template>
    </UiToolbar>
  </div>
</template>
