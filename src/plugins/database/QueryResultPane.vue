<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  UiAlert,
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiSpinner,
  UiSelect,
  UiTabs,
  UiToolbar,
} from '@/core/ui'
import EditableResultGrid from './EditableResultGrid.vue'
import type { QueryState, useDatabase } from './useDatabase'
import { hasGridChanges } from './workspace/useQueryWorkspace'
import { resultPageAction } from './resultRows'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
  queryState: QueryState
  rows: Array<{ __row: string } & Record<string, string | null>>
  statusText: string
}>()
const db = props.db
const queryState = computed(() => props.queryState)
const rows = computed(() => props.rows)
const statusText = computed(() => props.statusText)
const emit = defineEmits<{
  copy: []
  export: []
  patch: [value: Partial<QueryState>]
}>()
const gridPage = computed(() => queryState.value.gridPage ?? 1)
const gridPageSize = computed(() => queryState.value.gridPageSize ?? 100)
const pageDraft = ref(String(gridPage.value))
const pageError = ref('')
watch(gridPage, (page) => {
  pageDraft.value = String(page)
  pageError.value = ''
})
const loadedRows = computed(() => db.filteredRows.value.length)
const pagerDisabled = computed(
  () =>
    hasGridChanges(queryState.value) ||
    queryState.value.gridSaving ||
    queryState.value.loadingMore ||
    queryState.value.status === 'running' ||
    db.filteredRows.busy.value
)
const previousDisabled = computed(() => {
  const state = queryState.value
  if (pagerDisabled.value || gridPage.value <= 1) return true
  return (
    state.paginationMode === 'cursor' &&
    (gridPage.value - 2) * gridPageSize.value < (state.cursorBufferStart ?? 0)
  )
})
const firstDisabled = computed(
  () =>
    pagerDisabled.value ||
    gridPage.value <= 1 ||
    (queryState.value.paginationMode === 'cursor' && (queryState.value.cursorBufferStart ?? 0) > 0)
)
const lastDisabled = computed(() => {
  const state = queryState.value
  if (pagerDisabled.value || state.paginationMode === 'cursor') return true
  if (state.paginationMode === 'server' || state.paginationMode === 'table') return !state.hasMore
  return state.truncated || !!state.hasMore || loadedRows.value === 0
})
const pageAction = computed(() => {
  const state = queryState.value
  if (state.paginationMode === 'server') return state.hasMore ? 'load-next' : 'end'
  if (state.paginationMode === 'cursor') {
    const loadedEnd = (state.cursorBufferStart ?? 0) + state.rows.length
    const pageEnd = gridPage.value * gridPageSize.value
    if (loadedEnd > pageEnd) return 'advance'
    return state.hasMore ? 'load-next' : 'end'
  }
  return resultPageAction(gridPage.value, gridPageSize.value, loadedRows.value, !!state.hasMore)
})
const pageSizeOptions = [100, 200, 500, 1000].map((size) => ({
  value: String(size),
  label: String(size),
}))
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
  const value = Number(page)
  if (value === gridPage.value) return
  const sourceState = queryState.value
  if (!sourceState.paginationMode) {
    const pageCount = Math.max(1, Math.ceil(loadedRows.value / gridPageSize.value))
    if (value > pageCount) {
      pageError.value = `页码超出已加载结果范围，共 ${pageCount} 页。`
      return
    }
    emit('patch', { gridPage: value })
    return
  }
  const tabId =
    Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
  if (!tabId) return
  const loaded = await db.jumpToPage(tabId, value, gridPageSize.value)
  if (
    !loaded &&
    db.queryStates.value[tabId] === sourceState &&
    db.activeTabId.value === tabId &&
    queryState.value === sourceState &&
    !sourceState.loadMoreError
  )
    pageError.value = '无法读取目标页，当前页已保留。'
}

function firstPage() {
  if (firstDisabled.value) return
  const sourceState = queryState.value
  if (!sourceState.paginationMode) {
    emit('patch', { gridPage: 1 })
    return
  }
  const tabId =
    Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
  if (tabId) void db.goToPage(tabId, 1)
}

