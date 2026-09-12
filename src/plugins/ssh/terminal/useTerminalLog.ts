/**
 * SSH 终端会话日志 composable（状态机 idle → starting → recording → stopping）
 *
 * 目录来源一律是 Rust 侧 ssh_log_dir 解析出的框架存储 logs 分区，前端不拼路径、不本地记忆；
 * 写盘失败由 Rust 推 `ssh://terminal-log-error` 事件，这里负责停状态与可见提示。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'

/** 录制状态 */
export type TerminalLogState = 'idle' | 'starting' | 'recording' | 'stopping'

/** 日志写盘失败事件负载（与 Rust TerminalLogError 同步） */
interface TerminalLogErrorPayload {
  terminalId: string
  message: string
}

/** 单个终端的会话日志录制 */
export function useTerminalLog(terminalId: () => string) {
  const { t } = useI18n()
  const ui = useUiStore()

  /** 当前录制状态 */
  const state = ref<TerminalLogState>('idle')
  /** 当前（或最近一次）日志文件绝对路径 */
  const path = ref('')
  /** 已写入字节数 */
  const bytes = ref(0)

  /** 是否正在录制（按钮文案与红点依据） */
  function isRecording(): boolean {
    return state.value === 'recording'
  }

  /** 开始录制 */
  async function start(): Promise<void> {
    if (state.value === 'starting' || state.value === 'recording') return
    state.value = 'starting'
    try {
      const result = await ipc.terminalLogStart(terminalId(), null)
      path.value = result.path
      bytes.value = result.bytes
      state.value = 'recording'
      ui.toast(t('sshLog.started', { name: fileLabel(result.path) }))
    } catch (reason) {
      state.value = 'idle'
      ui.toast(t('sshLog.startFailed', { message: messageOf(reason) }))
    }
  }

  /** 停止录制 */
  async function stop(): Promise<void> {
    if (state.value !== 'recording') return
    state.value = 'stopping'
    try {
      const result = await ipc.terminalLogStop(terminalId())
      path.value = result.path
      bytes.value = result.bytes
      state.value = 'idle'
      ui.toast(t('sshLog.saved', { path: result.path }))
    } catch (reason) {
      state.value = 'idle'
      ui.toast(t('sshLog.stopFailed', { message: messageOf(reason) }))
    }
  }

  /** 切换录制（按钮点击入口） */
  function toggle(): Promise<void> {
    return isRecording() ? stop() : start()
  }

  /** 错误原因转文本 */
  function messageOf(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason)
  }

  /** 取路径末段作为提示里的文件名 */
  function fileLabel(fullPath: string): string {
    const parts = fullPath.split(/[\\/]/)
    return parts[parts.length - 1] ?? fullPath
  }

  let unlisten: UnlistenFn | null = null

  onMounted(async () => {
    unlisten = await listen<TerminalLogErrorPayload>('ssh://terminal-log-error', (event) => {
      if (event.payload.terminalId !== terminalId()) return
      state.value = 'idle'
      ui.toast(t('sshLog.writeFailed', { message: event.payload.message }))
    })
  })
  onBeforeUnmount(() => unlisten?.())

  return { state, path, bytes, isRecording, start, stop, toggle }
}
