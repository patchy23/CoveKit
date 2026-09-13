/**
 * 框架 · 工具关闭协商的唯一入口（可靠性 T10-1/T10-2）
 *
 * 为什么要有它：页签关闭原来就是一行 `openTabs.filter`，没有任何地方能「问一句」——
 * 插件里的未保存内容、正在跑的会话与进程都被静默丢弃。这里把关闭改成协商：
 *
 * 1. **prepare**：先问每个 owner「现在能关吗」。任一拒绝就不清理、不关闭，把原因交回界面；
 * 2. **dispose**：只在允许关闭后执行，逐个 owner 清理并收集结果；
 * 3. **总超时**：清理卡住时按失败计并继续（与后端 `lifecycle::DISPOSE_TIMEOUT` 同一口径），
 *    界面不会因为某个 owner 卡死而关不掉。
 *
 * owner 未显式声明时按标记兜底：`dirty` → 「有未保存内容」，`running` → 「有任务正在运行」，
 * 这样「插件忘了写 prepare」也不会退化成静默丢弃。
 */
import type {
  CloseIssue,
  CloseOutcome,
  CloseReason,
  DisposeFn,
  PrepareFn,
  ToolCloseState,
} from './types'

/** 默认清理总超时：与后端关闭清理同一量级（5s），避免前端先于后端放弃 */
export const DEFAULT_DISPOSE_TIMEOUT_MS = 5000

/** 单个 owner 的登记项 */
interface OwnerEntry {
  /** 归属者 id（插件 id 或插件内资源名） */
  owner: string
  /** 关闭前询问（可选） */
  prepare?: PrepareFn
  /** 允许关闭后的清理（可选） */
  dispose?: DisposeFn
  /** 有未保存内容（owner 显式标记；无 prepare 时作为兜底拒绝理由） */
  dirty: boolean
  /** 有进行中的任务（owner 显式标记；无 prepare 时作为兜底拒绝理由） */
  running: boolean
}

/** 登记句柄：交给插件持有，组件卸载时 `unregister` */
export interface ToolOwnerHandle {
  /** 标记/清除「有未保存内容」 */
  setDirty(value: boolean): void
  /** 标记/清除「有进行中的任务」 */
  setRunning(value: boolean): void
  /** 设置关闭前询问（覆盖旧值；返回取消函数） */
  onPrepare(fn: PrepareFn): () => void
  /** 设置关闭清理（覆盖旧值；返回取消函数） */
  onDispose(fn: DisposeFn): () => void
  /** 注销该 owner（组件卸载；不影响其他 owner） */
  unregister(): void
}

/** 工具 id → (owner → 登记项) */
const registry = new Map<string, Map<string, OwnerEntry>>()

/** 关闭状态订阅者（界面展示「未保存/运行中」标记用） */
const stateWatchers = new Map<string, Set<(state: ToolCloseState) => void>>()

/** 状态变化后通知订阅者（标记变化、owner 增删都会触发） */
function notifyState(toolId: string): void {
  const listeners = stateWatchers.get(toolId)
  if (!listeners || listeners.size === 0) return
  const state = toolCloseState(toolId)
  for (const listener of listeners) listener(state)
}

/** 关闭顺序可控：同一工具内按登记顺序询问与清理（结果顺序稳定，便于测试与展示） */
function ownersOf(toolId: string): OwnerEntry[] {
  const table = registry.get(toolId)
  return table ? [...table.values()] : []
}

/** 该工具是否登记过任何 owner（诊断/测试用） */
export function hasOwners(toolId: string): boolean {
  return ownersOf(toolId).length > 0
}

/** 该工具的关闭相关状态（界面展示「未保存/运行中」用） */
export function toolCloseState(toolId: string): ToolCloseState {
  const owners = ownersOf(toolId)
  return {
    dirty: owners.some((o) => o.dirty),
    running: owners.some((o) => o.running),
    owners: owners.length,
  }
}

/**
 * 登记一个 owner。
 *
 * 同 owner 重复登记按覆盖处理（组件重建时沿用同一 owner 名即可），不抛错。
 */
export function registerToolOwner(toolId: string, owner: string): ToolOwnerHandle {
  let table = registry.get(toolId)
  if (!table) {
    table = new Map()
    registry.set(toolId, table)
  }
  const entry: OwnerEntry = { owner, dirty: false, running: false }
  table.set(owner, entry)
  notifyState(toolId)

  const mutate = (patch: Partial<OwnerEntry>) => {
    const current = registry.get(toolId)?.get(owner)
    if (current) {
      Object.assign(current, patch)
      notifyState(toolId)
    }
  }

  return {
    setDirty: (value: boolean) => mutate({ dirty: value }),
    setRunning: (value: boolean) => mutate({ running: value }),
    onPrepare: (fn: PrepareFn) => {
      mutate({ prepare: fn })
      return () => mutate({ prepare: undefined })
    },
    onDispose: (fn: DisposeFn) => {
      mutate({ dispose: fn })
      return () => mutate({ dispose: undefined })
    },
    unregister: () => {
      registry.get(toolId)?.delete(owner)
      notifyState(toolId)
    },
  }
}

/** 清空全部登记（测试用；生产不调用） */
export function resetToolOwnersForTest(): void {
  registry.clear()
  stateWatchers.clear()
}

