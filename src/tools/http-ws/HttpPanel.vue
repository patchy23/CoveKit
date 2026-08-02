<script setup lang="ts">
/**
 * HttpPanel · HTTP 调试主面板（Postman 式布局）
 * 顶部请求行（方法/URL/发送/超时/保存）+ 左侧接口列表 + 右侧请求构建/响应查看。
 */
import { computed, onMounted, ref } from "vue";
import type { ApiRecord, HttpMethod, HttpResponseResult } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import ApiSidebar from "./ApiSidebar.vue";
import HttpRequestBuilder from "./HttpRequestBuilder.vue";
import HttpResponse from "./HttpResponse.vue";
import type { KvRow } from "./useHttp";
import { isValidUrl, kvToHeaders, kvToQuery, mergeQuery, newKvId } from "./useHttp";

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

/* 接口列表 */
const apis = ref<ApiRecord[]>([]);
const activeApiId = ref<number | null>(null);
const saveNameOpen = ref(false);
const saveName = ref("");

const showBody = computed(() => ["POST", "PUT", "PATCH"].includes(method.value));

async function loadApis() {
  try {
    apis.value = await ipc.apiList();
  } catch {
    apis.value = [];
  }
}

async function deleteApi(id: number) {
  try {
    await ipc.apiDelete(id);
    if (activeApiId.value === id) activeApiId.value = null;
    await loadApis();
  } catch {
    /* 忽略 */
  }
}

async function clearApis() {
  try {
    await ipc.apiClear();
    apis.value = [];
    activeApiId.value = null;
  } catch {
    /* 忽略 */
  }
}

/** 新建接口：清空表单 */
function newApi() {
  method.value = "GET";
  url.value = "";
  params.value = [{ id: newKvId(), key: "", value: "" }];
  headerRows.value = [{ id: newKvId(), key: "Accept", value: "application/json" }];
  bodyMode.value = "none";
  body.value = "";
  activeApiId.value = null;
  saveNameOpen.value = false;
  saveName.value = "";
  response.value = null;
  error.value = "";
}

/** 点击接口：加载到表单 */
function applyApi(a: ApiRecord) {
  method.value = (a.method as HttpMethod) || "GET";
  url.value = a.url;
  try {
    const p = JSON.parse(a.params || "[]");
    params.value = Array.isArray(p) && p.length ? p : [{ id: newKvId(), key: "", value: "" }];
  } catch {
    params.value = [{ id: newKvId(), key: "", value: "" }];
  }
  try {
    const h = JSON.parse(a.headers || "[]");
    headerRows.value = Array.isArray(h) && h.length ? h : [{ id: newKvId(), key: "", value: "" }];
  } catch {
    headerRows.value = [{ id: newKvId(), key: "", value: "" }];
  }
  bodyMode.value = (a.bodyMode as "none" | "json" | "text") || "none";
  body.value = a.body || "";
  activeApiId.value = a.id;
  saveName.value = a.name;
  saveNameOpen.value = false;
  response.value = null;
  error.value = "";
}

/** 保存当前请求为接口（无名称时展开命名输入） */
async function saveApi() {
  const name =
    saveName.value.trim() ||
    (activeApiId.value ? apis.value.find((a) => a.id === activeApiId.value)?.name || "" : "");
  if (!name) {
    saveNameOpen.value = true;
    return;
  }
  try {
    const id = await ipc.apiSave({
      id: activeApiId.value ?? 0,
      name,
      method: method.value,
      url: url.value.trim(),
      params: JSON.stringify(params.value),
      headers: JSON.stringify(headerRows.value),
      bodyMode: bodyMode.value,
      body: body.value,
    });
    activeApiId.value = id;
    saveNameOpen.value = false;
    await loadApis();
  } catch {
    /* 忽略 */
  }
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
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    response.value = null;
  } finally {
    sending.value = false;
  }
}

onMounted(loadApis);
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
      <button
        class="btn-secondary shrink-0"
        :title="activeApiId ? '更新当前接口' : '保存为接口'"
        @click="saveApi"
      >
        保存
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

    <!-- 命名输入（保存新接口） -->
    <div v-if="saveNameOpen" class="flex shrink-0 items-center gap-[8px]">
      <input
        v-model="saveName"
        class="field-input max-w-[320px] font-mono"
        placeholder="接口名称，如：获取用户列表"
        spellcheck="false"
        @keyup.enter="saveApi"
      />
      <button class="btn-primary" @click="saveApi">确定</button>
      <button class="btn-ghost" @click="saveNameOpen = false">取消</button>
    </div>

    <p v-if="error" class="shrink-0 text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <!-- 左侧接口列表 + 右侧构建/响应 -->
    <div class="flex min-h-0 flex-1 gap-[12px]">
      <ApiSidebar
        :apis="apis"
        :active-id="activeApiId"
        @select="applyApi"
        @delete="deleteApi"
        @clear="clearApis"
        @new="newApi"
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
