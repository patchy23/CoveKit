<script setup lang="ts">
/**
 * App.vue · M1 主界面（多页签工作区布局）
 * 侧栏 + 顶栏 + ToolWorkspace（工具库首页 / 工具页签子页面）；
 * 工具数据来自注册表（tools store 聚合），主题/收藏/最近使用持久化。
 * 视图模式：侧栏可折叠（sidebarCollapsed）；沉浸模式隐藏工具标题栏 TopBar
 * （窗口标题栏 TitleBar 保留，退出靠其沉浸按钮）。
 */
import { onMounted } from 'vue'
import Sidebar from '@/features/sidebar/Sidebar.vue'
import TopBar from '@/features/topbar/TopBar.vue'
import ToolWorkspace from '@/features/workspace/ToolWorkspace.vue'
import TitleBar from '@/features/ui/TitleBar.vue'
import Toast from '@/features/ui/Toast.vue'
import { useFavoritesStore } from '@/stores/favorites'
import { useSettingsStore } from '@/stores/settings'
import { useToolsStore } from '@/stores/tools'
import { useUiStore } from '@/stores/ui'

const tools = useToolsStore()
const favorites = useFavoritesStore()
const settings = useSettingsStore()
const ui = useUiStore()

onMounted(async () => {
  await Promise.all([favorites.init(), tools.initRecent(), settings.init()])
})
</script>

<template>
  <div class="relative flex h-screen flex-col overflow-hidden">
    <TitleBar />
    <div class="flex min-h-0 flex-1">
      <Sidebar v-if="!ui.sidebarCollapsed" />
      <main class="flex min-w-0 flex-1 flex-col">
        <TopBar v-if="!ui.immersive" />
        <ToolWorkspace />
      </main>
    </div>

    <Toast />
  </div>
</template>
