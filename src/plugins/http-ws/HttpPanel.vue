<script setup lang="ts">
/**
 * HttpPanel · 调试面板（Postman 式单面板）
 * 方法下拉含 WS 同级选项：选择 WS 动态显示请求头 + 消息收发区；
 * 选择 HTTP 方法显示 Params/Headers/Body + 响应区。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { HttpMethod, HttpResponseResult, WsSession } from './contracts'
import { useToolScope } from '@/core/lifecycle'
import { ipc } from './ipc'
import { UiButton, UiInput, UiSelect as Select } from '@/core/ui'
import HttpRequestBuilder from './HttpRequestBuilder.vue'
import HttpResponse from './HttpResponse.vue'
import WsMessageArea from './WsMessageArea.vue'
import type { ApiDraft, KvRow } from './useHttp'
import { isValidUrl, kvToHeaders, kvToQuery, mergeQuery, methodTextClass, newKvId } from './useHttp'

const emit = defineEmits<{ (e: 'save'): void }>()

/** 方法下拉：HTTP 方法 + WEBSOCKET 同级 */
const ALL_METHODS = [
  'GET',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
  'HEAD',
  'OPTIONS',
  'WEBSOCKET',
] as const
type Method = (typeof ALL_METHODS)[number]

const method = ref<Method>('GET')
const url = ref('https://httpbin.org/get')
const timeoutMs = ref(15000)
const sending = ref(false)
const response = ref<HttpResponseResult | null>(null)
const error = ref('')
const respondedAt = ref('')

/* 请求构建状态（Params/Headers/Body） */
const params = ref<KvRow[]>([{ id: newKvId(), key: '', value: '' }])
const headerRows = ref<KvRow[]>([{ id: newKvId(), key: 'Accept', value: 'application/json' }])
const bodyMode = ref<'none' | 'json' | 'text'>('none')
const body = ref('')

/* WebSocket 会话 */
const wsSession = ref<WsSession | null>(null)
const wsConnecting = ref(false)

// WS 消息轮询挂在工具作用域上（T10-5/T10-7）：卸载即释放；
// 窗口隐藏 / 页签切走时降频到 1s（消息仍会被拉到，只是不那么勤）
const { scope, visibility, onResume } = useToolScope('http-ws', 'http-ws.wspoll')
const WS_POLL_VISIBLE_MS = 300
const WS_POLL_HIDDEN_MS = 1000
/** 轮询开关：停止后不再排下一轮（定时器本身由 scope 持有并释放） */
let polling = false

const isWs = computed(() => method.value === 'WEBSOCKET')
const wsConnected = computed(() => wsSession.value?.open === true)
const showBody = computed(() => ['POST', 'PUT', 'PATCH'].includes(method.value) && !isWs.value)

/* ── 请求/连接 ── */
async function sendOrConnect() {
  error.value = ''
  if (isWs.value) {
    await connectWs()
  } else {
    await sendHttp()
  }
}

async function sendHttp() {
  const finalUrl = mergeQuery(url.value.trim(), kvToQuery(params.value))
  if (!isValidUrl(finalUrl)) {
    error.value = 'URL 格式无效（支持 http/https）'
    return
  }
  sending.value = true
  try {
    respondedAt.value = new Date().toLocaleTimeString('zh-CN', { hour12: false })
    response.value = await ipc.httpRequest({
      method: method.value as HttpMethod,
      url: finalUrl,
      headers: Object.entries(kvToHeaders(headerRows.value)),
      body: showBody.value && bodyMode.value !== 'none' ? body.value : undefined,
      timeoutMs: timeoutMs.value,
    })
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    response.value = null
  } finally {
    sending.value = false
  }
}

async function connectWs() {
  if (wsConnected.value) return
  if (!isValidUrl(url.value)) {
    error.value = 'URL 无效（支持 ws:// 与 wss://）'
    return
  }
  wsConnecting.value = true
  try {
    wsSession.value = await ipc.wsConnect({
      url: url.value.trim(),
      headers: Object.entries(kvToHeaders(headerRows.value)),
    })
    startPoll()
  } catch (e) {
    error.value = '连接失败：' + (e instanceof Error ? e.message : String(e))
  } finally {
    wsConnecting.value = false
  }
}

async function disconnectWs() {
  stopPoll()
  if (wsSession.value) {
    try {
      await ipc.wsClose(wsSession.value.id)
    } catch {
      /* 会话已不存在 */
    }
    wsSession.value = null
  }
}

async function sendWsMessage(text: string) {
  if (!wsSession.value) return
  await ipc.wsSend(wsSession.value.id, text)
  await pollOnce()
}

/** 排下一轮轮询：可见 300ms，隐藏/非激活降频到 1s（不停止，避免漏掉会话侧消息） */
function armPoll(delay: number): void {
  scope.timeout(() => {
    if (!polling) return
    void pollOnce()
    const hidden = !visibility.value.active || visibility.value.covered || visibility.value.hidden
    armPoll(hidden ? WS_POLL_HIDDEN_MS : WS_POLL_VISIBLE_MS)
  }, delay)
}

