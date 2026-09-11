<script setup lang="ts">
/**
 * TerminalTab · xterm 终端（PTY 通道 + 事件推送）
 * - 主终端由 connectRequest 显式驱动开启；容器终端由 Docker 页「终端」按钮打开。
 * - 意外断线：后端 terminal-closed 事件（非本地关闭）→ 上报 linkDead，由工作区决定自动重连。
 * - 自动/手动重连成功（reconnectTick 递增）：保留 xterm 缓冲，仅换 PTY 通道并插入重连分隔线。
 * - 断开提示沿用 OpenSSH 客户端措辞（英文、无装饰）；断开态按 Enter 重新连接（缓冲保留）。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'
import 'xterm/css/xterm.css'
import type { ServerConnection, ServerProfile } from './contracts'
import { UiButton } from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import { useTerminalContextMenu } from './useTerminalContextMenu'
import { ipc, onTerminalClosed, onTerminalData } from './ipc'
import { createTerminalResizeController } from './useTerminalResize'
import { bannerTime, disconnectBanner, reconnectSeparator } from './useTerminalBanner'
import { useTerminalLog } from './useTerminalLog'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  dockerContainerId?: string
  connectRequest?: number
  /** 重连成功计数：递增 = 在保留缓冲的前提下换新 PTY 通道并插入分隔线 */
  reconnectTick?: number
  /** 连接进度文案（connecting/reconnecting 期间非空，覆盖状态栏） */
  stageText?: string
  active?: boolean
}>()

const emit = defineEmits<{
  (e: 'reconnect'): void
  /** 终端通道异常关闭（连接可能已死），由工作区触发自动重连 */
  (e: 'linkDead'): void
}>()

const ui = useUiStore()
const { t } = useI18n()

/** 会话日志录制（日志目录由 Rust 侧框架存储 logs 分区决定，前端不拼路径） */
const terminalLog = useTerminalLog(() => terminalId ?? '')

const termHost = ref<HTMLDivElement | null>(null)
const statusLine = ref('终端未连接')
const terminalActive = ref(false)
/** 状态栏展示：连接进度优先（连接中/重连中），否则显示终端状态 */
const displayLine = computed(() => props.stageText || statusLine.value)

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let terminalId: string | null = null
let openingConnectionId: string | null = null
let openingGeneration = 0
let unlistenData: (() => void) | null = null
let unlistenClosed: (() => void) | null = null
let resizeObserver: ResizeObserver | null = null
let terminalGeneration = 0
let handledConnectRequest = 0
let handledReconnectTick = 0
let disposed = false

const terminalResize = createTerminalResizeController({
  getTerminal: () => term,
  getFitAddon: () => fitAddon,
  getHost: () => termHost.value,
  getTerminalId: () => terminalId,
  resize: ipc.sshTerminalResize,
  onError: (id, error) => {
    if (terminalId === id) statusLine.value = `终端尺寸同步失败：${error}`
  },
})

/** 最近一次已知对端地址：断开事件会把 host 清空，终端提示仍需写出连的是哪台机器 */
let knownHost = ''
watch(
  () => props.connection?.host,
  (host) => {
    if (host) knownHost = host
  }
)

/** 重连分隔线（灰色，与旧输出在视觉上区分） */
function writeReconnectSeparator() {
  term?.write(reconnectSeparator(knownHost, bannerTime()))
}

/** 断开提示（OpenSSH 客户端同款措辞，英文无装饰；缓冲保留） */
function writeDisconnectBanner() {
  term?.write(disconnectBanner(knownHost))
}

/** 打开终端通道（连接建立/重连时调用）；preserve=true 时保留缓冲（重连场景） */
async function openTerminal(preserve = false) {
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
    if (preserve) writeReconnectSeparator()
    // 连接可能在隐藏页签或窗口初始布局尚未稳定时建立；此处必须再按当前可见尺寸同步一次。
    terminalResize.scheduleFitAndSync()
    if (!preserve) ui.toast('终端已打开')
  } catch (e) {
    if (generation === terminalGeneration) {
      statusLine.value = `终端打开失败：${e}`
      ui.toast(`终端打开失败：${e}`)
    }
  } finally {
    if (openingGeneration === generation) openingConnectionId = null
  }
}

/** 关闭终端通道（断开/组件卸载时调用）；先清 terminalId，terminal-closed 事件即不会误报断线 */
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
  // 已连接：只重建 PTY 通道（保留缓冲），不惊动工作区——其重连守卫对 connected 直接返回
  if (props.connection?.status === 'connected') {
    await closeTerminal()
    await openTerminal(true)
    return
  }
  await closeTerminal()
  term?.reset()
  emit('reconnect')
}

