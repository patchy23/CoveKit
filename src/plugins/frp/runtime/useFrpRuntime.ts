/**
 * frp 运行状态与日志（事件驱动为主，5s 轮询兜底）
 * 状态机判定在 Rust 侧（runtime.rs），这里只维护展示态与调用命令。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { FrpLogPayload, FrpRuntimeState } from '../contracts'
import { appendLogLine, logLevel, type FrpLogLine } from './frpStatus'

/** 工具 id（工具级设置的分区键） */
const TOOL_ID = 'frp'
/** 轮询兜底间隔：事件丢失时还能纠正状态 */
const POLL_INTERVAL_MS = 5000
/** 日志缓冲默认行数 */
const DEFAULT_MAX_LINES = 2000

/** 运行状态与日志（供工作台与详情面板共用） */
export function useFrpRuntime() {
  const { t } = useI18n()
  const ui = useUiStore()
  const settings = useSettingsStore()

  /** 各档案的运行状态（key = fileName） */
  const states = ref<Record<string, FrpRuntimeState>>({})
  /** 各档案的日志环形缓冲（key = fileName） */
  const logs = ref<Record<string, FrpLogLine[]>>({})
  /** 正在执行启停操作的档案（按钮禁用以防连点） */
  const busy = ref<Record<string, boolean>>({})

  let unlisteners: UnlistenFn[] = []
  let timer: ReturnType<typeof setInterval> | null = null

  /** 日志缓冲上限（读工具设置，非法值回落默认） */
  function maxLines(): number {
    const value = Number(settings.getToolSetting(TOOL_ID, 'maxLogLines', DEFAULT_MAX_LINES))
    return Number.isFinite(value) && value > 0 ? value : DEFAULT_MAX_LINES
  }

  /** 单个档案的运行状态（未运行过则 undefined，调用方按 stopped 展示） */
  function stateOf(fileName: string): FrpRuntimeState | undefined {
    return states.value[fileName]
  }

  /** 单个档案的日志行 */
  function logsOf(fileName: string): FrpLogLine[] {
    return logs.value[fileName] ?? []
  }

  /** 该档案是否正在启停中 */
  function isBusy(fileName: string): boolean {
    return busy.value[fileName] === true
  }

  /** 合并单个档案状态（整体替换对象触发响应式） */
  function applyState(state: FrpRuntimeState): void {
    states.value = { ...states.value, [state.fileName]: state }
  }

  /** 追加一行日志（环形缓冲按设置上限裁剪） */
  function applyLog(payload: FrpLogPayload): void {
    const current = logs.value[payload.fileName] ?? []
    const line: FrpLogLine = {
      ts: payload.ts,
      line: payload.line,
      stream: payload.stream,
      level: logLevel(payload.line),
    }
    logs.value = { ...logs.value, [payload.fileName]: appendLogLine(current, line, maxLines()) }
  }

  /** 清空某档案的日志缓冲 */
  function clearLogs(fileName: string): void {
    logs.value = { ...logs.value, [fileName]: [] }
  }

  /** 全量拉取状态（进入工具时 + 轮询兜底） */
  async function refresh(): Promise<void> {
    try {
      const list = await ipc.status()
      const next: Record<string, FrpRuntimeState> = {}
      for (const item of list) next[item.fileName] = item
      states.value = next
    } catch {
      // 状态查询失败不打扰用户（轮询会自愈）；操作路径上的失败一定有 toast
    }
  }

  /** 启停命令的统一执行：busy 标记 + toast 反馈 + 状态回写 */
  async function runCommand(
    fileName: string,
    task: () => Promise<FrpRuntimeState>,
    successKey: string
  ): Promise<void> {
    busy.value = { ...busy.value, [fileName]: true }
    try {
      const state = await task()
      applyState(state)
      ui.toast(t(successKey, { name: fileName }))
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message }))
    } finally {
      busy.value = { ...busy.value, [fileName]: false }
    }
  }

  /** 启动档案 */
  function start(fileName: string): Promise<void> {
    return runCommand(fileName, () => ipc.start(fileName), 'frp.startRequested')
  }

  /** 停止档案 */
  function stop(fileName: string): Promise<void> {
    return runCommand(fileName, () => ipc.stop(fileName), 'frp.stopRequested')
  }

  /** 重启档案 */
  function restart(fileName: string): Promise<void> {
    return runCommand(fileName, () => ipc.restart(fileName), 'frp.restartRequested')
  }

  onMounted(async () => {
    unlisteners = await Promise.all([
      listen<FrpRuntimeState>('frp://state', (event) => applyState(event.payload)),
      listen<FrpLogPayload>('frp://log', (event) => applyLog(event.payload)),
    ])
    await refresh()
    timer = setInterval(() => {
      void refresh()
    }, POLL_INTERVAL_MS)
  })

  onBeforeUnmount(() => {
    for (const off of unlisteners) off()
    unlisteners = []
    if (timer !== null) {
      clearInterval(timer)
      timer = null
    }
  })

  return { states, logs, busy, stateOf, logsOf, isBusy, clearLogs, refresh, start, stop, restart }
}
