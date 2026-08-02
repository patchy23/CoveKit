<script setup lang="ts">
/**
 * HttpPanel · 调试面板（Postman 式单面板）
 * 方法下拉含 WS 同级选项：选择 WS 动态显示请求头 + 消息收发区；
 * 选择 HTTP 方法显示 Params/Headers/Body + 响应区。
 */
import { computed, onUnmounted, ref } from "vue";
import type { HttpMethod, HttpResponseResult, WsSession } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import HttpRequestBuilder from "./HttpRequestBuilder.vue";
import HttpResponse from "./HttpResponse.vue";
import WsMessageArea from "./WsMessageArea.vue";
import type { ApiDraft, KvRow } from "./useHttp";
import {
  isValidUrl,
  kvToHeaders,
  kvToQuery,
  mergeQuery,
  methodTextClass,
  newKvId,
} from "./useHttp";

const emit = defineEmits<{ (e: "save"): void }>();

/** 方法下拉：HTTP 方法 + WEBSOCKET 同级 */
const ALL_METHODS = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
  "WEBSOCKET",
] as const;
type Method = (typeof ALL_METHODS)[number];

const method = ref<Method>("GET");
const url = ref("https://httpbin.org/get");
const timeoutMs = ref(15000);
const sending = ref(false);
const response = ref<HttpResponseResult | null>(null);
const error = ref("");
const respondedAt = ref("");

/* 请求构建状态（Params/Headers/Body） */
const params = ref<KvRow[]>([{ id: newKvId(), key: "", value: "" }]);
const headerRows = ref<KvRow[]>([{ id: newKvId(), key: "Accept", value: "application/json" }]);
const bodyMode = ref<"none" | "json" | "text">("none");
const body = ref("");

/* WebSocket 会话 */
const wsSession = ref<WsSession | null>(null);
const wsConnecting = ref(false);
let pollTimer: ReturnType<typeof setInterval> | null = null;

const isWs = computed(() => method.value === "WEBSOCKET");
const wsConnected = computed(() => wsSession.value?.open === true);
const showBody = computed(() => ["POST", "PUT", "PATCH"].includes(method.value) && !isWs.value);

/* ── 请求/连接 ── */
async function sendOrConnect() {
  error.value = "";
  if (isWs.value) {
    await connectWs();
  } else {
    await sendHttp();
  }
}

async function sendHttp() {
  const finalUrl = mergeQuery(url.value.trim(), kvToQuery(params.value));
  if (!isValidUrl(finalUrl)) {
    error.value = "URL 格式无效（支持 http/https）";
    return;
  }
  sending.value = true;
  try {
    respondedAt.value = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    response.value = await ipc.httpRequest({
      method: method.value as HttpMethod,
      url: finalUrl,
      headers: Object.entries(kvToHeaders(headerRows.value)),
      body: showBody.value && bodyMode.value !== "none" ? body.value : undefined,
      timeoutMs: timeoutMs.value,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    response.value = null;
  } finally {
    sending.value = false;
  }
}

async function connectWs() {
  if (wsConnected.value) return;
  if (!isValidUrl(url.value)) {
    error.value = "URL 无效（支持 ws:// 与 wss://）";
    return;
  }
  wsConnecting.value = true;
  try {
    wsSession.value = await ipc.wsConnect({
      url: url.value.trim(),
      headers: Object.entries(kvToHeaders(headerRows.value)),
    });
    startPoll();
  } catch (e) {
    error.value = "连接失败：" + (e instanceof Error ? e.message : String(e));
  } finally {
    wsConnecting.value = false;
  }
}

async function disconnectWs() {
  stopPoll();
  if (wsSession.value) {
    try {
      await ipc.wsClose(wsSession.value.id);
    } catch {
      /* 会话已不存在 */
    }
    wsSession.value = null;
  }
}

async function sendWsMessage(text: string) {
  if (!wsSession.value) return;
  await ipc.wsSend(wsSession.value.id, text);
  await pollOnce();
}

/** 300ms 轮询拉取消息 */
function startPoll() {
  stopPoll();
  pollTimer = setInterval(pollOnce, 300);
}

function stopPoll() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

async function pollOnce() {
  if (!wsSession.value) return;
  try {
    wsSession.value = await ipc.wsRecv(wsSession.value.id);
  } catch {
    // 会话已不存在
  }
}

onUnmounted(() => {
  stopPoll();
  if (wsSession.value) {
    ipc.wsClose(wsSession.value.id).catch(() => {});
  }
});

/* ── 接口草稿（index.vue 接口列表调用） ── */
function getDraft(): ApiDraft {
  return {
    type: isWs.value ? "ws" : "http",
    method: method.value,
    url: url.value,
    params: params.value,
    headers: headerRows.value,
    bodyMode: bodyMode.value,
    body: body.value,
  };
}

function applyDraft(d: ApiDraft) {
  method.value = (d.type === "ws" ? "WEBSOCKET" : d.method || "GET") as Method;
  url.value = d.url;
  params.value = d.params?.length ? d.params : [{ id: newKvId(), key: "", value: "" }];
  headerRows.value = d.headers?.length
    ? d.headers
    : [{ id: newKvId(), key: "Accept", value: "application/json" }];
  bodyMode.value = (d.bodyMode as "none" | "json" | "text") || "none";
  body.value = d.body || "";
  response.value = null;
  error.value = "";
  disconnectWs();
}

defineExpose({ getDraft, applyDraft });
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[10px]">
    <!-- 请求行 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <select
        :value="method"
        class="field-input !w-[112px] !px-[10px] !py-[8px] font-semibold"
        :class="methodTextClass(method, isWs ? 'ws' : undefined)"
        @change="method = ($event.target as HTMLSelectElement).value as Method"
      >
        <option
          v-for="m in ALL_METHODS"
          :key="m"
          :value="m"
          class="font-semibold"
          :class="methodTextClass(m, m === 'WEBSOCKET' ? 'ws' : undefined)"
        >
          {{ m }}
        </option>
      </select>
      <input
        v-model="url"
        class="field-input min-w-0 flex-1 font-mono"
        :placeholder="isWs ? 'wss://example.com/socket' : 'https://example.com/api'"
        spellcheck="false"
        :disabled="isWs && wsConnected"
        @keyup.enter="sendOrConnect"
      />
      <button v-if="!isWs" class="btn-primary shrink-0" :disabled="sending" @click="sendOrConnect">
        {{ sending ? "发送中…" : "发送" }}
      </button>
      <template v-else>
        <button
          v-if="!wsConnected"
          class="btn-primary shrink-0"
          :disabled="wsConnecting"
          @click="connectWs"
        >
          {{ wsConnecting ? "连接中…" : "连接" }}
        </button>
        <button v-else class="btn-secondary shrink-0" @click="disconnectWs">断开</button>
      </template>
      <button class="btn-secondary shrink-0" title="保存为接口" @click="emit('save')">保存</button>
      <select
        v-if="!isWs"
        :value="timeoutMs"
        class="field-input !w-[110px] !px-[10px] !py-[8px]"
        title="超时时间"
        @change="timeoutMs = Number(($event.target as HTMLSelectElement).value)"
      >
        <option :value="5000">5s 超时</option>
        <option :value="15000">15s 超时</option>
        <option :value="60000">60s 超时</option>
      </select>
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
        {{ wsConnected ? "已连接" : "未连接" }}
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
