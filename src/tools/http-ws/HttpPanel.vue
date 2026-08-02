<script setup lang="ts">
/**
 * HttpPanel · HTTP 调试主面板（Postman 式布局）
 * 顶部请求行（方法/URL/发送/超时）+ 左侧历史栏 + 右侧请求构建/响应查看。
 */
import { computed, onMounted, ref } from "vue";
import type { HistoryRecord, HttpMethod, HttpResponseResult } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import HttpSidebar from "./HttpSidebar.vue";
import HttpRequestBuilder from "./HttpRequestBuilder.vue";
import HttpResponse from "./HttpResponse.vue";
import type { KvRow } from "./useHttp";
import {
  headersToText,
  isValidUrl,
  kvToHeaders,
  kvToQuery,
  mergeQuery,
  newKvId,
  parseHeaders,
} from "./useHttp";

const method = ref<HttpMethod>("GET");
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

/* 历史 */
const records = ref<HistoryRecord[]>([]);
const activeId = ref<number | null>(null);

const showBody = computed(() => ["POST", "PUT", "PATCH"].includes(method.value));

async function loadHistory() {
  try {
    records.value = await ipc.historyList(50);
  } catch {
    records.value = [];
  }
}

async function clearHistory() {
  try {
    await ipc.historyClear();
    records.value = [];
    activeId.value = null;
  } catch {
    /* 忽略 */
  }
}

/** 发送成功后记录历史 */
async function saveHistory(res: HttpResponseResult) {
  try {
    await ipc.historyAdd({
      id: 0,
      method: method.value,
      url: mergeQuery(url.value.trim(), kvToQuery(params.value)),
      headers: headersToText(Object.entries(kvToHeaders(headerRows.value))),
      body: showBody.value && bodyMode.value !== "none" ? body.value : "",
      status: res.ok ? res.status : undefined,
      durationMs: res.durationMs,
      bodySize: res.bodySize,
      createdAt: "",
    });
    await loadHistory();
  } catch {
    /* 历史失败不影响请求 */
  }
}

/** 点击历史：回填请求 */
function applyRecord(r: HistoryRecord) {
  method.value = (r.method as HttpMethod) || "GET";
  url.value = r.url;
  const pairs = parseHeaders(r.headers);
  headerRows.value = pairs.map(([k, v]) => ({ id: newKvId(), key: k, value: v }));
  if (r.headers && !headerRows.value.length)
    headerRows.value = [{ id: newKvId(), key: "", value: "" }];
  body.value = r.body || "";
  bodyMode.value = r.body ? "json" : "none";
  activeId.value = r.id;
  error.value = "";
  response.value = null;
}

async function send() {
  error.value = "";
  const finalUrl = mergeQuery(url.value.trim(), kvToQuery(params.value));
  if (!isValidUrl(finalUrl)) {
    error.value = "URL 格式无效（支持 http/https）";
    return;
  }
  sending.value = true;
  try {
    respondedAt.value = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    response.value = await ipc.httpRequest({
      method: method.value,
      url: finalUrl,
      headers: Object.entries(kvToHeaders(headerRows.value)),
      body: showBody.value && bodyMode.value !== "none" ? body.value : undefined,
      timeoutMs: timeoutMs.value,
    });
    if (response.value) await saveHistory(response.value);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    response.value = null;
  } finally {
    sending.value = false;
  }
}

onMounted(loadHistory);
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[10px]">
    <!-- 请求行 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <select
        :value="method"
        class="field-input !w-[100px] !px-[10px] !py-[8px]"
        @change="method = ($event.target as HTMLSelectElement).value as HttpMethod"
      >
        <option
          v-for="m in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const"
          :key="m"
          :value="m"
        >
          {{ m }}
        </option>
      </select>
      <input
        v-model="url"
        class="field-input min-w-0 flex-1 font-mono"
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

    <p v-if="error" class="shrink-0 text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <!-- 左侧历史栏 + 右侧构建/响应 -->
    <div class="flex min-h-0 flex-1 gap-[12px]">
      <HttpSidebar
        :records="records"
        :active-id="activeId"
        @select="applyRecord"
        @clear="clearHistory"
      />

      <div class="flex min-h-0 flex-1 flex-col gap-[10px]">
        <HttpRequestBuilder
          v-model:params="params"
          v-model:headers="headerRows"
          v-model:body-mode="bodyMode"
          v-model:body="body"
        />

        <!-- 响应区 -->
        <HttpResponse v-if="response" :response="response" :responded-at="respondedAt" />
        <div
          v-else
          class="grid min-h-0 flex-1 place-items-center rounded-md border border-dashed border-border text-body-sm text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          发送请求后在这里查看响应
        </div>
      </div>
    </div>
  </div>
</template>
