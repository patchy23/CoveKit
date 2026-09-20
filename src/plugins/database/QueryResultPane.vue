<script setup lang="ts">
import {
  UiAlert,
  UiButton,
  UiDataGrid,
  UiIcon,
  UiIconButton,
  UiInput,
  UiPagination,
  UiSpinner,
  UiSelect,
  UiTabs,
  UiToolbar,
} from '@/core/ui'
import { ref } from 'vue'
import CellValueDialog from './CellValueDialog.vue'
import type { DbValue } from './contracts'
import type { QueryState, useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
  queryState: QueryState
  rows: Array<{ __row: string } & Record<string, string | null>>
  statusText: string
}>()
const emit = defineEmits<{
  copy: []
  export: []
  patch: [value: Partial<QueryState>]
}>()
const cellDetail = ref<{ name: string; value: DbValue } | null>(null)
function showCell(event: { row: Record<string, unknown>; column: { key: string; label: string } }) {
  const index = Number(event.column.key.slice(1))
  const row = Number(event.row.__row)
  const value = props.queryState.values[row]?.[index] ?? {
    kind: 'text',
    value: String(event.row[event.column.key] ?? ''),
  }
  cellDetail.value = { name: event.column.label, value }
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col border-t border-border dark:border-border-dark">
    <UiToolbar density="compact" bordered>
      <UiSelect
        v-if="queryState.statements.length > 1"
        :model-value="`s${queryState.activeStatement}`"
        :options="
          queryState.statements.map((result, index) => ({
            value: `s${index}`,
            label: `语句 ${index + 1}${result.ok ? '' : ' · 失败'}`,
          }))
        "
        size="xs"
        class="w-[130px]"
        @update:model-value="db.selectStatement(Number(String($event).slice(1)))" />
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
      <p class="text-caption text-secondary dark:text-secondary-dark">
        {{ queryState.cancelRequested ? '已请求取消，等待数据库确认…' : '正在执行 SQL…' }}
      </p>
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
    <UiDataGrid
      v-else
      :model-value="queryState.selectedRow"
      class="min-h-0 flex-1"
      :columns="db.tableColumns.value"
      :rows="rows"
      row-key="__row"
      height="100%"
      @update:model-value="emit('patch', { selectedRow: String($event) })"
      @cell="showCell"
    >
      <template #empty>
        <span>{{ queryState.filter ? '无匹配结果' : '当前查询未返回数据' }}</span>
        <UiButton
          v-if="queryState.filter"
          size="xs"
          variant="ghost"
          @click="emit('patch', { filter: '', page: 1 })"
          >清除过滤</UiButton
        >
      </template>
    </UiDataGrid>
    <CellValueDialog :detail="cellDetail" @close="cellDetail = null" />
    <UiToolbar density="compact" class="border-t border-border px-[6px] dark:border-border-dark">
      <UiInput
        :model-value="queryState.filter"
        class="min-w-0 w-[160px]"
        size="xs"
        placeholder="筛选已加载结果…"
        @update:model-value="emit('patch', { filter: String($event), page: 1 })"
      />
      <span v-if="queryState.truncated" class="text-caption text-warning-strong"
        >结果未完整（达到行数或字节上限）</span
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
