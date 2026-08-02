<script setup lang="ts">
/**
 * App.vue · M1 主界面（多页签工作区布局）
 * 侧栏 + 顶栏 + ToolWorkspace（工具库首页 / 工具页签子页面）；
 * 工具数据来自注册表（tools store 聚合），主题/收藏/最近使用持久化。
 */
import { computed, onMounted } from "vue";
import Sidebar from "@/features/sidebar/Sidebar.vue";
import TopBar from "@/features/topbar/TopBar.vue";
import ToolWorkspace from "@/features/workspace/ToolWorkspace.vue";
import TitleBar from "@/features/ui/TitleBar.vue";
import SettingsModal from "@/features/settings/SettingsModal.vue";
import Toast from "@/features/ui/Toast.vue";
import PickerView from "@/features/picker/PickerView.vue";
import { useFavoritesStore } from "@/stores/favorites";
import { useSettingsStore } from "@/stores/settings";
import { useToolsStore } from "@/stores/tools";

const tools = useToolsStore();
const favorites = useFavoritesStore();
const settings = useSettingsStore();

/** 屏幕取色窗口路由（picker 窗口加载 index.html#/picker） */
const isPicker = computed(() => window.location.hash === "#/picker");

onMounted(async () => {
  await Promise.all([favorites.init(), tools.initRecent(), settings.init()]);
});
</script>

<template>
  <!-- 取色窗口（#/picker 路由）：全屏遮罩，替代主 UI -->
  <PickerView v-if="isPicker" />
  <div v-else class="flex h-screen flex-col overflow-hidden">
    <TitleBar />
    <div class="flex min-h-0 flex-1">
      <Sidebar />
      <main class="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <ToolWorkspace />
      </main>
    </div>

    <SettingsModal />
    <Toast />
  </div>
</template>
