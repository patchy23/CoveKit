<script setup lang="ts">
/**
 * 剪贴板历史 · 列表/搜索/置顶/复制/删除/清空
 * 数据来自 M1 就绪的 Rust 轮询服务（SQLite 落库），3s 自动刷新。
 */
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { ClipboardRecord } from "@/core/ipc/contracts";
import { ipc } from "@/core/ipc/ipc";
import { useCopy } from "@/tools/shared/useClipboard";
import AppIcon from "@/features/ui/AppIcon.vue";
import { useUiStore } from "@/stores/ui";
import { filterRecords, formatTime, previewText } from "./useClipboardHistory";

const { copyText } = useCopy();
const ui = useUiStore();

const records = ref<ClipboardRecord[]>([]);
const query = ref("");
const loading = ref(false);
const error = ref("");

const filtered = computed(() => filterRecords(records.value, query.value));

async function load() {
  try {
    loading.value = true;
    records.value = await ipc.clipboardList(200);
    error.value = "";
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function copyRecord(r: ClipboardRecord) {
  await copyText(r.content, "已复制到剪贴板");
}

async function togglePin(r: ClipboardRecord) {
  await ipc.clipboardTogglePin(r.id);
  r.pinned = !r.pinned;
  ui.toast(r.pinned ? "已置顶" : "已取消置顶");
}

async function remove(r: ClipboardRecord) {
  await ipc.clipboardDelete(r.id);
  records.value = records.value.filter((x) => x.id !== r.id);
  ui.toast("已删除");
}

async function clearAll() {
  await ipc.clipboardClear();
  records.value = [];
  ui.toast("已清空剪贴板历史");
}

let timer: ReturnType<typeof setInterval> | null = null;
onMounted(() => {
  load();
  timer = setInterval(load, 3000);
});
onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div class="flex max-w-[860px] flex-col gap-[12px]">
    <!-- 固定工具栏（搜索/刷新/清空，滚动时始终可见） -->
    <div class="sticky-toolbar !py-[10px]">
      <div
        class="flex h-[34px] min-w-[200px] flex-1 items-center gap-sm rounded-md border border-border bg-surface-muted px-[10px] dark:border-border-dark dark:bg-surface-muted-dark"
      >
        <AppIcon
          name="search"
          :size="14"
          class="shrink-0 text-text-muted dark:text-text-muted-dark"
        />
        <input
          v-model="query"
          class="flex-1 bg-transparent text-body text-primary outline-none placeholder:text-text-muted dark:text-primary-dark dark:placeholder:text-text-muted-dark"
          type="text"
          placeholder="搜索剪贴板历史…"
          spellcheck="false"
        />
      </div>
      <button class="btn-ghost" title="刷新" @click="load">刷新</button>
      <button v-if="records.length" class="btn-ghost" @click="clearAll">清空</button>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ filtered.length }} 条
      </span>
    </div>

    <p v-if="error" class="text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>
    <p
      v-if="loading && !records.length"
      class="text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      加载中…
    </p>

    <!-- 列表 -->
    <div v-if="filtered.length" class="flex flex-col gap-[8px]">
      <div
        v-for="r in filtered"
        :key="r.id"
        class="flex items-start gap-[12px] rounded-md border border-border bg-surface px-[14px] py-[10px] transition-colors hover:border-border-strong dark:border-border-dark dark:bg-surface-dark dark:hover:border-border-strong-dark"
        :class="r.pinned ? 'border-tertiary/40 dark:border-tertiary-dark/40' : ''"
      >
        <button
          class="mt-[2px] shrink-0 text-text-muted transition-colors hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:text-tertiary-dark"
          :title="r.pinned ? '取消置顶' : '置顶'"
          @click="togglePin(r)"
        >
          <AppIcon
            name="pin"
            :size="15"
            :class="r.pinned ? 'text-tertiary-strong dark:text-tertiary-dark' : ''"
          />
        </button>
        <button class="min-w-0 flex-1 text-left" :title="r.content" @click="copyRecord(r)">
          <div
            class="truncate font-mono text-body leading-relaxed text-primary dark:text-primary-dark"
          >
            {{ previewText(r.content) }}
          </div>
          <div
            class="mt-[2px] flex items-center gap-[10px] text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            <span>{{ formatTime(r.createdAt) }}</span>
            <span class="truncate">{{ r.content.length }} 字符</span>
          </div>
        </button>
        <button class="btn-ghost shrink-0" title="删除此条" @click="remove(r)">删除</button>
      </div>
    </div>

    <!-- 空状态 -->
    <div v-else-if="!loading" class="flex flex-col items-center py-[64px] text-center">
      <div
        class="grid h-11 w-11 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
      >
        <AppIcon name="clipboard" :size="22" class="text-tertiary-strong dark:text-tertiary-dark" />
      </div>
      <p class="mt-md text-h2 font-bold dark:text-primary-dark">
        {{ query ? "未找到匹配记录" : "暂无剪贴板历史" }}
      </p>
      <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ query ? "换个关键词试试" : "复制任意文本后会自动记录在这里" }}
      </p>
    </div>
  </div>
</template>
