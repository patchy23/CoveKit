<script setup lang="ts">
/** 响应查看：状态码与失败上下文常显，格式化不改变原文。 */
import { computed, ref } from 'vue'
import { UiAlert, UiButton, UiCodeEditor, UiTabs } from '@/core/ui'
import { useJsonPreview } from '@/core/format/useJsonPreview'
import { writeClipboardText } from '@/core/platform/clipboard'
import { useUiStore } from '@/stores/ui'
import type { HttpResponseResult } from './contracts'
import { formatBytes, formatHeaders } from './useHttp'
const props = defineProps<{ response: HttpResponseResult; respondedAt: string }>()
defineEmits<{ save: [] }>()
const tab = ref('pretty')
const ui = useUiStore()
const {
  content: pretty,
  error: formatError,
  pending: formatting,
} = useJsonPreview(
  () => props.response.body,
  () => tab.value === 'pretty'
)
const content = computed(() =>
  tab.value === 'headers'
    ? formatHeaders(props.response.headers)
    : tab.value === 'pretty'
      ? pretty.value
      : props.response.body
)
async function copy() {
  const result = await writeClipboardText(content.value)
  ui.toast(result.ok ? '已复制响应' : result.reason === 'empty' ? '响应内容为空' : '复制失败')
}
</script>
<template>
  <section class="flex min-h-0 flex-1 flex-col">
    <div
      class="flex shrink-0 flex-wrap items-center gap-[10px] border-b border-border px-[10px] py-[6px] text-body-sm dark:border-border-dark"
    >
      <span class="font-medium">响应</span
      ><span
        class="font-mono"
        :class="
          response.ok
            ? 'text-success-strong dark:text-success-dark'
            : 'text-tertiary-strong dark:text-tertiary-dark'
        "
        >{{ response.status ? `${response.status} ${response.statusText}` : '请求失败' }}</span
      ><span class="text-secondary dark:text-secondary-dark">{{ response.durationMs }} ms</span
      ><span class="text-secondary dark:text-secondary-dark">{{
        formatBytes(response.bodySize)
      }}</span
      ><span class="text-caption text-text-muted">{{ respondedAt }}</span
      ><UiButton size="xs" variant="ghost" class="ml-auto" @click="copy">复制</UiButton>
    </div>
    <UiTabs
      v-model="tab"
      class="shrink-0 px-[8px]"
      variant="line"
      size="sm"
      :items="[
        { value: 'pretty', label: '格式化' },
        { value: 'raw', label: '原文' },
        { value: 'headers', label: '响应头', badge: response.headers.length },
      ]"
    />
    <UiCodeEditor
      :model-value="content"
      :language="tab === 'pretty' && pretty !== response.body ? 'json' : 'text'"
      readonly
      :lint="false"
      :completion="false"
      class="min-h-0 flex-1"
      :line-wrapping="true"
      @save="$emit('save')"
    />
    <UiAlert v-if="formatError" tone="danger">{{ formatError }}</UiAlert>
    <span v-else-if="formatting" class="text-caption text-text-muted">正在格式化…</span>
  </section>
</template>