// 右键菜单逻辑在 useTerminalContextMenu（焦点归还规则见该文件头注释）
const { menu, menuItems, openContextMenu } = useTerminalContextMenu(() => term)

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
  terminalResize.scheduleFitAndSync()

  // 用户输入 → 后端；无通道时按 Enter = 重新连接（断开提示引导，缓冲保留）
  term.onData((data) => {
    if (terminalId) {
      ipc.sshTerminalWrite(terminalId, data).catch((e) => {
        statusLine.value = `终端写入失败：${e}`
      })
    } else if (!props.dockerContainerId && (data === '\r' || data === '\n')) {
      emit('reconnect')
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

  // PTY 通道异常关闭（服务端/网络原因；本地主动关闭前已清 terminalId，不会走到这里）。
  // 仅主终端上报 linkDead：容器终端 exit 退出属正常流程，不应触发整条连接自动重连。
  try {
    const stop = await onTerminalClosed((d) => {
      if (terminalId && d.terminalId === terminalId) {
        terminalId = null
        terminalActive.value = false
        statusLine.value = '连接已断开'
        if (!props.dockerContainerId) emit('linkDead')
      }
    })
    if (disposed) stop()
    else unlistenClosed = stop
  } catch {
    /* 浏览器预览没有 Tauri 事件系统。 */
  }

  // 窗口尺寸同步（xterm → SSH PTY）
  resizeObserver = new ResizeObserver(() => terminalResize.scheduleFitAndSync())
  resizeObserver.observe(termHost.value)
  window.addEventListener('resize', terminalResize.scheduleFitAndSync)
  terminalResize.scheduleFitAndSync()

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
  window.removeEventListener('resize', terminalResize.scheduleFitAndSync)
  terminalResize.cancelScheduledFit()
  unlistenData?.()
  unlistenClosed?.()
  closeTerminal()
  term?.dispose()
  term = null
})

// 连接切换时清理旧终端；仅有显式连接请求时打开新终端。
watch(
  [() => props.connection?.sessionId, () => props.connectRequest ?? 0],
  async ([newId, request], [oldId]) => {
    // 重连换会话（reconnectTick 有未处理递增）不清缓冲：reconnectTick watcher 负责
    // openTerminal(true) 换通道并打分隔线；只有真正的「换连接」才 reset
    const reconnectSwap = (props.reconnectTick ?? 0) > handledReconnectTick
    if (newId && newId !== oldId && !reconnectSwap) {
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

// 连接断开（connected → 断开态）：往终端缓冲写横幅提示，引导按 Enter 重连。
// 容器终端不提示（exit 退出属正常流程）；连接从未成功过的页签不经历此路径。
watch(
  () => props.connection?.status,
  (status, prev) => {
    if (props.dockerContainerId) return
    if (prev === 'connected' && (status === 'disconnected' || status === 'error')) {
      writeDisconnectBanner()
    }
  }
)

// 重连成功：保留缓冲换新通道（不 reset），分隔线标记断点
watch(
  () => props.reconnectTick ?? 0,
  async (tick) => {
    if (tick > handledReconnectTick) {
      handledReconnectTick = tick
      await openTerminal(true)
    }
  }
)

watch(
  () => props.active,
  (active) => {
    if (active) terminalResize.scheduleFitAndSync()
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
        {{ displayLine }}
      </span>
      <div class="ml-auto flex gap-[6px]">
        <UiButton
          variant="ghost"
          size="xs"
          class="!h-auto !px-[8px] !py-[3px] text-caption"
          :class="terminalLog.isRecording() ? 'text-danger-strong dark:text-danger-dark' : ''"
          :title="
            terminalLog.isRecording() ? t('sshLog.buttonStopTitle') : t('sshLog.buttonStartTitle')
          "
          :disabled="!terminalActive"
          @click="terminalLog.toggle()"
        >
          {{ terminalLog.isRecording() ? t('sshLog.buttonStop') : t('sshLog.buttonStart') }}
        </UiButton>
        <UiButton
          variant="ghost"
          size="xs"
          class="!h-auto !px-[8px] !py-[3px] text-caption"
          title="清空终端"
          @click="term?.clear()"
        >
          清空
        </UiButton>
        <UiButton
          variant="ghost"
          size="xs"
          class="!h-auto !px-[8px] !py-[3px] text-caption"
          :title="terminalActive ? '重连终端' : '连接终端'"
          @click="terminalActive ? reconnect() : emit('reconnect')"
        >
          {{ terminalActive ? '重连' : '连接' }}
        </UiButton>
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
