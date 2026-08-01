<script setup lang="ts">
/**
 * App.vue · M1 主界面（组件树重组）
 * 侧栏 + 顶栏 + 最近使用 + 网格/列表 + 弹窗 + 设置 + Toast；
 * 工具数据来自注册表（tools store 聚合），空状态区分「无工具」与「搜索无结果」。
 */
import { computed, onMounted } from "vue";
import Sidebar from "@/features/sidebar/Sidebar.vue";
import TopBar from "@/features/topbar/TopBar.vue";
import RecentStrip from "@/features/recent/RecentStrip.vue";
import ToolGrid from "@/features/grid/ToolGrid.vue";
import ToolList from "@/features/grid/ToolList.vue";
import ToolModal from "@/features/modal/ToolModal.vue";
import SettingsModal from "@/features/settings/SettingsModal.vue";
import Toast from "@/features/ui/Toast.vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import { useFavoritesStore } from "@/stores/favorites";
import { useSettingsStore } from "@/stores/settings";
import { useToolsStore } from "@/stores/tools";
import { useUiStore } from "@/stores/ui";

const tools = useToolsStore();
const favorites = useFavoritesStore();
const settings = useSettingsStore();
const ui = useUiStore();

const hasTools = computed(() => tools.tools.length > 0);
const searching = computed(() => ui.searchQuery.trim().length > 0);

onMounted(async () => {
  await Promise.all([favorites.init(), tools.initRecent(), settings.init()]);
});
</script>

<template>
  <div class="flex h-screen overflow-hidden">
    <Sidebar />
    <main class="flex min-w-0 flex-1 flex-col">
      <TopBar />
      <div class="flex-1 overflow-y-auto px-xl py-lg">
        <RecentStrip />
        <div class="mb-[12px] flex items-center gap-sm">
          <h2 class="text-[14px] font-bold tracking-[-0.01em] dark:text-primary-dark">工具列表</h2>
          <span class="text-[12px] text-text-muted dark:text-text-muted-dark"
            >{{ tools.filtered.length }} 个</span
          >
        </div>

        <ToolGrid v-if="tools.filtered.length && !ui.listView" />
        <ToolList v-else-if="tools.filtered.length && ui.listView" />

        <!-- 空状态 -->
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
    </main>

    <ToolModal />
    <SettingsModal />
    <Toast />
  </div>
</template>
