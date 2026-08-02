<script setup lang="ts">
/**
 * HTTP/WS 调试 · 请求构建 + WebSocket 长连接测试（第二批首个大工具）
 */
import { ref } from "vue";
import HttpPanel from "./HttpPanel.vue";
import WsPanel from "./WsPanel.vue";

const tab = ref<"http" | "ws">("http");
</script>

<template>
  <div class="flex h-full w-full flex-col gap-[12px]">
    <!-- 固定面板切换（滚动时始终可见） -->
    <div class="sticky-toolbar !py-[10px]">
      <button
        class="rounded-md px-[14px] py-[7px] text-body font-medium transition-colors"
        :class="
          tab === 'http'
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
        "
        @click="tab = 'http'"
      >
        HTTP 请求
      </button>
      <button
        class="rounded-md px-[14px] py-[7px] text-body font-medium transition-colors"
        :class="
          tab === 'ws'
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
        "
        @click="tab = 'ws'"
      >
        WebSocket
      </button>
    </div>
    <HttpPanel v-show="tab === 'http'" />
    <WsPanel v-show="tab === 'ws'" />
  </div>
</template>
