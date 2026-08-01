<script setup lang="ts">
/**
 * TopBar · 顶栏
 * 首页：分类标题 + 计数副标题 + 搜索框 + 视图切换 + 添加按钮；
 * 工具页签激活：显示工具名 + 描述（搜索框聚焦时自动回到工具库）。
 */
import { computed } from "vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import { getTool } from "@/core/registry/toolRegistry";
import { useToolsStore } from "@/stores/tools";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const tools = useToolsStore();

const titleMap: Record<string, string> = {
  all: "全部工具",
  dev: "开发工具",
  text: "文本处理",
  image: "图片工具",
  net: "网络工具",
  sys: "系统工具",
  fav: "我的收藏",
};

const activeTool = computed(() => (ui.activeTab ? getTool(ui.activeTab) : undefined));
const title = computed(() => activeTool.value?.name ?? titleMap[ui.activeCategory] ?? "全部工具");
const subtitle = computed(() =>
  activeTool.value
    ? activeTool.value.description
    : `共 ${tools.filtered.length} 个工具 · 点击卡片即可使用`
);

/** 聚焦搜索时若在工具页签，切回工具库首页 */
function onSearchFocus() {
  if (ui.activeTab) ui.goHome();
}
</script>

<template>
  <header
    class="flex h-[66px] shrink-0 items-center gap-md border-b border-border bg-surface px-xl dark:border-border-dark dark:bg-surface-dark"
  >
    <div class="min-w-0">
      <h1 class="truncate text-[17px] font-bold tracking-[-0.02em] dark:text-primary-dark">
        {{ title }}
      </h1>
      <p class="mt-[1px] truncate text-[12px] text-text-muted dark:text-text-muted-dark">
        {{ subtitle }}
      </p>
    </div>
    <div class="flex-1" />
    <div
      class="flex h-[38px] w-[280px] items-center gap-sm rounded-md border border-border bg-neutral px-[12px] transition-all duration-200 focus-within:w-[320px] focus-within:border-tertiary focus-within:shadow-[0_0_0_3px_var(--color-tertiary-soft)] dark:border-border-dark dark:bg-neutral-dark"
    >
      <AppIcon
        name="search"
        :size="15"
        class="shrink-0 text-text-muted dark:text-text-muted-dark"
      />
      <input
        v-model="ui.searchQuery"
        class="flex-1 bg-transparent text-[13px] text-primary outline-none placeholder:text-text-muted dark:text-primary-dark dark:placeholder:text-text-muted-dark"
        type="text"
        placeholder="搜索工具…"
        spellcheck="false"
        @focus="onSearchFocus"
      />
    </div>
    <button
      v-if="!activeTool"
      class="grid h-[38px] w-[38px] shrink-0 place-items-center rounded-md border border-border bg-surface text-secondary transition-colors duration-150 hover:border-border-strong hover:bg-surface-muted hover:text-primary dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark dark:hover:border-border-strong-dark dark:hover:bg-surface-muted-dark dark:hover:text-primary-dark"
      title="视图切换"
      @click="ui.listView = !ui.listView"
    >
      <AppIcon :name="ui.listView ? 'list' : 'grid'" :size="17" />
    </button>
    <button
      v-if="!activeTool"
      class="grid h-[38px] w-[38px] shrink-0 place-items-center rounded-md bg-tertiary text-on-tertiary transition-[filter] duration-150 hover:brightness-110 dark:bg-tertiary-dark dark:text-on-tertiary-dark"
      title="添加工具（M1 开放）"
    >
      <AppIcon name="add" :size="17" />
    </button>
  </header>
</template>
