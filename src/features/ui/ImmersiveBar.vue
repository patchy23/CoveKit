<script setup lang="ts">
/**
 * ImmersiveBar · 沉浸模式顶部悬浮条
 * 平时为 8px 拖拽条（可移动窗口，不占操作空间）；
 * hover 展开为工具条：退出沉浸 + 窗口控制（最小化/最大化/关闭）。
 * 由 App.vue 在 ui.immersive 时渲染（absolute 定位，悬浮于内容区之上）。
 */
import { safeWindow } from "@/core/ui/windowCtl";
import { useUiStore } from "@/stores/ui";
import AppIcon from "@/features/ui/AppIcon.vue";

const ui = useUiStore();
</script>

<template>
  <div
    class="group absolute inset-x-0 top-0 z-50 h-[8px] transition-[height] duration-150 hover:h-[36px]"
    data-tauri-drag-region
  >
    <div
      class="flex h-full items-center justify-end gap-[2px] overflow-hidden border-b border-border bg-surface opacity-0 transition-opacity duration-150 group-hover:opacity-100 dark:border-border-dark dark:bg-surface-dark"
    >
      <span class="flex-1 pl-[12px] text-caption text-text-muted dark:text-text-muted-dark">
        沉浸模式 · 悬停此处恢复
      </span>
      <button
        class="grid h-full w-[80px] place-items-center text-body-sm font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="退出沉浸模式"
        @click="ui.toggleImmersive()"
      >
        <AppIcon name="immersive" :size="13" class="mr-[6px]" />
        退出
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最小化"
        @click="safeWindow((w) => w.minimize())"
      >
        <AppIcon name="minus" :size="14" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最大化"
        @click="safeWindow((w) => w.toggleMaximize())"
      >
        <AppIcon name="maximize" :size="12" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors hover:bg-red-500 hover:text-white"
        title="关闭（最小化到托盘）"
        @click="safeWindow((w) => w.close())"
      >
        <AppIcon name="close" :size="13" />
      </button>
    </div>
  </div>
</template>
