<script setup lang="ts">
/** 单个接口页签内容，类型由接口绑定；实例切换只隐藏，不卸载。 */
import { computed } from 'vue'
import { UiButton, UiInput, UiSelect } from '@/core/ui'
import type { RequestDraft } from './requestDraft'
import type { RequestSession } from './useRequestSession'
import { METHODS, methodTextClass } from './useHttp'
import HttpRequestBuilder from './HttpRequestBuilder.vue'
import HttpResponse from './HttpResponse.vue'
import StreamMessages from './StreamMessages.vue'
const draft = defineModel<RequestDraft>({ required: true })
const props = defineProps<{ session: RequestSession; saving: boolean; active: boolean }>()
const emit = defineEmits<{ save: []; saveAs: [] }>()
const state = computed(() => props.session.state)
async function run() {
  try {
    await props.session.run()
  } catch {
    /* session 已保存并展示错误 */
  }
}
async function stop() {
  try {
    await props.session.stop()
  } catch {
    /* session 已保存并展示错误 */
  }
}
function shortcut(event: KeyboardEvent) {
  if (!props.active || !(event.ctrlKey || event.metaKey)) return
  if (event.key === 'Enter') {
    event.preventDefault()
    if (draft.value.type === 'ws' && state.value.connected) void props.session.send()
    else if (!state.value.connected) void run()
  }
  if (event.key.toLowerCase() === 's') {
    event.preventDefault()
    emit('save')
  }
}
</script>
<template>
  <div class="flex h-full min-h-0 min-w-0 flex-col" @keydown="shortcut">
    <div class="flex shrink-0 flex-wrap items-center gap-[6px] p-[10px]">
      <UiSelect
        v-if="draft.type !== 'ws'"
        v-model="draft.method"
        size="sm"
        class="w-[88px] shrink-0"
        title="请求方法"
        :options="METHODS.map((m) => ({ value: m, label: m }))"
        :option-class="methodTextClass"
        :disabled="state.busy || state.connected"
      />
      <UiInput
        v-model="draft.url"
        size="sm"
        class="min-w-[140px] flex-1 font-mono"
        aria-label="请求地址"
        :placeholder="
          draft.type === 'ws' ? 'ws://localhost:8080/socket' : 'http://localhost:8080/api'
        "
        :disabled="state.busy || state.connected"
        spellcheck="false"
        @keyup.enter.exact="run"
      />
      <UiButton
        v-if="state.connected || (state.busy && draft.type !== 'http')"
        size="sm"
        @click="stop"
        >{{ draft.type === 'sse' ? '停止订阅' : '断开' }}</UiButton
      >
      <UiButton v-else size="sm" variant="primary" :loading="state.busy" @click="run">{{
        draft.type === 'http' ? '发送' : draft.type === 'sse' ? '开始订阅' : '连接'
      }}</UiButton>
      <UiButton size="xs" variant="ghost" :disabled="saving" @click="emit('save')">保存</UiButton
      ><UiButton size="xs" variant="ghost" :disabled="saving" @click="emit('saveAs')"
        >另存</UiButton
      >
    </div>
    <p
      v-if="state.error"
      role="alert"
      class="shrink-0 select-text break-all px-[10px] pb-[8px] text-body-sm text-tertiary-strong dark:text-tertiary-dark"
    >
      {{ state.error }}
    </p>
    <HttpRequestBuilder v-model="draft" @save="emit('save')" />
    <HttpResponse
      v-if="draft.type === 'http' && state.response"
      :response="state.response"
      :responded-at="state.respondedAt"
      @save="emit('save')"
    />
    <div v-else-if="draft.type === 'http'" class="flex min-h-0 flex-1 flex-col">
      <div class="border-b border-border px-[10px] py-[8px] text-body-sm dark:border-border-dark">
        响应
      </div>
      <p class="p-[16px] text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ state.busy ? '正在等待响应…' : '发送请求后，在这里查看响应。' }}
      </p>
    </div>
    <StreamMessages
      v-else
      :kind="draft.type"
      :session="session"
      :active="active"
      @save="emit('save')"
    />
  </div>
</template>
