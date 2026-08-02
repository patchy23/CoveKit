<script setup lang="ts">
/**
 * HttpPanel · HTTP 调试面板（Postman 式：请求行 + Params/Headers/Body + 响应查看）
 * 接口列表管理在 index.vue（本组件通过 defineExpose 提供 getDraft/applyDraft）。
 */
import { computed, ref } from "vue";
import type { HttpMethod, HttpResponseResult } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import HttpRequestBuilder from "./HttpRequestBuilder.vue";
import HttpResponse from "./HttpResponse.vue";
import type { ApiDraft, KvRow } from "./useHttp";
import { isValidUrl, kvToHeaders, kvToQuery, mergeQuery, newKvId } from "./useHttp";

const emit = defineEmits<{ (e: "save"): void }>();

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

const showBody = computed(() => ["POST", "PUT", "PATCH"].includes(method.value));

/* ── 接口草稿（index.vue 接口列表调用） ── */
function getDraft(): ApiDraft {
  return {
    type: "http",
    method: method.value,
    url: url.value,
    params: params.value,
    headers: headerRows.value,
    bodyMode: bodyMode.value,
    body: body.value,
  };
}

function applyDraft(d: ApiDraft) {
  method.value = (d.method as HttpMethod) || "GET";
  url.value = d.url;
  params.value = d.params?.length ? d.params : [{ id: newKvId(), key: "", value: "" }];
  headerRows.value = d.headers?.length
    ? d.headers
    : [{ id: newKvId(), key: "Accept", value: "application/json" }];
  bodyMode.value = (d.bodyMode as "none" | "json" | "text") || "none";
  body.value = d.body || "";
  response.value = null;
  error.value = "";
}

defineExpose({ getDraft, applyDraft });

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
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    response.value = null;
  } finally {
    sending.value = false;
  }
}
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
      <button class="btn-secondary shrink-0" title="保存为接口" @click="emit('save')">保存</button>
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
</template>
