<script setup lang="ts">
/**
 * WsMessageArea · WebSocket 消息收发区（消息流 + 输入行）
 */
import { nextTick, ref, watch } from 'vue'
import type { WsSession } from './contracts'

const props = defineProps<{
  session: WsSession | null
}>()

const emit = defineEmits<{
  (e: 'send', text: string): void
  (e: 'close'): void
}>()

const text = ref('')
const listEl = ref<HTMLElement | null>(null)

const connected = () => props.session?.open === true
const hasMessage = () => (props.session?.messages.length ?? 0) > 0

/** 新消息自动滚到底部 */
watch(
  () => props.session?.messages.length ?? 0,
  async () => {
    await nextTick()
    if (listEl.value) listEl.value.scrollTop = listEl.value.scrollHeight
  }
)

function send() {
  const t = text.value.trim()
  if (!t || !connected()) return
  emit('send', t)
  text.value = ''
}

function formatMsgTime(t: number) {
  return new Date(t).toLocaleTimeString('zh-CN', { hour12: false })
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-[8px]">
    <!-- 消息流 -->
    <div
      ref="listEl"
      class="min-h-0 flex-1 overflow-y-auto rounded-md border border-border bg-surface-muted p-[12px] dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <div v-if="hasMessage() && props.session" class="flex flex-col gap-[8px]">
        <div
          v-for="(m, i) in props.session.messages"
          :key="i"
          class="flex"
          :class="m.direction === 'sent' ? 'justify-end' : 'justify-start'"
        >
          <div
            class="max-w-[80%] rounded-lg px-[12px] py-[8px]"
            :class="
              m.direction === 'sent'
                ? 'bg-tertiary-strong text-on-tertiary dark:bg-tertiary-dark dark:text-on-tertiary-dark'
                : 'bg-surface text-primary shadow-sm dark:bg-surface-dark dark:text-primary-dark'
            "
          >
            <div class="mb-[2px] text-caption opacity-70">
              {{ m.direction === 'sent' ? '发送' : '接收' }} · {{ formatMsgTime(m.time) }}
            </div>
            <div class="whitespace-pre-wrap break-all font-mono text-body-sm leading-relaxed">
              {{ m.content }}
            </div>
          </div>
        </div>
      </div>
      <p
        v-else
        class="mt-[40px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ connected() ? '暂无消息' : '连接后在此收发消息' }}
      </p>
    </div>

    <!-- 输入行 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <input
        v-model="text"
        class="field-input flex-1 font-mono"
        placeholder="输入消息，Enter 发送"
        spellcheck="false"
        :disabled="!connected()"
        @keyup.enter="send"
      />
      <button class="btn-primary shrink-0" :disabled="!connected()" @click="send">发送</button>
      <button v-if="connected()" class="btn-secondary shrink-0" @click="emit('close')">断开</button>
    </div>
  </div>
</template>
