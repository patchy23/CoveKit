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
  UiSelect,
  UiSpinner,
  UiToolbar,
} from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const state = computed(() => db.queryState.value)
const gridPage = computed(() => state.value.gridPage ?? 1)
const gridPageSize = computed(() => state.value.gridPageSize ?? 100)
const pageDraft = ref(String(gridPage.value))
const pageError = ref('')
watch(gridPage, (page) => {
  pageDraft.value = String(page)
  pageError.value = ''
})
const pageSizeOptions = [100, 200, 500, 1000].map((size) => ({
  value: String(size),
  label: String(size),
}))
const pagerDisabled = computed(
  () =>
    hasGridChanges(state.value) ||
    state.value.gridSaving ||
    state.value.loadingMore ||
    state.value.status === 'running'
)
const pageAction = computed(() => (state.value.hasMore ? 'load-next' : 'end'))

function changePageSize(value: string) {
  if (pagerDisabled.value) return
  const size = Number(value)
  if (!pageSizeOptions.some((option) => Number(option.value) === size)) return
  void db.goToPage(db.activeTabId.value, gridPage.value, size)
}

function previousPage() {
  if (pagerDisabled.value || gridPage.value <= 1) return
  void db.goToPage(db.activeTabId.value, gridPage.value - 1)
}

async function nextPage() {
  const tabId = db.activeTabId.value
  if (pagerDisabled.value || !state.value.hasMore) return
  await db.goToPage(tabId, gridPage.value + 1)
}

function updatePageDraft(value: string | number) {
  pageDraft.value = String(value)
  pageError.value = ''
}

async function jumpToPage() {
  if (pagerDisabled.value) return
  const raw = pageDraft.value.trim()
  if (!/^[0-9]+$/.test(raw)) {
    pageError.value = '请输入正整数页码。'
    return
  }
  const page = BigInt(raw)
  if (page < 1n || page > 0xffff_ffffn) {
    pageError.value = '页码需在 1 至 4294967295 之间。'
    return
  }
  const tabId = db.activeTabId.value
  if (Number(page) === gridPage.value) return
  const source = state.value
  const loaded = await db.jumpToPage(tabId, Number(page), gridPageSize.value)
  if (
    !loaded &&
    db.activeTabId.value === tabId &&
    db.queryStates.value[tabId] === source &&
    state.value === source &&
    !source.loadMoreError
  )
    pageError.value = '无法读取目标页，当前页已保留。'
}

function firstPage() {
  if (pagerDisabled.value || gridPage.value <= 1) return
  void db.goToPage(db.activeTabId.value, 1)
}

async function lastPage() {
  if (pagerDisabled.value || !state.value.hasMore) return
  const tabId = db.activeTabId.value
  const source = state.value
  const loaded = await db.goToLastPage(tabId, gridPageSize.value)
  if (
    !loaded &&
    db.activeTabId.value === tabId &&
    db.queryStates.value[tabId] === source &&
    state.value === source &&
    !source.loadMoreError
  )
    pageError.value = '无法读取尾页，当前页已保留。'
}

/** 动态行对象（列名为 c0/c1…） */
type GridRow = { __row: string; __label: string } & Record<string, string | null>
const gridRows = computed<GridRow[]>(() => {
  const page = gridPage.value
  const size = gridPageSize.value
  return state.value.rows.map((row, offset) => ({
    __row: String(offset),
    __label: String((page - 1) * size + offset + 1),
    ...Object.fromEntries(
      state.value.columns.map((_, colIndex) => [
        `c${colIndex}`,
        state.value.values[offset]?.[colIndex]?.kind === 'null' ? null : (row[colIndex] ?? ''),
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
      :disabled="
        state.status === 'running' || state.loadingMore || hasGridChanges(state) || state.gridSaving
      "
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

  <div :inert="hasGridChanges(state) || state.gridSaving || state.loadingMore">
    <TableTools :db="db" />
  </div>
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
        aria-label="首页"
        title="首页"
        @click="firstPage"
        >首页</UiButton
      >
      <UiButton
        size="xs"
        variant="ghost"
        :disabled="pagerDisabled || gridPage <= 1"
        @click="previousPage"
        >上一页</UiButton
      >
      <UiInput
        :model-value="pageDraft"
        type="number"
        min="1"
        max="4294967295"
        step="1"
        aria-label="跳转页码"
        title="输入页码并按 Enter 跳转"
        class="w-[68px]"
        size="xs"
        :invalid="!!pageError"
        :disabled="pagerDisabled"
        @update:model-value="updatePageDraft"
        @keydown.enter.prevent="jumpToPage"
      />
      <UiButton size="xs" variant="ghost" :disabled="pagerDisabled" @click="jumpToPage"
        >跳转</UiButton
      >
      <UiSelect
        :model-value="String(gridPageSize)"
        :options="pageSizeOptions"
        title="每页条数"
        aria-label="每页条数"
        class="w-[64px]"
        size="xs"
        :disabled="pagerDisabled"
        @update:model-value="changePageSize"
      />
      <UiButton
        size="xs"
        variant="ghost"
        :disabled="pagerDisabled || pageAction === 'end' || !!state.loadLimit"
        @click="nextPage"
      >
        {{
          state.loadingMore
            ? '读取中…'
            : state.loadMoreError
              ? '重试读取'
              : pageAction === 'load-next'
                ? '读取下一页数据'
                : '下一页'
        }}
      </UiButton>
      <UiButton
        size="xs"
        variant="ghost"
        :disabled="pagerDisabled || !state.hasMore"
        aria-label="尾页"
        title="统计总数并跳转到尾页"
        @click="lastPage"
        >尾页</UiButton
      >
      <UiButton v-if="state.loadingMore" size="xs" variant="secondary" @click="db.cancelQuery"
        >停止读取</UiButton
      >
    </template>
  </UiToolbar>
  <p v-if="pageError" role="alert" class="px-sm text-caption text-danger">{{ pageError }}</p>
</template>
