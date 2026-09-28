<script setup lang="ts">
/**
 * 数据浏览页签：可暂存修改的表格浏览（后端分页）+ 刷新 + 查看结构
 */
import { computed, ref, watch } from 'vue'
import EditableResultGrid from './EditableResultGrid.vue'
import { hasGridChanges } from './workspace/useQueryWorkspace'
import TableTools from './TableTools.vue'
import {
  UiBadge,
  UiButton,
  UiEmptyState,
  UiIcon,
  UiIconButton,
  UiInput,
  UiSpinner,
  UiToolbar,
} from '@/core/ui'
import { resultPageAction, sliceResultPage } from './resultRows'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const state = computed(() => db.queryState.value)
const gridPage = computed(() => state.value.gridPage ?? 1)
const gridPageSize = computed(() => state.value.gridPageSize ?? 100)
const pageSizeDraft = ref('100')
const pageSizeError = ref('')
watch(gridPageSize, (size) => (pageSizeDraft.value = String(size)), { immediate: true })
const pagerDisabled = computed(
  () =>
    hasGridChanges(state.value) ||
    state.value.gridSaving ||
    state.value.loadingMore ||
    state.value.status === 'running'
)
const pageAction = computed(() =>
  resultPageAction(
    gridPage.value,
    gridPageSize.value,
    state.value.rows.length,
    !!state.value.hasMore
  )
)

function patchGridPage(value: Partial<typeof state.value>, tabId = db.activeTabId.value) {
  db.patchTabQueryState(tabId, value)
}

function applyPageSize() {
  if (pagerDisabled.value) return
  const size = Number(pageSizeDraft.value)
  if (!Number.isInteger(size) || size < 1 || size > 1000) {
    pageSizeError.value = '每页条数须为 1 到 1000 的整数'
    return
  }
  pageSizeError.value = ''
  patchGridPage({ gridPage: 1, gridPageSize: size })
}

function previousPage() {
  if (pagerDisabled.value || gridPage.value <= 1) return
  patchGridPage({ gridPage: gridPage.value - 1 })
}

async function nextPage() {
  const tabId = db.activeTabId.value
  const sourceState = state.value
  const page = sourceState.gridPage ?? 1
  const size = sourceState.gridPageSize ?? 100
  const action = resultPageAction(page, size, sourceState.rows.length, !!sourceState.hasMore)
  if (pagerDisabled.value || action === 'end') return
  const next = page + 1
  if (action === 'advance') {
    patchGridPage({ gridPage: next }, tabId)
    return
  }
  if (sourceState.loadLimit) return
  await db.loadMore(tabId)
  if (
    db.activeTabId.value !== tabId ||
    db.queryStates.value[tabId] !== sourceState ||
    hasGridChanges(sourceState)
  )
    return
  if (action === 'load-next' && sourceState.rows.length > page * size)
    patchGridPage({ gridPage: next }, tabId)
}

/** 动态行对象（列名为 c0/c1…） */
type GridRow = { __row: string } & Record<string, string | null>
const gridRows = computed<GridRow[]>(() => {
  const page = gridPage.value
  const size = gridPageSize.value
  const first = (page - 1) * size
  return sliceResultPage(state.value.rows, page, size).map((row, offset) => ({
    __row: String(first + offset),
    ...Object.fromEntries(
      state.value.columns.map((_, colIndex) => [
        `c${colIndex}`,
        state.value.values[first + offset]?.[colIndex]?.kind === 'null'
          ? null
          : (row[colIndex] ?? ''),
      ])
    ),
  }))
})

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
        {{ db.activeTabConnection.value?.label ?? '' }} · 已加载 {{ state.rows.length }} 行{{
          state.hasMore ? '，还有更多' : ''
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
    <template #trailing>
      <span role="status" class="text-caption text-text-muted dark:text-text-muted-dark">{{
        pageSizeError ||
        state.loadLimit ||
        state.loadMoreError ||
        (state.loadingMore
          ? '正在读取下一批…'
          : hasGridChanges(state)
            ? '请先保存或放弃修改，再翻页或调整每页条数'
            : `第 ${gridPage} 页 · 已加载 ${state.rows.length} 行${state.hasMore ? '，仍有后续' : ''}`)
      }}</span>
      <UiButton
        size="xs"
        variant="ghost"
        :disabled="pagerDisabled || gridPage <= 1"
        @click="previousPage"
        >上一页</UiButton
      >
      <UiInput
        v-model="pageSizeDraft"
        aria-label="每页条数"
        type="number"
        min="1"
        max="1000"
        class="w-[64px]"
        size="xs"
        :disabled="pagerDisabled"
        @keydown.enter.prevent="applyPageSize"
      />
      <UiButton size="xs" variant="ghost" :disabled="pagerDisabled" @click="applyPageSize"
        >应用</UiButton
      >
      <UiButton
        size="xs"
        variant="ghost"
        :disabled="
          pagerDisabled || pageAction === 'end' || (!!state.loadLimit && pageAction !== 'advance')
        "
        @click="nextPage"
      >
        {{
          state.loadingMore
            ? '读取中…'
            : state.loadMoreError
              ? '重试读取'
              : pageAction === 'fill-current'
                ? '读取当前页剩余数据'
                : pageAction === 'load-next'
                  ? '读取下一页数据'
                  : '下一页'
        }}
      </UiButton>
    </template>
  </UiToolbar>
</template>
