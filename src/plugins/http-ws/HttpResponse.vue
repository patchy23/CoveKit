<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * HttpResponse · 响应查看区（Postman 式：元信息 + Pretty/Raw + 响应头分页签）
 */
import { computed, ref } from 'vue'
import hljs from 'highlight.js/lib/core'
import json from 'highlight.js/lib/languages/json'
import type { HttpResponseResult } from './contracts'
import { formatBytes, formatHeaders, looksLikeJson } from './useHttp'
import { UiBadge, UiTabs } from '@/core/ui'

hljs.registerLanguage('json', json)

const props = defineProps<{
  response: HttpResponseResult
  respondedAt: string
}>()

const viewTab = ref<'pretty' | 'raw' | 'headers'>('pretty')

const statusClass = computed(() => {
  const s = props.response.status
  if (s >= 200 && s < 300)
    return 'bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark'
  if (s >= 400)
    return 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
  return 'bg-warning-soft text-warning-strong dark:bg-warning-soft-dark dark:text-warning-dark'
})

const highlighted = computed(() => {
  const body = props.response.body ?? ''
  if (!looksLikeJson(body)) return ''
  try {
    return hljs.highlight(body, { language: 'json', ignoreIllegals: true }).value
  } catch {
    return ''
  }
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-[10px]">
    <!-- 元信息 -->
    <div class="flex flex-wrap items-center gap-[10px]">
      <UiBadge size="sm" :class="statusClass">
        {{ response.ok ? `HTTP ${response.status}` : response.statusText || '请求失败' }}
      </UiBadge>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ response.durationMs }} ms
      </span>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ formatBytes(response.bodySize) }}
      </span>
      <span v-if="respondedAt" class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ respondedAt }}
      </span>
      <span
        v-if="response.error"
        class="text-body-sm text-tertiary-strong dark:text-tertiary-dark"
        >{{ response.error }}</span
      >
    </div>

    <!-- 响应查看分页签 -->
    <UiTabs
      v-model="viewTab"
      variant="line"
      size="sm"
      :items="[
        { value: 'pretty', label: 'Pretty' },
        { value: 'raw', label: 'Raw' },
        { value: 'headers', label: `响应头（${response.headers.length}）` },
      ]"
    />

    <!-- 响应体：Pretty（JSON 高亮）/ Raw（原样） -->
    <UiScrollArea v-if="viewTab !== 'headers'" as-child axis="both">
      <div
        class="min-h-0 flex-1 rounded-md border border-border bg-surface-muted font-mono text-body leading-relaxed dark:border-border-dark dark:bg-surface-muted-dark"
      >
        <pre
          v-if="viewTab === 'pretty' && highlighted"
          class="p-[13px]"
        ><code class="hljs" v-html="highlighted" /></pre>
        <pre v-else class="whitespace-pre-wrap p-[13px] text-primary dark:text-primary-dark">{{
          response.body || '(空响应体)'
        }}</pre>
      </div>
    </UiScrollArea>
    <!-- 响应头 -->
    <UiScrollArea v-else as-child axis="both">
      <pre
        class="min-h-0 flex-1 rounded-md border border-border bg-surface-muted p-[13px] font-mono text-body leading-relaxed text-secondary dark:border-border-dark dark:bg-surface-muted-dark dark:text-secondary-dark"
        >{{ formatHeaders(response.headers) }}</pre>
    </UiScrollArea>
  </div>
</template>