/**
 * 订阅某工具的关闭状态变化。
 *
 * 注册时立即回调一次：页签不会因为「标记发生在订阅之前」而漏掉未保存提示。
 */
export function watchToolCloseState(
  toolId: string,
  handler: (state: ToolCloseState) => void
): () => void {
  let listeners = stateWatchers.get(toolId)
  if (!listeners) {
    listeners = new Set()
    stateWatchers.set(toolId, listeners)
  }
  listeners.add(handler)
  handler(toolCloseState(toolId))
  return () => {
    listeners?.delete(handler)
  }
}

/** 兜底拒绝理由：owner 没写 prepare 但有标记时，不允许静默丢弃 */
function fallbackBlocker(entry: OwnerEntry): string | null {
  if (entry.running) return '有任务正在运行'
  if (entry.dirty) return '有未保存内容'
  return null
}

/** 超时包装：超时不取消底层动作（无法真正中断插件逻辑），只让关闭流程能继续 */
async function withTimeout<T>(
  task: Promise<T>,
  timeoutMs: number
): Promise<{ value: T; timedOut: false } | { timedOut: true }> {
  let timer: ReturnType<typeof setTimeout> | null = null
  const guard = new Promise<{ timedOut: true }>((resolve) => {
    timer = setTimeout(() => resolve({ timedOut: true }), timeoutMs)
  })
  try {
    const result = await Promise.race([
      task.then((value) => ({ value, timedOut: false as const })),
      guard,
    ])
    return result
  } finally {
    if (timer !== null) clearTimeout(timer)
  }
}

/** 归一化 prepare 结果：非空字符串才算拒绝（空串/undefined 视为允许） */
function normalizeRefusal(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const text = value.trim()
  return text.length > 0 ? text : null
}

/**
 * 询问 + 清理的完整流程（不负责改页面状态，调用方按结果决定是否移除页签）。
 *
 * @param toolId 工具 id
 * @param reason 关闭原因
 * @param options `force` = 用户已确认放弃（跳过 prepare）；`timeoutMs` = 清理总超时
 */
export async function negotiateToolClose(
  toolId: string,
  reason: CloseReason = 'tab',
  options: { force?: boolean; timeoutMs?: number } = {}
): Promise<CloseOutcome> {
  const owners = ownersOf(toolId)
  const blockers: CloseIssue[] = []

  if (!options.force) {
    for (const entry of owners) {
      try {
        const refusal = entry.prepare
          ? normalizeRefusal(await entry.prepare(reason))
          : fallbackBlocker(entry)
        if (refusal) blockers.push({ owner: entry.owner, message: refusal })
      } catch (error) {
        // 询问本身出错不能当成允许关闭：宁可拦下来让用户处理
        blockers.push({
          owner: entry.owner,
          message: `关闭前询问失败：${error instanceof Error ? error.message : String(error)}`,
        })
      }
    }
  } else {
    // 强制关闭：记录「本来不同意」的 owner，但不阻断（诊断与报告需要）
    for (const entry of owners) {
      const refusal = fallbackBlocker(entry)
      if (refusal) blockers.push({ owner: entry.owner, message: refusal })
    }
  }

  if (blockers.length > 0 && !options.force) {
    return { ok: false, blockers, failures: [], timedOut: false }
  }

  const disposal = await disposeToolOwners(toolId, reason, options.timeoutMs)
  return { ok: true, blockers, failures: disposal.failures, timedOut: disposal.timedOut }
}

/** 只执行清理阶段（退出应用时对全部工具调用；不做询问） */
export async function disposeToolOwners(
  toolId: string,
  reason: CloseReason,
  timeoutMs = DEFAULT_DISPOSE_TIMEOUT_MS
): Promise<{ failures: CloseIssue[]; timedOut: boolean }> {
  const owners = ownersOf(toolId).filter((entry) => entry.dispose)
  if (owners.length === 0) return { failures: [], timedOut: false }

  const run = (async () => {
    const failures: CloseIssue[] = []
    for (const entry of owners) {
      try {
        await entry.dispose?.(reason)
      } catch (error) {
        failures.push({
          owner: entry.owner,
          message: error instanceof Error ? error.message : String(error),
        })
      }
    }
    return failures
  })()

  const result = await withTimeout(run, timeoutMs)
  if (result.timedOut) {
    return {
      failures: [
        {
          owner: toolId,
          message: `关闭清理超过 ${timeoutMs} ms 未完成，按清理失败处理`,
        },
      ],
      timedOut: true,
    }
  }
  return { failures: result.value, timedOut: false }
}

/** 已登记 owner 的工具 id 列表（退出清理与诊断用） */
export function registeredToolIds(): string[] {
  return [...registry.keys()].filter((toolId) => ownersOf(toolId).length > 0)
}

/** 退出应用前清理全部已登记工具；逐 owner 失败都收集，不让一个失败阻断其余 */
export async function disposeAllTools(
  reason: CloseReason,
  timeoutMs = DEFAULT_DISPOSE_TIMEOUT_MS
): Promise<{ failures: CloseIssue[]; timedOut: boolean }> {
  const failures: CloseIssue[] = []
  let timedOut = false
  for (const toolId of registeredToolIds()) {
    const outcome = await disposeToolOwners(toolId, reason, timeoutMs)
    failures.push(...outcome.failures)
    timedOut = timedOut || outcome.timedOut
  }
  return { failures, timedOut }
}