async function lastPage() {
  if (lastDisabled.value) return
  const sourceState = queryState.value
  if (!sourceState.paginationMode) {
    emit('patch', { gridPage: Math.max(1, Math.ceil(loadedRows.value / gridPageSize.value)) })
    return
  }
  const tabId =
    Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
  if (!tabId) return
  const loaded = await db.goToLastPage(tabId, gridPageSize.value)
  if (
    !loaded &&
    db.queryStates.value[tabId] === sourceState &&
    db.activeTabId.value === tabId &&
    queryState.value === sourceState &&
    !sourceState.loadMoreError
  )
    pageError.value = '无法读取尾页，当前页已保留。'
}

function changePageSize(value: string) {
  if (pagerDisabled.value) return
  const size = Number(value)
  if (!pageSizeOptions.some((option) => Number(option.value) === size)) return
  const tabId =
    Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === queryState.value) ??
    ''
  if (queryState.value.paginationMode && tabId) {
    void db.goToPage(tabId, gridPage.value, size)
    return
  }
  const page = Math.min(gridPage.value, Math.max(1, Math.ceil(loadedRows.value / size)))
  emit('patch', { gridPage: page, gridPageSize: size })
}

function previousPage() {
  if (previousDisabled.value) return
  if (queryState.value.paginationMode) {
    const tabId =
      Object.keys(db.queryStates.value).find(
        (id) => db.queryStates.value[id] === queryState.value
      ) ?? ''
    if (tabId) void db.goToPage(tabId, gridPage.value - 1)
    return
  }
  emit('patch', { gridPage: gridPage.value - 1 })
}

