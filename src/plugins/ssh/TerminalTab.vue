<script setup lang="ts">
/**
 * TerminalTab · SSH 终端子页签
 * 当前为纯前端演示：xterm.js 占位 + 假数据回显，后端 IPC 接入后替换为真实数据流。
 */
import { onMounted, ref } from "vue";
import type { ServerConnection, ServerProfile, TerminalSession } from "./contracts";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
  terminals: TerminalSession[];
}>();

const terminalEl = ref<HTMLElement | null>(null);

onMounted(() => {
  // TODO: 接入 xterm.js 与 Tauri IPC 终端数据流
  // 当前仅展示占位与假数据
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 终端工具栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 终端
      </span>
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        80×24 UTF-8
      </span>
      <div class="ml-auto flex gap-[6px]">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption" title="清空终端">清空</button>
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption" title="重新连接">重连</button>
      </div>
    </div>

    <!-- 终端区域（xterm.js 挂载点） -->
    <div
      ref="terminalEl"
      class="min-h-0 flex-1 select-text overflow-auto bg-surface p-[12px] font-mono text-body-sm leading-relaxed text-primary dark:bg-surface-dark dark:text-primary-dark"
    >
      <template v-if="connection?.status === 'connected'">
        <p class="font-sans text-success-strong dark:text-success-dark">
          ● 已连接到 {{ connection.host }}（延迟 {{ connection.latencyMs }}ms）
        </p>
        <p class="mt-[8px]">$ whoami</p>
        <p>{{ profile?.username ?? "root" }}</p>
        <p class="mt-[4px]">$ ls -la /var/log/</p>
        <p>total 128</p>
        <p>drwxr-xr-x 8 root root 4096 Aug 1 12:00 .</p>
        <p>drwxr-xr-x 3 root root 4096 Aug 1 12:00 ..</p>
        <p>-rw-r--r-- 1 root root 12.4M Aug 8 14:32 access.log</p>
        <p>-rw-r--r-- 1 root root 3.2M Aug 8 14:32 error.log</p>
        <p class="mt-[8px]">$ <span class="animate-pulse">█</span></p>
      </template>
      <template v-else>
        <p class="font-sans text-text-muted dark:text-text-muted-dark">
          未连接。请从左侧选择服务器并点击「连接」。
        </p>
      </template>
    </div>

    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>终端 {{ terminals.length }} 个</span>
      <span>SSH 通道复用</span>
      <span class="ml-auto">{{ connection?.status === "connected" ? "就绪" : "未连接" }}</span>
    </div>
  </div>
</template>
