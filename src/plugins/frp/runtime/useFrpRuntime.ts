/**
 * frp 运行状态与日志（事件驱动为主，5s 轮询兜底）
 * 状态机判定在 Rust 侧（runtime.rs），这里只维护展示态与调用命令。
 */
import { onMounted, ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { throttledInterval, useToolLifecycle } from '@/core/lifecycle'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { FrpLogPayload, FrpRuntimeState } from '../contracts'
import { isFrpLive } from '../toolLifecycle'
import { appendLogLine, logLevel, type FrpLogLine } from './frpStatus'

/** 工具 id（工具级设置的分区键） */
const TOOL_ID = 'frp'
/** 轮询兜底间隔：事件丢失时还能纠正状态 */
const POLL_INTERVAL_MS = 5000
/** 工具非激活或被窗口隐藏时的轮询间隔（T10-7：降频但不完全停摆，避免回来时状态过期） */
const HIDDEN_POLL_INTERVAL_MS = 30000
/** 日志缓冲行数上限（2026-09-14 起写死：环形缓冲约几百 KB，没有值得调节的空间） */
const LOG_MAX_LINES = 2000

/** 运行状态与日志（供工作台与详情面板共用） */
export function useFrpRuntime() {
  const { t } = useI18n()
  const ui = useUiStore()

  /** 各档案的运行状态（key = fileName） */
  const states = ref<Record<string, FrpRuntimeState>>({})
  /** 各档案的日志环形缓冲（key = fileName） */
  // 日志行创建后不修改；追加/清空替换快照，不为每行建立深层响应式代理。
  const logs = shallowRef<Record<string, FrpLogLine[]>>({})
  /** 正在执行启停操作的档案（按钮禁用以防连点） */
  const busy = ref<Record<string, boolean>>({})

  // 生命周期（T10-1/T10-5/T10-7）：订阅与定时器都挂在 scope 上，卸载统一释放；
  // visibility 区分「页签切换/设置页覆盖/窗口隐藏」，只有真正不可见时才降频
  const { scope, visibility, running } = useToolLifecycle(TOOL_ID)

  /** 当前是否处于「用户看得见」的状态（激活且未被覆盖、窗口可见） */
  function engaged(): boolean {
    return visibility.value.active && !visibility.value.covered && !visibility.value.hidden
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
    if (scope.disposed) return
    states.value = { ...states.value, [state.fileName]: state }
  }

  /** 追加一行日志（环形缓冲按设置上限裁剪） */
  function applyLog(payload: FrpLogPayload): void {
    if (scope.disposed) return
    if (payload.state) {
      applyState({
        ...payload.state,
        fileName: payload.fileName,
        lastLine: payload.line,
        lastError: payload.lastErrorFromLine ? payload.line : payload.state.lastError,
      })
    }
    const current = logs.value[payload.fileName] ?? []
    const line: FrpLogLine = {
      ts: payload.ts,
      line: payload.line,
      stream: payload.stream,
      level: logLevel(payload.line),
    }
    logs.value = { ...logs.value, [payload.fileName]: appendLogLine(current, line, LOG_MAX_LINES) }
  }

  /** 清空某档案的日志缓冲 */
  function clearLogs(fileName: string): void {
    logs.value = { ...logs.value, [fileName]: [] }
  }

  /** 全量拉取状态（进入工具时 + 轮询兜底） */
  async function refresh(): Promise<void> {
    if (scope.disposed) return
    try {
      const list = await ipc.status()
      if (scope.disposed) return
      const next: Record<string, FrpRuntimeState> = {}
      for (const item of list) next[item.fileName] = item
      states.value = next
      // 运行标记进关闭协商：关页签时用户能看到「还有进程在跑」（T10-4）
      running.value = list.some((item) => isFrpLive(item.state))
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
    if (scope.disposed || isBusy(fileName)) return
    busy.value = { ...busy.value, [fileName]: true }
    try {
      const state = await task()
      if (scope.disposed) return
      applyState(state)
      ui.toast(t(successKey, { name: fileName }))
    } catch (reason) {
      if (scope.disposed) return
      const message = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message }))
    } finally {
      if (!scope.disposed) {
        const next = { ...busy.value }
        delete next[fileName]
        busy.value = next
      }
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

  /** 兜底轮询：间隔随可见性变化，重新可见时立刻刷新一次（不丢状态） */
  function schedulePoll(): void {
    if (scope.disposed) return
    const delay = throttledInterval(POLL_INTERVAL_MS, HIDDEN_POLL_INTERVAL_MS, !engaged())
    scope.timeout(() => {
      void refresh().finally(schedulePoll)
    }, delay)
  }

  onMounted(async () => {
    await scope.listenEvent<FrpRuntimeState>('frp://state', (payload) => applyState(payload))
    if (scope.disposed) return
    await scope.listenEvent<FrpLogPayload>('frp://log', (payload) => applyLog(payload))
    if (scope.disposed) return
    await refresh()
    schedulePoll()
  })

  // 在 setup 同步注册，避免 await 之后新建的 watcher 脱离组件作用域。
  const stopVisibility = watch(visibility, (next, previous) => {
    if (
      next.active &&
      !next.covered &&
      !next.hidden &&
      !(previous.active && !previous.covered && !previous.hidden)
    ) {
      void refresh()
    }
  })
  scope.onResume(() => {
    void refresh()
  })
  scope.addDispose(() => {
    stopVisibility()
    states.value = {}
    logs.value = {}
    busy.value = {}
  })

  return { states, logs, busy, stateOf, logsOf, isBusy, clearLogs, refresh, start, stop, restart }
}
