<script setup lang="ts">
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from 'vue'
import { UiButton, UiSpinner, UiTooltip } from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = withDefaults(
  defineProps<{
    db: ReturnType<typeof useDatabase>
    nodeId: string
    active: boolean
    mode?: 'inline' | 'progress'
  }>(),
  { mode: 'inline' }
)

const state = computed(() => props.db.redisKeyLoadState(props.nodeId))
const countText = computed(() => {
  const current = state.value
  if (!current) return ''
  const loaded = current.loaded.toLocaleString()
  return current.total == null ? loaded : `${loaded} / ${current.total.toLocaleString()}`
})
const countTitle = computed(() => {
  const current = state.value
  if (!current) return ''
  return current.total == null
    ? `已加载 ${current.loaded.toLocaleString()}，总数未知`
    : `已加载 ${current.loaded.toLocaleString()}，总数 ${current.total.toLocaleString()}`
})
const limitTitle = computed(() => {
  if (state.value?.limitReason === 'keys') return '键数量达到加载上限（50,000 个）'
  if (state.value?.limitReason === 'bytes') return '键名数据达到加载上限（32 MiB）'
  return '已达到加载上限'
})
const root = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | null = null
let autoRequestInFlight = false
let observerGeneration = 0

function canAutoLoad() {
  const current = state.value
  return (
    props.mode === 'inline' &&
    props.active &&
    props.db.redisAutoLoadEnabled.value &&
    !!current?.hasMore &&
    !current.loading &&
    !current.fetchingAll &&
    !current.error &&
    !current.limitReached &&
    !current.autoLoadBudgetReached &&
    !current.automaticPaused
  )
}

function clearObserver() {
  observerGeneration += 1
  observer?.disconnect()
  observer = null
}

function observeFooter() {
  clearObserver()
  if (!canAutoLoad() || !root.value || typeof IntersectionObserver === 'undefined') return
  const target = root.value
  const generation = observerGeneration
  observer = new IntersectionObserver(
    (entries) => {
      if (
        generation !== observerGeneration ||
        !entries.some((entry) => entry.target === target && entry.isIntersecting) ||
        !canAutoLoad() ||
        autoRequestInFlight
      )
        return
      autoRequestInFlight = true
      void props.db.autoLoadMoreRedisKeys(props.nodeId).finally(() => {
        autoRequestInFlight = false
      })
    },
    { root: null, rootMargin: '100px 0px' }
  )
  observer.observe(target)
}

watch(
  () => {
    const current = state.value
    return [
      props.active,
      props.mode,
      props.db.redisAutoLoadEnabled.value,
      current?.loaded,
      current?.hasMore,
      current?.loading,
      current?.fetchingAll,
      current?.error,
      current?.limitReached,
      current?.autoLoadBudgetReached,
      current?.automaticPaused,
    ]
  },
  observeFooter,
  { flush: 'post' }
)

onMounted(observeFooter)
onActivated(observeFooter)
onDeactivated(clearObserver)
onBeforeUnmount(clearObserver)
</script>

<template>
  <div
    v-if="state && (mode === 'inline' || state.fetchingAll)"
    ref="root"
    class="redis-key-list-footer"
    data-no-drag
    :role="mode === 'progress' ? 'status' : 'group'"
    :aria-label="mode === 'progress' ? `${state.database} 键加载进度` : 'Redis 键加载操作'"
    :class="
      mode === 'progress'
        ? 'flex h-[24px] min-w-0 shrink-0 items-center gap-[4px] border-t border-border px-[6px] text-caption text-secondary dark:border-border-dark dark:text-secondary-dark'
        : 'absolute inset-0 z-[1] flex min-w-0 items-center gap-[1px] px-[3px] text-caption text-secondary dark:text-secondary-dark'
    "
  >
    <span v-if="mode === 'progress'" class="min-w-0 shrink truncate" :aria-label="state.database">{{
      state.database
    }}</span>
    <UiTooltip :content="countTitle">
      <span
        class="min-w-[70px] flex-1 truncate whitespace-nowrap tabular-nums"
        :aria-label="countTitle"
        >{{ countText }}</span
      >
    </UiTooltip>
    <UiTooltip v-if="state.limitReached" :content="limitTitle">
      <span class="shrink-0 text-text-muted dark:text-text-muted-dark">已达上限</span>
    </UiTooltip>
    <template v-if="mode === 'progress' && state.fetchingAll">
      <UiButton
        size="xs"
        variant="ghost"
        class="shrink-0 !px-[3px]"
        @click.stop="db.stopRedisKeyLoad(nodeId)"
        >停止</UiButton
      >
    </template>
    <template v-else-if="mode === 'inline' && !state.fetchingAll">
      <span
        v-if="state.loading"
        role="status"
        aria-label="正在加载 Redis 键"
        class="inline-flex shrink-0 items-center"
      >
        <UiSpinner size="xs" label="" />
      </span>
      <template v-else-if="state.hasMore && !state.limitReached">
        <UiButton
          size="xs"
          variant="ghost"
          class="redis-load-action shrink-0"
          title="加载更多"
          aria-label="加载更多"
          @click.stop="db.loadMoreRedisKeys(nodeId)"
          >更多</UiButton
        >
        <UiButton
          size="xs"
          variant="ghost"
          class="redis-load-action shrink-0"
          title="获取全部"
          aria-label="获取全部"
          @click.stop="db.fetchAllRedisKeys(nodeId)"
          >全部</UiButton
        >
      </template>
    </template>
  </div>
</template>

<style scoped>
@layer components {
  .redis-key-list-footer :deep(.redis-load-action.ui-control-xs) {
    padding-inline: 2px !important;
  }
}
</style>