async function nextPage() {
  const sourceState = queryState.value
  const page = sourceState.gridPage ?? 1
  const size = sourceState.gridPageSize ?? 100
  const action = pageAction.value
  if (pagerDisabled.value || action === 'end') return
  const next = page + 1
  if (sourceState.paginationMode && sourceState.paginationMode !== 'cursor') {
    const tabId =
      Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
    if (tabId) await db.goToPage(tabId, next, size)
    return
  }
  if (sourceState.paginationMode === 'cursor') {
    const tabId =
      Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
    if (tabId) await db.goToPage(tabId, next, size)
    return
  }
  if (action === 'advance') {
    emit('patch', { gridPage: next })
    return
  }
  const tabId =
    Object.keys(db.queryStates.value).find((id) => db.queryStates.value[id] === sourceState) ?? ''
  if (!tabId) return
  const filter = sourceState.filter
  await db.loadMore(tabId)
  if (
    db.queryStates.value[tabId] !== sourceState ||
    db.activeTabId.value !== tabId ||
    queryState.value !== sourceState ||
    sourceState.filter !== filter ||
    gridPage.value !== page ||
    gridPageSize.value !== size ||
    hasGridChanges(sourceState)
  )
    return
  if (action === 'load-next') {
    try {
      const filtered = await db.filteredRows.ready()
      if (
        filtered.length > page * size &&
        db.queryStates.value[tabId] === sourceState &&
        db.activeTabId.value === tabId &&
        queryState.value === sourceState &&
        sourceState.filter === filter &&
        gridPage.value === page &&
        gridPageSize.value === size &&
        !hasGridChanges(sourceState)
      )
        emit('patch', { gridPage: next })
    } catch (error) {
      if (
        db.queryStates.value[tabId] === sourceState &&
        db.activeTabId.value === tabId &&
        !(error instanceof Error && error.name === 'AbortError')
      )
        db.showError(error)
    }
  }
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
        :disabled="queryState.loadingMore"
        class="w-[130px]"
        @update:model-value="db.selectStatement(Number(String($event).slice(1)))" />
      <UiTabs
        :model-value="queryState.resultTab"
        :items="db.resultTabs.value"
        variant="line"
        size="xs"
        @update:model-value="emit('patch', { resultTab: String($event) })" />
      <template #trailing
        ><span
          v-if="db.filteredRows.busy.value"
          role="status"
          class="text-caption text-secondary dark:text-secondary-dark"
          >正在筛选…</span
        >
        <UiButton
          v-if="db.filteredRows.busy.value"
          variant="ghost"
          @click="
            emit('patch', {
              filter: '',
              page: 1,
              gridPage: queryState.paginationMode ? gridPage : 1,
            })
          "
          >取消筛选</UiButton
        >
        <span class="text-caption text-text-muted dark:text-text-muted-dark">{{ statusText }}</span>
        <UiIconButton
          label="复制筛选结果"
          size="xs"
          :disabled="
            (!db.filteredRows.busy.value && !db.filteredRows.value.length) ||
            queryState.status === 'running' ||
            !!db.filteredRows.error.value
          "
          @click="emit('copy')"
          ><UiIcon name="copy" :size="12"
        /></UiIconButton>
        <UiIconButton
          label="导出筛选结果 CSV"
          size="xs"
          :disabled="
            (!db.filteredRows.busy.value && !db.filteredRows.value.length) ||
            queryState.status === 'running' ||
            !!db.filteredRows.error.value
          "
          @click="emit('export')"
          ><UiIcon name="download" :size="12"
        /></UiIconButton> </template
    ></UiToolbar>
    <UiAlert
      v-if="db.filteredRows.error.value"
      tone="danger"
      title="筛选失败"
      size="sm"
      class="m-[8px]"
      >{{ db.filteredRows.error.value }}</UiAlert
    >
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
    <EditableResultGrid v-else :db="db" :state="queryState" class="min-h-0 flex-1" :rows="rows">
      <template #empty>
        <span>{{ queryState.filter ? '无匹配结果' : '当前查询未返回数据' }}</span>
        <UiButton
          v-if="queryState.filter"
          size="xs"
          variant="ghost"
          @click="emit('patch', { filter: '', page: 1, gridPage: 1 })"
          >清除过滤</UiButton
        >
      </template>
    </EditableResultGrid>
    <UiToolbar density="compact" class="border-t border-border px-[6px] dark:border-border-dark">
      <UiInput
        :model-value="queryState.filter"
        class="min-w-0 w-[160px]"
        size="xs"
        placeholder="筛选已加载结果…"
        @update:model-value="
          emit('patch', {
            filter: String($event),
            page: 1,
            gridPage: queryState.paginationMode ? gridPage : 1,
          })
        "
      />
      <span
        v-if="queryState.truncated && !queryState.hasMore"
        class="text-caption text-warning-strong"
        >结果未完整（达到行数或字节上限）</span
      >
      <template #trailing>
        <span role="status" class="text-caption text-text-muted dark:text-text-muted-dark">{{
          queryState.loadLimit ||
          queryState.loadMoreError ||
          (queryState.loadingMore
            ? '正在读取下一批…'
            : hasGridChanges(queryState)
              ? '请先保存或放弃修改，再翻页或调整每页条数'
              : queryState.paginationMode === 'cursor'
                ? `第 ${gridPage} 页 · 游标仅保留最近 5 页，不能统计或直达尾页`
                : `第 ${gridPage} 页 · 筛选后 ${loadedRows} 行${queryState.hasMore ? '，仍有后续' : ''}`)
        }}</span>
        <UiButton
          size="xs"
          variant="ghost"
          :disabled="firstDisabled"
          aria-label="首页"
          title="首页"
          @click="firstPage"
          >首页</UiButton
        >
        <UiButton size="xs" variant="ghost" :disabled="previousDisabled" @click="previousPage"
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
          :disabled="
            pagerDisabled ||
            pageAction === 'end' ||
            (!!queryState.loadLimit && pageAction !== 'advance')
          "
          @click="nextPage"
        >
          {{
            queryState.loadingMore
              ? '读取中…'
              : queryState.loadMoreError
                ? '重试读取'
                : pageAction === 'load-next'
                  ? '读取下一页数据'
                  : '下一页'
          }}
        </UiButton>
        <UiButton
          size="xs"
          variant="ghost"
          :disabled="lastDisabled"
          aria-label="尾页"
          title="统计总数并跳转到尾页"
          @click="lastPage"
          >尾页</UiButton
        >
        <UiButton
          v-if="queryState.loadingMore && queryState.paginationMode !== 'cursor'"
          size="xs"
          variant="secondary"
          @click="db.cancelQuery"
          >停止读取</UiButton
        >
      </template>
    </UiToolbar>
    <p v-if="pageError" role="alert" class="px-sm text-caption text-danger">{{ pageError }}</p>
  </div>
</template>
