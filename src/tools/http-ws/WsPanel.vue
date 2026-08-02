<script setup lang="ts">
/**
 * WebSocket 调试面板 · 连接/收发消息（长连接会话，300ms 轮询拉取）
 */
import { computed, onUnmounted, ref } from "vue";
import type { WsSession } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import { useUiStore } from "@/stores/ui";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import { isValidUrl, parseHeaders } from "./useHttp";

const ui = useUiStore();

const url = ref("wss://echo.websocket.org");
const headersText = ref("Authorization: Bearer YOUR_TOKEN");
const message = ref("");
const session = ref<WsSession | null>(null);
const connecting = ref(false);
const error = ref("");

const connected = computed(() => session.value?.open === true);

const hasMessage = computed(() => (session.value?.messages.length ?? 0) > 0);

let pollTimer: ReturnType<typeof setInterval> | null = null;

function formatMsgTime(t: number) {
  const d = new Date(t);
  return d.toLocaleTimeString("zh-CN", { hour12: false });
}

async function connect() {
  error.value = "";
  if (!isValidUrl(url.value)) {
    error.value = "URL 无效（支持 ws:// 与 wss://）";
    return;
  }
  connecting.value = true;
  try {
    session.value = await ipc.wsConnect({
      url: url.value.trim(),
      headers: parseHeaders(headersText.value),
    });
    startPoll();
  } catch (e) {
    error.value = "连接失败：" + (e instanceof Error ? e.message : String(e));
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  if (session.value) {
    await ipc.wsClose(session.value.id);
    session.value = null;
    stopPoll();
  }
}

async function send() {
  const text = message.value.trim();
  if (!text || !session.value) return;
  try {
    const r = await ipc.wsSend(session.value.id, text);
    if (!r.ok) {
      ui.toast(r.message ?? "发送失败");
    }
    message.value = "";
  } catch (e) {
    ui.toast("发送失败：" + (e instanceof Error ? e.message : String(e)));
  }
}

function startPoll() {
  stopPoll();
  pollTimer = setInterval(async () => {
    if (!session.value) return;
    try {
      session.value = await ipc.wsRecv(session.value.id);
    } catch {
      // 会话已不存在
    }
  }, 300);
}

function stopPoll() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

onUnmounted(() => {
  stopPoll();
  if (session.value) {
    ipc.wsClose(session.value.id).catch(() => {});
  }
});
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <!-- 连接栏 -->
    <div class="flex items-center gap-[8px]">
      <input
        v-model="url"
        class="field-input font-mono"
        placeholder="wss://example.com/socket"
        spellcheck="false"
        :disabled="connected"
        @keyup.enter="connected ? undefined : connect()"
      />
      <button
        v-if="!connected"
        class="btn-primary shrink-0"
        :disabled="connecting"
        @click="connect"
      >
        {{ connecting ? "连接中…" : "连接" }}
      </button>
      <button v-else class="btn-secondary shrink-0" @click="disconnect">断开</button>
      <span
        class="flex items-center gap-[6px] text-body-sm"
        :class="
          connected
            ? 'text-success-strong dark:text-success-dark'
            : 'text-text-muted dark:text-text-muted-dark'
        "
      >
        <span
          class="h-[8px] w-[8px] rounded-full"
          :class="
            connected
              ? 'bg-success dark:bg-success-dark'
              : 'bg-border-strong dark:bg-border-strong-dark'
          "
        />
        {{ connected ? "已连接" : "未连接" }}
      </span>
    </div>

    <p v-if="error" class="text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <!-- 连接请求头（token 等；握手时随 Upgrade 请求发送） -->
    <details
      class="rounded-md border border-border px-[12px] py-[8px] dark:border-border-dark"
      :open="!!headersText.trim()"
    >
      <summary
        class="cursor-pointer text-body-sm font-medium text-secondary dark:text-secondary-dark"
      >
        请求头（握手时发送，如 Authorization: Bearer xxx）
      </summary>
      <div class="mt-[8px]">
        <LineNumberTextarea v-model="headersText" min-height="64px" />
      </div>
    </details>

    <!-- 消息流 -->
    <div
      class="min-h-[280px] flex-1 overflow-y-auto rounded-md border border-border bg-surface-muted p-[12px] dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <div v-if="hasMessage && session" class="flex flex-col gap-[8px]">
        <div
          v-for="(m, i) in session.messages"
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
            <div class="break-all whitespace-pre-wrap font-mono text-body-sm leading-relaxed">
              {{ m.content }}
            </div>
            <div
              class="mt-[2px] text-caption opacity-70"
              :class="
                m.direction === 'sent'
                  ? 'text-on-tertiary'
                  : 'text-text-muted dark:text-text-muted-dark'
              "
            >
              {{ m.direction === "sent" ? "发送" : "接收" }} · {{ formatMsgTime(m.time) }}
            </div>
          </div>
        </div>
      </div>
      <p
        v-else
        class="py-[24px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ connected ? "已连接，等待消息…" : "连接后收发消息将显示在这里" }}
      </p>
    </div>

    <!-- 发送栏 -->
    <div class="flex items-center gap-[8px]">
      <input
        v-model="message"
        class="field-input font-mono"
        placeholder="输入要发送的消息…"
        spellcheck="false"
        :disabled="!connected"
        @keyup.enter="send"
      />
      <button class="btn-primary shrink-0" :disabled="!connected" @click="send">发送</button>
    </div>
  </div>
</template>
