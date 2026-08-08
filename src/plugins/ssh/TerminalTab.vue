<script setup lang="ts">
/**
 * TerminalTab · SSH 交互式终端（xterm.js）
 * 连接会话 → 打开 PTY 通道；输出走 ssh://terminal-data 事件推送，
 * 输入走 ssh_terminal_write；ResizeObserver 同步窗口大小。
 */
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Terminal } from "xterm";
import { FitAddon } from "@xterm/addon-fit";
import "xterm/css/xterm.css";
import type { ServerConnection, ServerProfile } from "./contracts";
import { useUiStore } from "@/stores/ui";
import { ipc, onTerminalData } from "./ipc";

const props = defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const termHost = ref<HTMLDivElement | null>(null);
const statusLine = ref("未连接");

let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let terminalId: string | null = null;
let unlistenData: (() => void) | null = null;
let resizeObserver: ResizeObserver | null = null;

/** 打开终端通道（连接建立/重连时调用） */
async function openTerminal() {
  if (!props.connection?.sessionId || !term) return;
  try {
    const cols = term.cols;
    const rows = term.rows;
    const t = await ipc.sshTerminalOpen({
      connectionId: props.connection.sessionId,
      cols,
      rows,
    });
    terminalId = t.id;
    statusLine.value = `已连接 ${props.connection.host ?? ""} · ${t.cols}×${t.rows}`;
    ui.toast("终端已打开");
  } catch (e) {
    statusLine.value = `终端打开失败：${e}`;
    ui.toast(`终端打开失败：${e}`);
  }
}

/** 关闭终端通道（断开/组件卸载时调用） */
async function closeTerminal() {
  if (terminalId) {
    try {
      await ipc.sshTerminalClose(terminalId);
    } catch {
      /* 通道可能已被服务端关闭 */
    }
    terminalId = null;
  }
}

async function reconnect() {
  await closeTerminal();
  term?.reset();
  await openTerminal();
}

onMounted(async () => {
  if (!termHost.value) return;
  term = new Terminal({
    cursorBlink: true,
    fontSize: 13,
    fontFamily: "var(--font-mono)",
    theme: {
      background: "#0d1117",
      foreground: "#e6edf3",
      cursor: "#f0562c",
    },
    scrollback: 2000,
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(termHost.value);
  fitAddon.fit();

  // 用户输入 → 后端
  term.onData((data) => {
    if (terminalId) {
      ipc.sshTerminalWrite(terminalId, data).catch(() => undefined);
    }
  });

  // 后端输出 → 终端（事件推送）
  unlistenData = await onTerminalData((d) => {
    if (terminalId && d.terminalId === terminalId) {
      term?.write(d.data);
    }
  });

  // 窗口尺寸同步（xterm → SSH PTY）
  resizeObserver = new ResizeObserver(() => {
    if (!term || !fitAddon || !terminalId) return;
    fitAddon.fit();
    const { cols, rows } = term;
    if (cols > 0 && rows > 0) {
      ipc.sshTerminalResize(terminalId, cols, rows).catch(() => undefined);
    }
  });
  resizeObserver.observe(termHost.value);

  // 连接会话变化 → 打开/关闭终端
  await openTerminal();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  unlistenData?.();
  closeTerminal();
  term?.dispose();
  term = null;
});

// 连接切换（侧栏点击其他服务器）时重开终端
watch(
  () => props.connection?.sessionId,
  async (newId, oldId) => {
    if (newId && newId !== oldId) {
      await closeTerminal();
      term?.reset();
      await openTerminal();
    }
  },
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 终端
      </span>
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        {{ statusLine }}
      </span>
      <div class="ml-auto flex gap-[6px]">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption" title="清空终端" @click="term?.clear()">
          清空
        </button>
        <button
          class="btn-ghost !px-[8px] !py-[3px] text-caption"
          title="重连"
          @click="reconnect"
        >
          重连
        </button>
      </div>
    </div>

    <!-- xterm 挂载区 -->
    <div ref="termHost" class="min-h-0 flex-1 overflow-hidden bg-[#0d1117] p-[8px]" />
  </div>
</template>
