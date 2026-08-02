<script setup lang="ts">
/**
 * ToolWorkspace · 多页签工作区（用户决策：工具以子页面形式打开，替代弹窗）
 * 页签条 + 内容区：首页（工具库）/ 各工具页签（v-show 保持组件状态，切换不销毁）。
 * 页签过多时：新页签在首页后第一位，超出显示宽度的页签收纳进「···」下拉。
 */
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, type Component } from "vue";
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

/* ── 页签溢出收纳：按页签条宽度估算可见页签数，其余进「···」下拉 ── */
// 估算值取页签上限宽（图标15 + 间距 + 文字120 + 关闭18 + padding ≈ 190px），
// 并为「首页」与「···」按钮预留固定空间，避免溢出时省略号不出现。
const MIN_TAB_WIDTH = 190; // px
const RESERVED_WIDTH = 150; // px（首页页签 + 溢出按钮 + 尾部留白）
const tabBar = ref<HTMLElement | null>(null);
const overflowOpen = ref(false);
const visibleTabCount = ref(5);
let ro: ResizeObserver | null = null;

function calcVisible() {
  const w = tabBar.value?.clientWidth ?? 0;
  visibleTabCount.value = Math.max(1, Math.floor((w - RESERVED_WIDTH) / MIN_TAB_WIDTH));
}

/** 页签条直接显示的页签（首页占 1 个位置） */
const visibleTabs = computed(() => ui.openTabs.slice(0, Math.max(0, visibleTabCount.value - 1)));
/** 收纳进下拉的页签 */
const hiddenTabs = computed(() => ui.openTabs.slice(Math.max(0, visibleTabCount.value - 1)));

function onDocMouseDown() {
  overflowOpen.value = false;
}

onMounted(() => {
  calcVisible();
  ro = new ResizeObserver(calcVisible);
  if (tabBar.value) ro.observe(tabBar.value);
  document.addEventListener("mousedown", onDocMouseDown);
});

onUnmounted(() => {
  ro?.disconnect();
  document.removeEventListener("mousedown", onDocMouseDown);
});
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <!-- 页签条 -->
    <div
      ref="tabBar"
      class="flex shrink-0 items-center gap-[2px] overflow-hidden border-b border-border bg-surface-muted px-sm dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <button
        class="relative flex h-[38px] shrink-0 items-center gap-[8px] rounded-t-[8px] px-[14px] text-body font-medium transition-colors"
        :class="
          ui.activeTab === null
            ? 'bg-surface text-primary dark:bg-surface-dark dark:text-primary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        title="回到工具库首页"
        @click="ui.goHome()"
      >
        <!-- 选中指示器：纯横线（独立元素，不随圆角弯曲） -->
        <span
          v-if="ui.activeTab === null"
          class="absolute inset-x-0 top-0 h-[2px] bg-tertiary-strong dark:bg-tertiary-dark"
        />
        <AppIcon name="all" :size="15" />
        首页
      </button>
      <div
        v-for="id in visibleTabs"
        :key="id"
        class="group relative flex h-[38px] shrink-0 cursor-pointer items-center gap-[8px] rounded-t-[8px] px-[12px] text-body font-medium transition-colors"
        :class="
          ui.activeTab === id
            ? 'bg-surface text-primary dark:bg-surface-dark dark:text-primary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        @click="ui.openTool(id)"
      >
        <!-- 选中指示器：纯横线（独立元素，不随圆角弯曲） -->
        <span
          v-if="ui.activeTab === id"
          class="absolute inset-x-0 top-0 h-[2px] bg-tertiary-strong dark:bg-tertiary-dark"
        />
        <AppIcon
          :name="tabIcon(id)"
          :size="15"
          class="text-tertiary-strong dark:text-tertiary-dark"
        />
        <span class="max-w-[120px] truncate">{{ tabTitle(id) }}</span>
        <button
          class="grid h-[18px] w-[18px] shrink-0 place-items-center rounded-[4px] text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
          title="关闭页签"
          @click.stop="ui.closeTab(id)"
        >
          <AppIcon name="close" :size="11" />
        </button>
      </div>

      <!-- 溢出省略号：显示未收纳的页签 -->
      <div v-if="hiddenTabs.length" class="relative">
        <button
          class="flex h-[38px] shrink-0 items-center gap-[4px] rounded-t-[8px] px-[10px] text-body font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :title="`更多页签（${hiddenTabs.length}）`"
          @mousedown.stop
          @click.stop="overflowOpen = !overflowOpen"
        >
          ···
        </button>
        <div
          v-if="overflowOpen"
          class="absolute left-0 top-full z-50 mt-[4px] max-h-[320px] w-[230px] overflow-y-auto rounded-lg border border-border bg-surface py-[4px] shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
        >
          <div
            v-for="id in hiddenTabs"
            :key="id"
            class="flex cursor-pointer items-center gap-[10px] px-[10px] py-[8px] text-body transition-colors hover:bg-border"
            :class="
              ui.activeTab === id
                ? 'bg-tertiary-soft font-medium text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
                : 'text-secondary dark:text-secondary-dark'
            "
            @click="
              ui.openTool(id);
              overflowOpen = false;
            "
          >
            <AppIcon
              :name="tabIcon(id)"
              :size="15"
              class="shrink-0 text-tertiary-strong dark:text-tertiary-dark"
            />
            <span class="min-w-0 flex-1 truncate">{{ tabTitle(id) }}</span>
            <button
              class="grid h-[18px] w-[18px] shrink-0 place-items-center rounded-[4px] text-text-muted hover:bg-border hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:bg-border-dark"
              title="关闭页签"
              @click.stop="ui.closeTab(id)"
            >
              <AppIcon name="close" :size="11" />
            </button>
          </div>
        </div>
      </div>
      <div class="flex-1" />
    </div>

    <!-- 内容区 -->
    <div class="min-h-0 flex-1 overflow-y-auto px-xl py-lg">
      <!-- 首页：工具库 -->
      <div v-show="ui.activeTab === null">
        <RecentStrip />
        <div class="mb-[12px] flex items-center gap-sm">
          <h2 class="text-h2 font-bold tracking-[-0.01em] dark:text-primary-dark">工具列表</h2>
          <span class="text-body-sm text-text-muted dark:text-text-muted-dark"
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
          <p class="mt-md text-h2 font-bold dark:text-primary-dark">
            {{ searching ? "未找到匹配工具" : hasTools ? "该分类暂无工具" : "暂无工具" }}
          </p>
          <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
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

      <!-- 工具页签（v-show 保持状态，切换不销毁；h-full 让工具可内部滚动） -->
      <div v-for="id in ui.openTabs" v-show="ui.activeTab === id" :key="id" class="h-full">
        <component :is="compFor(id)" v-if="compFor(id)" />
      </div>
    </div>
  </div>
</template>
