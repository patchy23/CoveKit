<script setup lang="ts">
/** 每条消息独立拥有展开状态与格式化结果；移除消息时随组件回收。 */
import { computed, ref } from 'vue'
import { UiButton } from '@/core/ui'
import { useJsonPreview } from '@/core/format/useJsonPreview'
import type { StreamEntry } from './useRequestSession'

const props = defineProps<{ entry: StreamEntry; kind: 'sse' | 'ws' }>()
defineEmits<{ copy: [text: string]; fill: [text: string] }>()
const open = ref(false)
// 快照可能重建消息对象；正文未变化时不使格式化结果失效。
const content = computed(() => props.entry.content)
const {
  content: formatted,
  error: formatError,
  pending: formatting,
} = useJsonPreview(
  () => content.value,
  () => open.value
)
function onToggle(event: Event) {
  open.value = (event.currentTarget as HTMLDetailsElement).open
}
</script>
<template>
  <details class="border-t border-border dark:border-border-dark" @toggle="onToggle">
    <summary class="flex cursor-pointer items-center gap-[8px] px-[10px] py-[6px] text-body-sm">
      <span class="shrink-0 font-mono text-caption text-secondary dark:text-secondary-dark">{{
        new Date(entry.time).toLocaleTimeString()
      }}</span
      ><span class="max-w-[130px] truncate text-success-strong dark:text-success-dark">{{
        entry.kind
      }}</span
      ><span v-if="entry.eventId" class="max-w-[90px] truncate text-caption"
        >#{{ entry.eventId }}</span
      ><span class="min-w-0 flex-1 truncate font-mono">{{ entry.content }}</span>
    </summary>
    <div v-if="open" class="px-[10px] pb-[8px]">
      <div class="mb-[4px] flex gap-[6px]">
        <UiButton size="xs" variant="ghost" @click="$emit('copy', entry.content)">复制内容</UiButton
        ><UiButton
          v-if="kind === 'ws'"
          size="xs"
          variant="ghost"
          @click="$emit('fill', entry.content)"
          >填入发送框</UiButton
        ><span
          v-if="entry.retry != null"
          class="text-caption text-secondary dark:text-secondary-dark"
          >retry: {{ entry.retry }} ms</span
        >
      </div>
      <pre class="select-text whitespace-pre-wrap break-all font-mono text-body-sm">{{
        formatted
      }}</pre>
      <span v-if="formatError || formatting" class="text-caption text-text-muted">{{
        formatError || '正在格式化…'
      }}</span>
    </div>
  </details>
</template>
