<script setup lang="ts">
/**
 * HTTP/WS 调试 · 主容器（接口列表管理中枢）
 * 顶部：HTTP/WebSocket 面板切换 + 保存按钮 + 命名输入
 * 左侧：接口列表（HTTP/WS 共用，SQLite 持久化）· 右侧：当前面板
 */
import { onMounted, ref } from "vue";
import type { ApiRecord } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import HttpPanel from "./HttpPanel.vue";
import WsPanel from "./WsPanel.vue";
import ApiSidebar from "./ApiSidebar.vue";
import type { ApiDraft } from "./useHttp";

const tab = ref<"http" | "ws">("http");

/* 面板实例（通过 defineExpose 取草稿/应用草稿） */
const httpPanel = ref<InstanceType<typeof HttpPanel> | null>(null);
const wsPanel = ref<InstanceType<typeof WsPanel> | null>(null);

/* 接口列表 */
const apis = ref<ApiRecord[]>([]);
const activeApiId = ref<number | null>(null);
const saveNameOpen = ref(false);
const saveName = ref("");

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

/** 当前面板的草稿 */
function currentDraft(): ApiDraft | null {
  if (tab.value === "http") return httpPanel.value?.getDraft() ?? null;
  return wsPanel.value?.getDraft() ?? null;
}

/** 新建接口：切到对应面板并清空（由面板 applyDraft 空草稿实现） */
function newApi() {
  activeApiId.value = null;
  saveNameOpen.value = false;
  saveName.value = "";
  const empty: ApiDraft = {
    type: tab.value,
    method: "",
    url: "",
    params: [],
    headers: [],
    bodyMode: "none",
    body: "",
  };
  if (tab.value === "http") httpPanel.value?.applyDraft(empty);
  else wsPanel.value?.applyDraft(empty);
}

/** 点击接口：切换到对应面板并加载 */
function applyApi(a: ApiRecord) {
  tab.value = a.type === "ws" ? "ws" : "http";
  const draft: ApiDraft = {
    type: a.type,
    method: a.method,
    url: a.url,
    params: safeParse(a.params),
    headers: safeParse(a.headers),
    bodyMode: (a.bodyMode as "none" | "json" | "text") || "none",
    body: a.body || "",
  };
  // 等面板切换渲染后应用草稿
  setTimeout(() => {
    if (draft.type === "ws") wsPanel.value?.applyDraft(draft);
    else httpPanel.value?.applyDraft(draft);
  }, 0);
  activeApiId.value = a.id;
  saveName.value = a.name;
  saveNameOpen.value = false;
}

function safeParse(json: string): never[] | { id: string; key: string; value: string }[] {
  try {
    const v = JSON.parse(json || "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

/** 保存当前请求为接口 */
async function saveApi() {
  const draft = currentDraft();
  if (!draft) return;
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
      type: draft.type,
      name,
      method: draft.method,
      url: draft.url.trim(),
      params: JSON.stringify(draft.params),
      headers: JSON.stringify(draft.headers),
      bodyMode: draft.bodyMode,
      body: draft.body,
    });
    activeApiId.value = id;
    saveNameOpen.value = false;
    await loadApis();
  } catch {
    /* 忽略 */
  }
}

onMounted(loadApis);
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[10px]">
    <!-- 面板切换 + 保存 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <button
        class="rounded-md px-[14px] py-[7px] text-body font-medium transition-colors"
        :class="
          tab === 'http'
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
        "
        @click="tab = 'http'"
      >
        HTTP 请求
      </button>
      <button
        class="rounded-md px-[14px] py-[7px] text-body font-medium transition-colors"
        :class="
          tab === 'ws'
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
        "
        @click="tab = 'ws'"
      >
        WebSocket
      </button>
      <button
        class="btn-secondary ml-auto shrink-0"
        :title="activeApiId ? '更新当前接口' : '保存为接口'"
        @click="saveApi"
      >
        保存
      </button>
    </div>

    <!-- 命名输入（保存新接口） -->
    <div v-if="saveNameOpen" class="flex shrink-0 items-center gap-[8px]">
      <input
        v-model="saveName"
        class="field-input max-w-[320px] font-mono"
        placeholder="接口名称，如：获取用户列表 / WS 推送通道"
        spellcheck="false"
        @keyup.enter="saveApi"
      />
      <button class="btn-primary" @click="saveApi">确定</button>
      <button class="btn-ghost" @click="saveNameOpen = false">取消</button>
    </div>

    <!-- 左侧接口列表 + 右侧面板 -->
    <div class="flex min-h-0 flex-1 gap-[12px]">
      <ApiSidebar
        :apis="apis"
        :active-id="activeApiId"
        @select="applyApi"
        @delete="deleteApi"
        @clear="clearApis"
        @new="newApi"
      />

      <div class="min-h-0 flex-1">
        <HttpPanel v-show="tab === 'http'" ref="httpPanel" class="h-full" />
        <WsPanel v-show="tab === 'ws'" ref="wsPanel" class="h-full" />
      </div>
    </div>
  </div>
</template>
