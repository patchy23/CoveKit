<script setup lang="ts">
/** SSE/WS 共用紧凑日志；清空仅影响视图，切换页签保留消息和待发送内容。 */
import { computed, nextTick, ref, watch } from 'vue'
import { UiButton, UiCheckbox, UiCodeEditor, UiInput, UiScrollArea, UiSelect } from '@/core/ui'
import { writeClipboardText } from '@/core/platform/clipboard'
import { useUiStore } from '@/stores/ui'
import type { RequestSession } from './useRequestSession'
const props = defineProps<{ kind: 'sse' | 'ws'; session: RequestSession; active: boolean }>()
defineEmits<{ save: [] }>()
const state = computed(() => props.session.state)
const search = ref(''),
  direction = ref('all'),
  follow = ref(true),
  showHeaders = ref(false)
const bottom = ref<HTMLElement | null>(null)
const ui = useUiStore()
const visible = computed(() =>
  state.value.entries.filter(
    (m) =>
      (direction.value === 'all' || m.direction === direction.value) &&
      `${m.kind} ${m.eventId} ${m.content}`.toLowerCase().includes(search.value.toLowerCase())
  )
)
watch(
  () => [state.value.entries.at(-1)?.seq, props.active],
  async () => {
    if (props.active && follow.value) {
      await nextTick()
      bottom.value?.scrollIntoView({ block: 'nearest' })
    }
  }
)
async function copy(text: string) {
  const result = await writeClipboardText(text)
  ui.toast(result.ok ? '已复制' : result.reason === 'empty' ? '暂无内容可复制' : '复制失败')
}
function pretty(text: string) {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}
</script>
<template>
  <section class="flex min-h-0 flex-1 flex-col">
    <div
      class="flex shrink-0 flex-wrap items-center gap-[8px] border-b border-border px-[10px] py-[6px] text-body-sm dark:border-border-dark"
    >
      <span class="font-medium">{{ kind === 'sse' ? '事件流' : '消息' }}</span
      ><span
        :class="
          state.connected
            ? 'text-success-strong dark:text-success-dark'
            : 'text-secondary dark:text-secondary-dark'
        "
        >{{
          state.busy
            ? '连接中…'
            : state.connected
              ? kind === 'sse'
                ? '订阅中'
                : '已连接'
              : '未连接'
        }}</span
      ><span v-if="state.streamStatus" class="font-mono">HTTP {{ state.streamStatus }}</span
      ><span class="text-text-muted">{{ state.entries.length }} 条</span
      ><span v-if="state.dropped" class="text-text-muted">已淘汰 {{ state.dropped }} 条</span
      ><UiButton
        v-if="kind === 'sse' && state.streamHeaders.length"
        size="xs"
        variant="ghost"
        @click="showHeaders = !showHeaders"
        >响应头</UiButton
      ><UiButton
        size="xs"
        variant="ghost"
        class="ml-auto"
        @click="copy(visible.map((m) => m.content).join('\n'))"
        >复制</UiButton
      >
    </div>
    <UiScrollArea v-if="showHeaders" as-child axis="both">
      <pre class="max-h-[100px] shrink-0 select-text p-[8px] font-mono text-body-sm">{{
        state.streamHeaders.map(([k, v]) => `${k}: ${v}`).join('\n')
      }}</pre>
    </UiScrollArea>
    <div class="flex shrink-0 flex-wrap items-center gap-[6px] px-[10px] py-[6px]">
      <UiInput
        v-model="search"
        size="xs"
        class="min-w-[100px] flex-1"
        aria-label="搜索消息"
        placeholder="搜索事件或消息"
      /><UiSelect
        v-if="kind === 'ws'"
        v-model="direction"
        size="xs"
        class="w-[100px]"
        title="消息方向"
        :options="[
          { value: 'all', label: '全部方向' },
          { value: 'sent', label: '仅发送' },
          { value: 'received', label: '仅接收' },
        ]"
      /><UiButton size="xs" variant="ghost" @click="session.clear()">清空</UiButton
      ><UiCheckbox v-model="follow" size="xs" label="跟随最新" />
    </div>
    <UiScrollArea class="min-h-0 flex-1" axis="vertical">
      <details
        v-for="entry in visible"
        :key="entry.seq"
        class="border-t border-border dark:border-border-dark"
      >
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
        <div class="px-[10px] pb-[8px]">
          <div class="mb-[4px] flex gap-[6px]">
            <UiButton size="xs" variant="ghost" @click="copy(entry.content)">复制内容</UiButton
            ><UiButton
              v-if="kind === 'ws'"
              size="xs"
              variant="ghost"
              @click="state.message = entry.content"
              >填入发送框</UiButton
            ><span
              v-if="entry.retry != null"
              class="text-caption text-secondary dark:text-secondary-dark"
              >retry: {{ entry.retry }} ms</span
            >
          </div>
          <pre class="select-text whitespace-pre-wrap break-all font-mono text-body-sm">{{
            pretty(entry.content)
          }}</pre>
        </div>
      </details>
      <p
        v-if="!visible.length"
        class="p-[16px] text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{
          state.entries.length
            ? '没有匹配的内容'
            : state.connected
              ? '等待服务端消息…'
              : kind === 'sse'
                ? '开始订阅后查看服务端事件。'
                : '连接后可收发消息。'
        }}
      </p>
      <div ref="bottom" />
    </UiScrollArea>
    <div
      v-if="kind === 'ws'"
      class="shrink-0 space-y-[6px] border-t border-border p-[8px] dark:border-border-dark"
    >
      <UiCodeEditor
        v-model="state.message"
        language="text"
        mode="minimal"
        height="80px"
        placeholder="文本或 JSON 消息，保留原始空白"
        @save="$emit('save')"
      />
      <div class="flex items-center justify-between">
        <span class="text-caption text-secondary dark:text-secondary-dark"
          >发送后保留内容，方便重复测试</span
        ><UiButton
          size="sm"
          variant="primary"
          :disabled="!state.connected || !state.message"
          :loading="state.sending"
          @click="session.send()"
          >发送消息</UiButton
        >
      </div>
    </div>
  </section>
</template>
