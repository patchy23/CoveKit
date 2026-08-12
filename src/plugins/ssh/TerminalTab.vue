<script setup lang="ts">
/**
 * TerminalTab · SSH 交互式终端（xterm.js）
 * 连接会话 → 打开 PTY 通道；输出走 ssh://terminal-data 事件推送，
 * 输入走 ssh_terminal_write；ResizeObserver 同步窗口大小。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'
import 'xterm/css/xterm.css'
import type { ServerConnection, ServerProfile } from './contracts'
import { useUiStore } from '@/stores/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import { ipc, onTerminalData } from './ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  dockerContainerId?: string
  connectRequest?: number
}>()

const ui = useUiStore()

const termHost = ref<HTMLDivElement | null>(null)
const statusLine = ref('终端未连接')
const terminalActive = ref(false)
const menu = ref<{ x: number; y: number; hasSelection: boolean } | null>(null)

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let terminalId: string | null = null
let openingConnectionId: string | null = null
let openingGeneration = 0
let unlistenData: (() => void) | null = null
let resizeObserver: ResizeObserver | null = null
let terminalGeneration = 0
let handledConnectRequest = 0
let disposed = false

/** fit 终端并兜底：fit 的行高度量与渲染行高（line-height: normal）存在亚像素偏差，
 *  行数偏大时最后一行会被容器裁掉半截（最大化时最明显），溢出则减一行。 */
function fitTerminal() {
  if (!term || !fitAddon) return
  fitAddon.fit()
  const hostEl = termHost.value
  const screenEl = term.element?.querySelector('.xterm-screen')
  if (hostEl && screenEl) {
    const cs = getComputedStyle(hostEl)
    const availH =
      hostEl.clientHeight - parseInt(cs.paddingTop || '0') - parseInt(cs.paddingBottom || '0')
    if (screenEl.scrollHeight > availH + 1) {
      term.resize(term.cols, Math.max(1, term.rows - 1))
    }
  }
}

/** 打开终端通道（连接建立/重连时调用） */
async function openTerminal() {
  const connectionId = props.connection?.sessionId
  if (!connectionId || !term) return
  if (terminalId || openingConnectionId === connectionId) return
  const generation = ++terminalGeneration
  openingConnectionId = connectionId
  openingGeneration = generation
  try {
    const cols = term.cols
    const rows = term.rows
    const t = props.dockerContainerId
      ? await ipc.sshDockerExec({
          connectionId,
          containerId: props.dockerContainerId,
          shell: '/bin/sh',
          cols,
          rows,
        })
      : await ipc.sshTerminalOpen({ connectionId, cols, rows })
    if (
      disposed ||
      generation !== terminalGeneration ||
      props.connection?.sessionId !== connectionId
    ) {
      await ipc.sshTerminalClose(t.id).catch(() => undefined)
      return
    }
    terminalId = t.id
    terminalActive.value = true
    statusLine.value = `已连接 ${props.connection.host ?? ''} · ${t.cols}×${t.rows}`
    ui.toast('终端已打开')
  } catch (e) {
    if (generation === terminalGeneration) {
      statusLine.value = `终端打开失败：${e}`
      ui.toast(`终端打开失败：${e}`)
    }
  } finally {
    if (openingGeneration === generation) openingConnectionId = null
  }
}

/** 关闭终端通道（断开/组件卸载时调用） */
async function closeTerminal() {
  terminalGeneration += 1
  openingConnectionId = null
  openingGeneration = terminalGeneration
  const id = terminalId
  terminalId = null
  terminalActive.value = false
  if (id) {
    try {
      await ipc.sshTerminalClose(id)
    } catch {
      /* 通道可能已被服务端关闭 */
    }
  }
}

async function reconnect() {
  await closeTerminal()
  term?.reset()
  await openTerminal()
}

/** 在终端区域打开项目统一右键菜单。 */
function openContextMenu(event: MouseEvent) {
  event.preventDefault()
  const width = 150
  const height = 112
  menu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8)),
    hasSelection: Boolean(term?.hasSelection()),
  }
}

function selectAll() {
  term?.selectAll()
}

