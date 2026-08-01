<script setup lang="ts">
/**
 * ToolWorkspace · 多页签工作区（用户决策：工具以子页面形式打开，替代弹窗）
 * 页签条 + 内容区：首页（工具库）/ 各工具页签（v-show 保持组件状态，切换不销毁）。
 * 架构关系：workspace 载体（core/presentation）的具体 UI，第二批大工具复用同一载体。
 */
import { computed, defineAsyncComponent, type Component } from "vue";
import { getTool } from "@/core/registry/toolRegistry";
import AppIcon from "@/features/ui/AppIcon.vue";
import RecentStrip from "@/features/recent/RecentStrip.vue";
import ToolGrid from "@/features/grid/ToolGrid.vue";
import ToolList from "@/features/grid/ToolList.vue";
import { useToolsStore } from "@/stores/tools";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const tools = useToolsStore();

const hasTools = computed(() => tools.tools.length > 0);
const searching = computed(() => ui.searchQuery.trim().length > 0);

// 页签组件缓存：同一工具只创建一次异步组件（v-show 保持实例状态）
const compCache = new Map<string, Component>();
function compFor(id: string): Component | null {
  const m = getTool(id);
  if (!m) return null;
  if (!compCache.has(id)) compCache.set(id, defineAsyncComponent(m.component));
  return compCache.get(id)!;
}

function tabTitle(id: string) {
  return getTool(id)?.name ?? id;
}

function tabIcon(id: string) {
  return getTool(id)?.icon ?? "all";
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <!-- 页签条 -->
    <div
      class="flex shrink-0 items-center gap-[2px] border-b border-border bg-surface-muted px-sm dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <button
        class="flex h-[38px] items-center gap-[8px] rounded-t-[8px] px-[14px] text-[13px] font-medium transition-colors"
        :class="
          ui.activeTab === null
            ? 'bg-surface text-primary shadow-[inset_0_2px_0_0_var(--color-tertiary)] dark:bg-surface-dark dark:text-primary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        title="工具库"
        @click="ui.goHome()"
      >
        <AppIcon name="all" :size="15" />
        工具库
      </button>
      <div
        v-for="id in ui.openTabs"
        :key="id"
        class="group flex h-[38px] cursor-pointer items-center gap-[8px] rounded-t-[8px] px-[12px] text-[13px] font-medium transition-colors"
        :class="
          ui.activeTab === id
            ? 'bg-surface text-primary shadow-[inset_0_2px_0_0_var(--color-tertiary)] dark:bg-surface-dark dark:text-primary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        @click="ui.openTool(id)"
      >
        <AppIcon
          :name="tabIcon(id)"
          :size="15"
          class="text-tertiary-strong dark:text-tertiary-dark"
        />
        {{ tabTitle(id) }}
        <button
          class="grid h-[18px] w-[18px] place-items-center rounded-[4px] text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
          title="关闭页签"
          @click.stop="ui.closeTab(id)"
        >
          <AppIcon name="close" :size="11" />
        </button>
      </div>
      <div class="flex-1" />
    </div>

    <!-- 内容区 -->
    <div class="min-h-0 flex-1 overflow-y-auto px-xl py-lg">
      <!-- 首页：工具库 -->
      <div v-show="ui.activeTab === null">
        <RecentStrip />
        <div class="mb-[12px] flex items-center gap-sm">
          <h2 class="text-[14px] font-bold tracking-[-0.01em] dark:text-primary-dark">工具列表</h2>
          <span class="text-[12px] text-text-muted dark:text-text-muted-dark"
            >{{ tools.filtered.length }} 个</span
          >
        </div>
        <ToolGrid v-if="tools.filtered.length && !ui.listView" />
        <ToolList v-else-if="tools.filtered.length && ui.listView" />
        <div v-else class="flex flex-col items-center justify-center py-[96px] text-center">
          <div
            class="grid h-11 w-11 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
          >
            <AppIcon
              :name="searching ? 'search' : 'all'"
              :size="22"
              class="text-tertiary-strong dark:text-tertiary-dark"
            />
          </div>
          <p class="mt-md text-[14px] font-bold dark:text-primary-dark">
            {{ searching ? "未找到匹配工具" : hasTools ? "该分类暂无工具" : "暂无工具" }}
          </p>
          <p class="mt-xs text-[12px] text-text-muted dark:text-text-muted-dark">
            {{
              searching
                ? "换个关键词试试"
                : hasTools
                  ? "工具将在此分类上线"
                  : "首批 8 个文本工具将在 M1 上线"
            }}
          </p>
        </div>
      </div>

      <!-- 工具页签（v-show 保持状态，切换不销毁） -->
      <div v-for="id in ui.openTabs" v-show="ui.activeTab === id" :key="id">
        <component :is="compFor(id)" v-if="compFor(id)" />
      </div>
    </div>
  </div>
</template>
