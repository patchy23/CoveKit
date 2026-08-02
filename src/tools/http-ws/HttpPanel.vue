<script setup lang="ts">
/**
 * HTTP 调试面板 · 请求构建 + 响应查看（JSON 高亮）
 */
import { computed, ref } from "vue";
import hljs from "highlight.js";
import type { HttpMethod, HttpResponseResult } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import {
  METHODS,
  formatBytes,
  formatHeaders,
  isValidUrl,
  looksLikeJson,
  parseHeaders,
} from "./useHttp";

const method = ref<HttpMethod>("GET");
const url = ref("https://httpbin.org/get");
const headersText = ref("Accept: application/json");
const body = ref("");
const timeoutMs = ref(15000);
const sending = ref(false);
const response = ref<HttpResponseResult | null>(null);
const error = ref("");

const showBody = computed(() => ["POST", "PUT", "PATCH"].includes(method.value));

const highlightedBody = computed(() => {
  if (!response.value?.body) return "";
  if (!looksLikeJson(response.value.body)) return "";
  try {
    return hljs.highlight(response.value.body, { language: "json", ignoreIllegals: true }).value;
  } catch {
    return "";
  }
});

const statusClass = computed(() => {
  if (!response.value) return "";
  const s = response.value.status;
  if (s >= 200 && s < 300)
    return "bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark";
  if (s >= 400)
    return "bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark";
  return "bg-warning-soft text-warning-strong dark:bg-warning-soft-dark dark:text-warning-dark";
});

async function send() {
  error.value = "";
  if (!isValidUrl(url.value)) {
    error.value = "URL 无效（支持 http/https）";
    return;
  }
  sending.value = true;
  try {
    response.value = await ipc.httpRequest({
      method: method.value,
      url: url.value.trim(),
      headers: parseHeaders(headersText.value),
      body: showBody.value ? body.value : undefined,
      timeoutMs: timeoutMs.value,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    response.value = null;
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <!-- 固定请求行（发送按钮滚动时始终可见） -->
    <div class="sticky-toolbar !py-[10px]">
      <select
        :value="method"
        class="field-input !w-[100px] !px-[10px] !py-[8px]"
        @change="method = ($event.target as HTMLSelectElement).value as HttpMethod"
      >
        <option v-for="m in METHODS" :key="m" :value="m">{{ m }}</option>
      </select>
      <input
        v-model="url"
        class="field-input font-mono"
        placeholder="https://example.com/api"
        spellcheck="false"
        @keyup.enter="send"
      />
      <button class="btn-primary shrink-0" :disabled="sending" @click="send">
        {{ sending ? "发送中…" : "发送" }}
      </button>
      <select
        :value="timeoutMs"
        class="field-input !w-[110px] !px-[10px] !py-[8px]"
        title="超时时间"
        @change="timeoutMs = Number(($event.target as HTMLSelectElement).value)"
      >
        <option :value="5000">5s 超时</option>
        <option :value="15000">15s 超时</option>
        <option :value="60000">60s 超时</option>
      </select>
    </div>

    <div class="grid grid-cols-2 gap-[12px]">
      <div>
        <label class="mb-[6px] field-label">请求头（Name: Value，每行一个）</label>
        <LineNumberTextarea v-model="headersText" min-height="120px" />
      </div>
      <div v-if="showBody">
        <label class="mb-[6px] field-label">请求体</label>
        <LineNumberTextarea v-model="body" min-height="120px" placeholder="请求体内容（JSON 等）" />
      </div>
    </div>

    <p v-if="error" class="text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <!-- 响应 -->
    <div v-if="response" class="flex flex-col gap-[8px]">
      <div class="flex flex-wrap items-center gap-[10px]">
        <span class="rounded-full px-[10px] py-[3px] text-caption font-medium" :class="statusClass">
          {{ response.ok ? response.status : response.statusText || "请求失败" }}
        </span>
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ response.durationMs }} ms
        </span>
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ formatBytes(response.bodySize) }}
        </span>
        <span
          v-if="response.error"
          class="text-body-sm text-tertiary-strong dark:text-tertiary-dark"
          >{{ response.error }}</span
        >
      </div>

      <details class="rounded-md border border-border px-[12px] py-[8px] dark:border-border-dark">
        <summary
          class="cursor-pointer text-body-sm font-medium text-secondary dark:text-secondary-dark"
        >
          响应头（{{ response.headers.length }}）
        </summary>
        <pre
          class="mt-[8px] max-h-[160px] overflow-auto font-mono text-body-sm leading-relaxed text-secondary dark:text-secondary-dark"
          >{{ formatHeaders(response.headers) }}</pre>
      </details>

      <div>
        <label class="mb-[6px] field-label">响应体</label>
        <pre
          v-if="highlightedBody"
          class="max-h-[360px] overflow-auto rounded-md border border-border bg-surface-muted p-[13px] font-mono text-body leading-relaxed dark:border-border-dark dark:bg-surface-muted-dark"
        ><code class="hljs" v-html="highlightedBody" /></pre>
        <pre
          v-else
          class="max-h-[360px] overflow-auto whitespace-pre-wrap rounded-md border border-border bg-surface-muted p-[13px] font-mono text-body leading-relaxed text-primary dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
          >{{ response.body || "(空响应体)" }}</pre>
      </div>
    </div>
  </div>
</template>
