<script setup lang="ts">
/**
 * TitleBar · 内置标题栏（无边框窗口，2026-08-02 用户决策）
 * 背景随主题切换（bg-surface / dark:bg-surface-dark）；拖拽区移动窗口；
 * 双击标题栏最大化；右侧窗口控制按钮（关闭 = 最小化到托盘，见 Rust CloseRequested 处理）。
 * 额外提供：侧栏折叠切换（panel 按钮）与沉浸模式切换（expand 按钮，隐藏标题栏+侧栏）。
 */
import { runWindowAction, type WindowAction } from '@/core/platform/window'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import covekitIcon from '@/assets/covekit-icon-color.png'

const ui = useUiStore()

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
    <div class="flex items-center gap-[8px] px-[14px]" data-tauri-drag-region>
      <img :src="covekitIcon" alt="" class="h-[20px] w-[20px] rounded-[6px] object-cover" />
      <span class="text-body-sm font-medium text-secondary dark:text-secondary-dark">
        CoveKit
      </span>
    </div>
    <div class="flex-1" data-tauri-drag-region />
    <div class="flex h-full items-center">
      <button
        v-if="!ui.immersive"
        class="grid h-full w-[40px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        :class="
          ui.sidebarCollapsed
            ? 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark'
            : ''
        "
        :title="ui.sidebarCollapsed ? '展开侧栏' : '隐藏侧栏'"
        @click="ui.sidebarCollapsed = !ui.sidebarCollapsed"
      >
        <AppIcon name="panel" :size="15" />
      </button>
      <button
        class="grid h-full w-[40px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        :class="
          ui.immersive ? 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark' : ''
        "
        :title="ui.immersive ? '退出沉浸模式' : '沉浸模式（隐藏工具标题栏与侧栏）'"
        @click="ui.toggleImmersive()"
      >
        <AppIcon name="immersive" :size="14" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最小化"
        @click="onWindow('minimize')"
      >
        <AppIcon name="minus" :size="14" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最大化"
        @click="onWindow('toggleMaximize')"
      >
        <AppIcon name="maximize" :size="12" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-red-500 hover:text-white"
        title="关闭（最小化到托盘）"
        @click="onWindow('close')"
      >
        <AppIcon name="close" :size="13" />
      </button>
    </div>
  </div>
</template>