async function copySelection() {
  const selection = term?.getSelection() ?? ''
  if (!selection) return
  try {
    await writeText(selection)
  } catch (error) {
    ui.toast(`复制失败：${error}`)
  }
}

async function pasteClipboard() {
  try {
    const text = await readText()
    if (text) term?.paste(text)
  } catch (error) {
    ui.toast(`粘贴失败：${error}`)
  }
}

const menuItems = computed<ContextMenuItem[]>(() => [
  { label: '全选', onClick: selectAll },
  {
    label: '复制',
    disabled: !menu.value?.hasSelection,
    onClick: () => void copySelection(),
  },
  { label: '粘贴', onClick: () => void pasteClipboard() },
])

onMounted(async () => {
  if (!termHost.value) return
  term = new Terminal({
    cursorBlink: true,
    fontSize: 13,
    fontFamily: 'var(--font-mono)',
    theme: {
      background: '#0d1117',
      foreground: '#e6edf3',
      cursor: '#f0562c',
    },
    scrollback: 2000,
  })
  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.open(termHost.value)
  fitTerminal()

  // 用户输入 → 后端
  term.onData((data) => {
    if (terminalId) {
      ipc.sshTerminalWrite(terminalId, data).catch((e) => {
        statusLine.value = `终端写入失败：${e}`
      })
    }
  })

  // 后端输出 → 终端（事件推送）
  try {
    const stop = await onTerminalData((d) => {
      if (terminalId && d.terminalId === terminalId) {
        term?.write(d.data)
      }
    })
    if (disposed) stop()
    else unlistenData = stop
  } catch {
    /* 浏览器预览没有 Tauri 事件系统。 */
  }

  // 窗口尺寸同步（xterm → SSH PTY）
  resizeObserver = new ResizeObserver(() => {
    if (!term || !fitAddon || !terminalId) return
    fitTerminal()
    const { cols, rows } = term
    if (cols > 0 && rows > 0) {
      ipc.sshTerminalResize(terminalId, cols, rows).catch((e) => {
        statusLine.value = `终端尺寸同步失败：${e}`
      })
    }
  })
  resizeObserver.observe(termHost.value)

  // 主终端仅响应左侧服务器的显式连接请求；容器终端由“终端”按钮显式打开。
  const request = props.connectRequest ?? 0
  if (request > handledConnectRequest && props.connection?.sessionId) {
    handledConnectRequest = request
    await openTerminal()
  }
})

onBeforeUnmount(() => {
  disposed = true
  resizeObserver?.disconnect()
  unlistenData?.()
  closeTerminal()
  term?.dispose()
  term = null
})

// 连接切换时清理旧终端；仅有显式连接请求时打开新终端。
watch(
  [() => props.connection?.sessionId, () => props.connectRequest ?? 0],
  async ([newId, request], [oldId]) => {
    if (newId && newId !== oldId) {
      await closeTerminal()
      term?.reset()
      statusLine.value = '终端未连接'
    } else if (!newId && oldId) {
      await closeTerminal()
      term?.reset()
      statusLine.value = '终端未连接'
    }
    if (newId && request > handledConnectRequest) {
      handledConnectRequest = request
      await openTerminal()
    }
  }
)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ dockerContainerId ? '容器终端' : '终端' }}
      </span>
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        {{ statusLine }}
      </span>
      <div class="ml-auto flex gap-[6px]">
        <button
          class="btn-ghost !px-[8px] !py-[3px] text-caption"
          title="清空终端"
          @click="term?.clear()"
        >
          清空
        </button>
        <button
          class="btn-ghost !px-[8px] !py-[3px] text-caption"
          :title="terminalActive ? '重连终端' : '连接终端'"
          @click="terminalActive ? reconnect() : openTerminal()"
        >
          {{ terminalActive ? '重连' : '连接' }}
        </button>
      </div>
    </div>

    <!-- xterm 挂载区 -->
    <div
      ref="termHost"
      class="min-h-0 flex-1 overflow-hidden bg-[#0d1117] p-[8px]"
      @contextmenu="openContextMenu"
    />
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
  </div>
</template>
