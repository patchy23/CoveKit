<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * TitleBar · 内置标题栏（无边框窗口，2026-08-02 用户决策）
 * 背景随主题切换（bg-surface / dark:bg-surface-dark）；拖拽区移动窗口；
 * 双击标题栏最大化；右侧窗口控制按钮（关闭 = 最小化到托盘，见 Rust CloseRequested 处理）。
 * standalone 复用外观与窗口控制，关闭交由该窗口的关闭协商，不展示主工作区控制。
 * 额外提供：侧栏折叠切换（panel 按钮）与沉浸模式切换（expand 按钮，隐藏标题栏+侧栏）。
 */
import { runWindowAction, type WindowAction } from '@/core/platform/window'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import covekitIcon from '@/assets/covekit-icon-color.png'
import { useI18n } from 'vue-i18n'
import { ipc } from '@/core/ipc/ipc'
import { PROJECT_URL } from '@/core/project'

withDefaults(defineProps<{ standalone?: boolean; title?: string }>(), { title: 'CoveKit' })
const ui = useUiStore()
const { t } = useI18n()
async function openGitHub() {
  try {
    await ipc.openExternal(PROJECT_URL)
  } catch {
    ui.toast(t('settings.openLinkFailed'))
  }
}

/** 窗口动作统一入口：桌面环境下失败必须可见，浏览器预览的不可用属预期缺失不打扰 */
function onWindow(action: WindowAction) {
  void runWindowAction(action).then((result) => {
    if (!result.ok && result.reason === 'failed') ui.toast('窗口操作失败，请重试')
  })
}

function onDblClick() {
  onWindow('toggleMaximize')
}
</script>

<template>
  <div
    class="flex h-[36px] shrink-0 select-none items-center border-b border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    data-tauri-drag-region
    @dblclick="onDblClick"
  >
    <div class="flex min-w-0 items-center gap-[8px] px-[14px]" data-tauri-drag-region>
      <img
        :src="covekitIcon"
        alt=""
        class="h-[20px] w-[20px] shrink-0 rounded-[6px] object-cover"
      />
      <span class="truncate text-body-sm font-medium text-secondary dark:text-secondary-dark">
        {{ title }}
      </span>
    </div>
    <div class="flex-1" data-tauri-drag-region />
    <div class="flex h-full shrink-0 items-center">
      <UiTooltip v-if="!standalone" :content="t('settings.openGitHub')">
        <button
          class="mr-sm grid h-full w-[40px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :aria-label="t('settings.openGitHub')"
          @dblclick.stop
          @click="openGitHub"
        >
          <AppIcon name="github" :size="16" />
        </button>
      </UiTooltip>
      <UiTooltip
        v-if="!standalone && !ui.immersive"
        :content="ui.sidebarCollapsed ? '展开侧栏' : '隐藏侧栏'"
      >
        <button
          class="grid h-full w-[40px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :class="
            ui.sidebarCollapsed
              ? 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark'
              : ''
          "
          :aria-label="ui.sidebarCollapsed ? '展开侧栏' : '隐藏侧栏'"
          @click="ui.sidebarCollapsed = !ui.sidebarCollapsed"
        >
          <AppIcon name="panel" :size="15" />
        </button>
      </UiTooltip>
      <UiTooltip
        v-if="!standalone"
        :content="ui.immersive ? '退出沉浸模式' : '沉浸模式（隐藏工具标题栏与侧栏）'"
      >
        <button
          class="grid h-full w-[40px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :class="
            ui.immersive ? 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark' : ''
          "
          :aria-label="ui.immersive ? '退出沉浸模式' : '沉浸模式（隐藏工具标题栏与侧栏）'"
          @click="ui.toggleImmersive()"
        >
          <AppIcon name="immersive" :size="14" />
        </button>
      </UiTooltip>
      <UiTooltip content="最小化">
        <button
          class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          aria-label="最小化"
          @click="onWindow('minimize')"
        >
          <AppIcon name="minus" :size="14" />
        </button>
      </UiTooltip>
      <UiTooltip content="最大化">
        <button
          class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          aria-label="最大化"
          @click="onWindow('toggleMaximize')"
        >
          <AppIcon name="maximize" :size="12" />
        </button>
      </UiTooltip>
      <UiTooltip :content="standalone ? '关闭' : '关闭（最小化到托盘）'">
        <button
          class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-red-500 hover:text-white"
          :aria-label="standalone ? '关闭' : '关闭（最小化到托盘）'"
          @click="onWindow('close')"
        >
          <AppIcon name="close" :size="13" />
        </button>
      </UiTooltip>
    </div>
  </div>
</template>
