<script setup lang="ts">
/**
 * App.vue · M1 主界面（多页签工作区布局）
 * 侧栏 + 顶栏 + ToolWorkspace（工具库首页 / 工具页签子页面）；
 * 工具数据来自注册表（tools store 聚合），主题/收藏/最近使用持久化。
 * 视图模式：侧栏可折叠（sidebarCollapsed）；沉浸模式隐藏工具标题栏 TopBar
 * （窗口标题栏 TitleBar 保留，退出靠其沉浸按钮）。
 *
 * 另承担两件框架级职责（可靠性 T10-3/T10-7）：
 * - 主窗口隐藏状态广播：窗口失焦/最小化/隐藏到托盘时通知工具降频刷新（不断连接、不停止任务）；
 * - 退出被拒绝提示：托盘退出时窗口会被后端唤到前台，这里弹「重试 / 强制退出 / 取消」。
 */
import { onMounted, onUnmounted } from 'vue'
import Sidebar from '@/features/sidebar/Sidebar.vue'
import TopBar from '@/features/topbar/TopBar.vue'
import ToolWorkspace from '@/features/workspace/ToolWorkspace.vue'
import AppExitDialog from '@/features/workspace/AppExitDialog.vue'
import SettingsPage from '@/features/settings/SettingsPage.vue'
import StorageRecoveryOverlay from '@/features/settings/StorageRecoveryOverlay.vue'
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

/**
 * 窗口隐藏状态来源一：页面可见性（最小化、切到其他虚拟桌面时浏览器层会更新）。
 *
 * 只影响刷新频率；隐藏后仍保持连接与任务（用户明确要求「失焦不自动断开」）。
 */
function onVisibilityChange() {
  ui.setWorkspaceHidden(document.visibilityState === 'hidden')
}

/** 窗口隐藏状态来源二：Tauri 窗口失焦/隐藏（隐藏到托盘时页面可见性不一定变化） */
let stopWindowWatch: (() => void) | null = null

async function watchWindowVisibility() {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const appWindow = getCurrentWindow()
    ui.setWorkspaceHidden(!(await appWindow.isFocused()) || document.visibilityState === 'hidden')
    stopWindowWatch = await appWindow.onFocusChanged(({ payload: focused }) => {
      ui.setWorkspaceHidden(!focused || document.visibilityState === 'hidden')
    })
  } catch (error) {
    // 浏览器预览环境没有窗口 API：退化为只用页面可见性，不阻断界面
    console.warn('[app] 窗口焦点监听不可用，改用页面可见性', error)
  }
}

onMounted(async () => {
  document.addEventListener('visibilitychange', onVisibilityChange)
  onVisibilityChange()
  await watchWindowVisibility()
  await Promise.all([favorites.init(), tools.initRecent(), settings.init()])
})

onUnmounted(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
  stopWindowWatch?.()
  stopWindowWatch = null
})
</script>

<template>
  <div class="relative flex h-screen flex-col overflow-hidden">
    <TitleBar />
    <div class="flex min-h-0 flex-1">
      <Sidebar v-if="!ui.sidebarCollapsed" />
      <main class="flex min-w-0 flex-1 flex-col">
        <TopBar v-if="!ui.immersive" v-show="!ui.settingsOpen" />
        <!-- 设置页：框架级整页模式（v-show 保留工作区页签状态） -->
        <SettingsPage v-if="ui.settingsOpen" />
        <ToolWorkspace v-show="!ui.settingsOpen" />
      </main>
    </div>

    <!-- 存储恢复页：配置盘不可用或上次迁移失败时覆盖整个窗口，必须用户显式选择动作 -->
    <StorageRecoveryOverlay />

    <!-- 退出被拒绝提示：不允许「点了退出没反应」 -->
    <AppExitDialog />

    <Toast />
  </div>
</template>