/** 开始轮询 */
function startPoll() {
  polling = true
  armPoll(WS_POLL_VISIBLE_MS)
}

/** 停止轮询（定时器由 scope 释放，这里只关掉「继续排下一轮」） */
function stopPoll() {
  polling = false
}

async function pollOnce() {
  if (!wsSession.value) return
  try {
    wsSession.value = await ipc.wsRecv(wsSession.value.id)
  } catch {
    // 会话已不存在
  }
}

onMounted(() => {
  // 恢复可见时立刻补一次消息拉取，不等下一个周期
  onResume(() => {
    void pollOnce()
  })
})

onUnmounted(() => {
  if (wsSession.value) {
    ipc.wsClose(wsSession.value.id).catch(() => {})
  }
})

/* ── 接口草稿（index.vue 接口列表调用） ── */
function getDraft(): ApiDraft {
  return {
    type: isWs.value ? 'ws' : 'http',
    method: method.value,
    url: url.value,
    params: params.value,
    headers: headerRows.value,
    bodyMode: bodyMode.value,
    body: body.value,
  }
}

function applyDraft(d: ApiDraft) {
  method.value = (d.type === 'ws' ? 'WEBSOCKET' : d.method || 'GET') as Method
  url.value = d.url
  params.value = d.params?.length ? d.params : [{ id: newKvId(), key: '', value: '' }]
  headerRows.value = d.headers?.length
    ? d.headers
    : [{ id: newKvId(), key: 'Accept', value: 'application/json' }]
  bodyMode.value = (d.bodyMode as 'none' | 'json' | 'text') || 'none'
  body.value = d.body || ''
  response.value = null
  error.value = ''
  disconnectWs()
}

defineExpose({ getDraft, applyDraft })
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[10px]">
    <!-- 请求行 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <Select
        v-model="method"
        class="!w-[140px] shrink-0"
        title="请求方法"
        :options="ALL_METHODS.map((m) => ({ value: m, label: m }))"
        :option-class="(v: string) => methodTextClass(v, v === 'WEBSOCKET' ? 'ws' : undefined)"
        :value-class="(v: string) => methodTextClass(v, v === 'WEBSOCKET' ? 'ws' : undefined)"
      />
      <UiInput
        v-model="url"
        class="min-w-0 flex-1 font-mono"
        :placeholder="isWs ? 'wss://example.com/socket' : 'https://example.com/api'"
        spellcheck="false"
        :disabled="isWs && wsConnected"
        @keyup.enter="sendOrConnect"
      />
      <UiButton
        v-if="!isWs"
        variant="primary"
        class="shrink-0"
        :loading="sending"
        @click="sendOrConnect"
      >
        {{ sending ? '发送中…' : '发送' }}
      </UiButton>
      <template v-else>
        <UiButton
          v-if="!wsConnected"
          variant="primary"
          class="shrink-0"
          :loading="wsConnecting"
          @click="connectWs"
        >
          {{ wsConnecting ? '连接中…' : '连接' }}
        </UiButton>
        <UiButton v-else class="shrink-0" @click="disconnectWs">断开</UiButton>
      </template>
      <UiButton class="shrink-0" title="保存为接口" @click="emit('save')">保存</UiButton>
      <Select
        v-if="!isWs"
        :model-value="String(timeoutMs)"
        class="!w-[110px] shrink-0"
        title="超时时间"
        :options="[
          { value: '5000', label: '5s 超时' },
          { value: '15000', label: '15s 超时' },
          { value: '60000', label: '60s 超时' },
        ]"
        @update:model-value="timeoutMs = Number($event)"
      />
      <span
        v-else
        class="flex shrink-0 items-center gap-[6px] text-body-sm"
        :class="
          wsConnected
            ? 'text-success-strong dark:text-success-dark'
            : 'text-text-muted dark:text-text-muted-dark'
        "
      >
        <span
          class="h-[8px] w-[8px] rounded-full"
          :class="
            wsConnected
              ? 'bg-success dark:bg-success-dark'
              : 'bg-border-strong dark:bg-border-strong-dark'
          "
        />
        {{ wsConnected ? '已连接' : '未连接' }}
      </span>
    </div>

    <p v-if="error" class="shrink-0 text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <!-- 构建区：HTTP 用 Params/Headers/Body；WS 仅 Headers -->
    <HttpRequestBuilder
      v-model:params="params"
      v-model:headers="headerRows"
      v-model:body-mode="bodyMode"
      v-model:body="body"
      :headers-only="isWs"
    />

    <!-- 下方：HTTP 响应 / WS 消息收发 -->
    <template v-if="!isWs">
      <HttpResponse v-if="response" :response="response" :responded-at="respondedAt" />
      <div
        v-else
        class="grid min-h-0 flex-1 place-items-center rounded-md border border-dashed border-border text-body-sm text-text-muted dark:border-border-dark dark:text-text-muted-dark"
      >
        发送请求后在这里查看响应
      </div>
    </template>
    <WsMessageArea v-else :session="wsSession" @send="sendWsMessage" @close="disconnectWs" />
  </div>
</template>
