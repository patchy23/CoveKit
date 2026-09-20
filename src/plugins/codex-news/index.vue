<script setup lang="ts">
/** 单栏消息页：原文展示、按行展开、未读筛选；无通知和账户操作。 */
import { computed, ref } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  UiBadge,
  UiButton,
  UiEmptyState,
  UiRadioGroup,
  UiScrollArea,
  UiSwitch,
  UiTable,
  UiTableCell,
  UiTableExpandableRow,
} from '@/core/ui'
import type { NewsEvent } from './contracts'
import { localTime, safeUrl, statusLabel } from './news'
import { useNews } from './useNews'

const news = useNews()
const { snapshot, pendingCount, auto, checkedAt, error, saveError, loading, ready, stale } = news
const filter = ref('all'),
  expanded = ref(''),
  linkError = ref('')
const unreadCount = computed(() => snapshot.value?.events.filter(news.unread).length ?? 0)
const options = computed(() => [
  { value: 'all', label: '全部' },
  { value: 'unread', label: `未读 ${unreadCount.value}` },
])
const events = computed(() =>
  (snapshot.value?.events ?? []).filter(
    (item) => filter.value === 'all' || news.unread(item) || expanded.value === item.id
  )
)
function expand(item: NewsEvent, value: boolean) {
  expanded.value = value ? item.id : ''
  if (value) news.markRead(item)
}
function original(item: NewsEvent) {
  return item.sources.find((url) =>
    ['x.com', 'twitter.com', 'www.x.com', 'www.twitter.com'].includes(new URL(url).hostname)
  )
}
function hostname(url: string) {
  return new URL(url).hostname
}
async function open(url: string) {
  linkError.value = ''
  if (!safeUrl(url)) {
    linkError.value = '来源链接无效'
    return
  }
  try {
    await openUrl(url)
  } catch (e) {
    linkError.value = `打开链接失败：${String(e)}`
  }
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex flex-wrap items-center justify-between gap-sm border-b border-border px-md py-sm dark:border-border-dark"
    >
      <h2 class="text-h2 font-semibold text-primary dark:text-primary-dark">Codex 重置消息</h2>
      <div class="flex items-center gap-md">
        <UiSwitch
          :model-value="auto"
          label="自动更新"
          title="页面可见时每 5 分钟检查一次"
          size="sm"
          :disabled="!ready"
          @update:model-value="news.setAuto"
        />
        <UiButton
          size="sm"
          variant="secondary"
          :loading="loading"
          :disabled="!ready"
          @click="news.refresh()"
          >刷新</UiButton
        >
      </div>
    </div>
    <div
      class="flex flex-wrap items-center gap-sm px-md py-sm text-caption text-text-muted dark:text-text-muted-dark"
    >
      <span>上次检查：{{ localTime(checkedAt) }}</span>
      <UiBadge v-if="snapshot" size="xs" :tone="statusLabel(snapshot.state).tone">{{
        statusLabel(snapshot.state).label
      }}</UiBadge>
      <span v-if="stale" class="text-warning-strong dark:text-warning-dark"
        >数据源已过期，当前显示历史信息</span
      >
      <span class="ml-auto">来源：SaveMeTibo · 原文保留</span>
    </div>
    <p
      v-if="snapshot?.summary"
      class="px-md pb-sm text-body-sm text-secondary dark:text-secondary-dark"
    >
      {{ snapshot.summary }}
    </p>
    <p
      v-if="error || saveError || linkError"
      role="alert"
      class="select-text px-md pb-sm text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ [error, saveError, linkError].filter(Boolean).join('；') }}
    </p>
    <div
      class="flex flex-wrap items-center justify-between gap-sm border-b border-border px-md pb-sm dark:border-border-dark"
    >
      <UiRadioGroup
        v-model="filter"
        :options="options"
        name="codex-news-filter"
        variant="chips"
        size="sm"
        aria-label="消息筛选"
      />
      <UiButton size="xs" variant="ghost" :disabled="!unreadCount" @click="news.markAllRead"
        >全部已读</UiButton
      >
    </div>
    <UiButton
      v-if="pendingCount"
      variant="ghost"
      size="sm"
      class="shrink-0"
      @click="news.applyPending"
      >有 {{ pendingCount }} 条新消息或更新，点击查看</UiButton
    >
    <UiScrollArea class="min-h-0 flex-1" :aria-busy="loading">
      <UiTable v-if="events.length" :framed="false" density="default">
        <tbody>
          <UiTableExpandableRow
            v-for="item in events"
            :key="item.id"
            :label="item.headline"
            :columns="2"
            :expanded="expanded === item.id"
            @update:expanded="expand(item, $event)"
          >
            <template #label>
              <div class="min-w-0 flex-1 py-xs font-sans">
                <UiButton
                  variant="ghost"
                  size="sm"
                  class="!h-auto max-w-full !justify-start !px-0 !py-0 text-left whitespace-normal"
                  :aria-expanded="expanded === item.id"
                  @click="expand(item, expanded !== item.id)"
                >
                  <span
                    v-if="news.unread(item)"
                    class="mr-sm h-1.5 w-1.5 shrink-0 rounded-full bg-tertiary-strong dark:bg-tertiary-dark"
                    aria-label="未读"
                  />
                  <span
                    class="line-clamp-2 text-body-sm font-medium text-primary dark:text-primary-dark"
                    >{{ item.headline }}</span
                  >
                </UiButton>
                <div
                  class="mt-xs flex flex-wrap items-center gap-sm text-caption text-text-muted dark:text-text-muted-dark"
                >
                  <UiBadge size="xs" :tone="statusLabel(item.state).tone">{{
                    statusLabel(item.state).label
                  }}</UiBadge>
                  <span>{{ localTime(item.updatedAt) }}</span>
                </div>
              </div>
            </template>
            <UiTableCell content="action" align="right" class="w-[100px] whitespace-nowrap">
              <UiButton
                v-if="original(item)"
                size="xs"
                variant="ghost"
                @click="open(original(item)!)"
                >查看原文</UiButton
              >
              <UiButton v-else-if="item.url" size="xs" variant="ghost" @click="open(item.url)"
                >查看来源</UiButton
              >
            </UiTableCell>
            <template #details>
              <div
                class="space-y-sm p-md font-sans text-body-sm text-secondary dark:text-secondary-dark"
              >
                <p class="select-text whitespace-pre-wrap break-words">{{ item.headline }}</p>
                <p class="text-caption text-text-muted dark:text-text-muted-dark">
                  首次发布：{{ localTime(item.publishedAt) }} · 最近更新：{{
                    localTime(item.updatedAt)
                  }}
                </p>
                <ol
                  v-if="item.updates.length > 1"
                  class="space-y-sm border-l border-border pl-md dark:border-border-dark"
                >
                  <li v-for="(update, index) in item.updates" :key="index">
                    <div class="mb-xs flex items-center gap-sm">
                      <UiBadge size="xs" :tone="statusLabel(update.state).tone">{{
                        statusLabel(update.state).label
                      }}</UiBadge
                      ><span class="text-caption">{{ localTime(update.at) }}</span>
                    </div>
                    <p class="select-text break-words">{{ update.headline }}</p>
                  </li>
                </ol>
                <div class="flex flex-wrap gap-sm">
                  <UiButton
                    v-for="(url, index) in item.sources"
                    :key="url"
                    variant="ghost"
                    size="xs"
                    @click="open(url)"
                    >来源 {{ index + 1 }} · {{ hostname(url) }}</UiButton
                  >
                  <UiButton v-if="item.url" variant="ghost" size="xs" @click="open(item.url)"
                    >消息详情</UiButton
                  >
                </div>
              </div>
            </template>
          </UiTableExpandableRow>
        </tbody>
      </UiTable>
      <UiEmptyState v-else-if="loading || !ready" compact title="正在获取消息…" />
      <UiEmptyState
        v-else-if="error"
        compact
        title="消息暂时无法加载"
        description="请使用顶部刷新按钮重试。"
      />
      <UiEmptyState v-else compact :title="filter === 'unread' ? '没有未读消息' : '暂无历史消息'" />
    </UiScrollArea>
  </div>
</template>
