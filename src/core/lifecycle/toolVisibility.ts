/**
 * 框架 · 工具可见性状态（可靠性 T10-1）
 *
 * 「工具切换」「设置页覆盖」「窗口隐藏」是三种不同状态，插件需要分别知道：
 * 失焦不是断开、隐藏不该停心跳、覆盖只是别占 CPU——把它们压成一个 boolean 就没法表达。
 *
 * 本模块只做状态分发：谁关心谁订阅，框架不替插件决定该做什么。
 */
import type { Ref } from 'vue'

/** 工具的可见性状态（三种来源分别可见，不合并） */
export interface ToolVisibility {
  /** 是否为当前激活页签 */
  active: boolean
  /** 是否被设置页等整页界面覆盖（仍在页签上，但用户看不到） */
  covered: boolean
  /** 主窗口是否隐藏到托盘 / 页面不可见 */
  hidden: boolean
}

/** 默认状态：全部不可见（新登记的工具在收到第一次分发前按不可见处理） */
export const HIDDEN_VISIBILITY: ToolVisibility = { active: false, covered: false, hidden: false }

/** toolId → 当前状态 */
const visibility = new Map<string, ToolVisibility>()
/** toolId → 订阅者 */
const watchers = new Map<string, Set<(state: ToolVisibility) => void>>()

/** 读取工具当前可见性（未登记过则返回默认值，不抛错） */
export function toolVisibility(toolId: string): ToolVisibility {
  return visibility.get(toolId) ?? { ...HIDDEN_VISIBILITY }
}

/**
 * 发布工具可见性变化。
 *
 * 只传变化的字段（patch 合并），调用方不必关心其他来源，避免「谁最后写谁正确」。
 */
export function publishToolVisibility(toolId: string, patch: Partial<ToolVisibility>): void {
  const next = { ...toolVisibility(toolId), ...patch }
  const current = visibility.get(toolId)
  if (
    current &&
    current.active === next.active &&
    current.covered === next.covered &&
    current.hidden === next.hidden
  ) {
    return
  }
  visibility.set(toolId, next)
  const listeners = watchers.get(toolId)
  if (!listeners) return
  for (const listener of listeners) listener(next)
}

/** 主窗口隐藏状态变化时，广播到所有已登记工具（隐藏是全局的，不是某个工具的属性） */
export function publishGlobalHidden(hidden: boolean): void {
  for (const toolId of visibility.keys()) publishToolVisibility(toolId, { hidden })
}

/** 订阅某工具的可见性变化；返回退订函数 */
export function watchToolVisibility(
  toolId: string,
  handler: (state: ToolVisibility) => void
): () => void {
  let listeners = watchers.get(toolId)
  if (!listeners) {
    listeners = new Set()
    watchers.set(toolId, listeners)
  }
  listeners.add(handler)
  return () => {
    listeners?.delete(handler)
  }
}

/** 清空全部可见性状态与订阅（测试用；生产不调用） */
export function resetToolVisibilityForTest(): void {
  visibility.clear()
  watchers.clear()
}

/** 供组合式函数使用的 Vue 引用形态（避免插件各自写 ref + 订阅样板） */
export type ToolVisibilityRef = Ref<ToolVisibility>
