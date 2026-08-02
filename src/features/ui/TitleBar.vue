<script setup lang="ts">
/**
 * TitleBar · 内置标题栏（无边框窗口，2026-08-02 用户决策）
 * 背景随主题切换（bg-surface / dark:bg-surface-dark）；拖拽区移动窗口；
 * 双击标题栏最大化；右侧窗口控制按钮（关闭 = 最小化到托盘，见 Rust CloseRequested 处理）。
 */
import { getCurrentWindow, type Window } from "@tauri-apps/api/window";
import AppIcon from "@/features/ui/AppIcon.vue";

const isTauri = "__TAURI_INTERNALS__" in window;

/** 非 Tauri 环境（浏览器预览）惰性获取窗口句柄，取不到则窗口操作静默跳过 */
function windowCtl(): Window | null {
  if (!isTauri) return null;
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

function safe(fn: (w: Window) => Promise<unknown>) {
  const w = windowCtl();
  if (!w) return;
  fn(w).catch((e) => console.warn("[TitleBar] 窗口操作失败（检查 capabilities 权限）:", e));
}

function onDblClick() {
  safe((w) => w.toggleMaximize());
}
</script>

<template>
  <div
    class="flex h-[36px] shrink-0 select-none items-center border-b border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    data-tauri-drag-region
    @dblclick="onDblClick"
  >
    <div class="flex items-center gap-[8px] px-[14px]" data-tauri-drag-region>
      <div
        class="grid h-[20px] w-[20px] place-items-center rounded-[6px] bg-gradient-to-br from-tertiary to-tertiary-strong text-[10px] font-extrabold text-on-tertiary"
      >
        P
      </div>
      <span class="text-body-sm font-medium text-secondary dark:text-secondary-dark">
        patchyBox
      </span>
    </div>
    <div class="flex-1" data-tauri-drag-region />
    <div class="flex h-full items-center">
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最小化"
        @click="safe((w) => w.minimize())"
      >
        <AppIcon name="minus" :size="14" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="最大化"
        @click="safe((w) => w.toggleMaximize())"
      >
        <AppIcon name="maximize" :size="12" />
      </button>
      <button
        class="grid h-full w-[44px] place-items-center text-text-muted transition-colors duration-100 hover:bg-red-500 hover:text-white"
        title="关闭（最小化到托盘）"
        @click="safe((w) => w.close())"
      >
        <AppIcon name="close" :size="13" />
      </button>
    </div>
  </div>
</template>
