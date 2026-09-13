/**
 * 框架 · 工具关闭协商的公共类型
 *
 * 关闭原因与 Rust 侧 `CloseReason` 的稳定字符串一一对应（见 framework/lifecycle.rs），
 * 两端新增变体必须同时改这里，不允许只改一端。
 */

/** 关闭原因（与后端 `CloseReason::code()` 完全一致的稳定字符串） */
export type CloseReason = 'tab' | 'exit' | 'restart' | 'update' | 'space-switch'

/** 某一个 owner（插件内的一处资源）的拒绝或失败描述 */
export interface CloseIssue {
  /** 归属者：插件 id 或插件内的具体资源名（失败定位用） */
  owner: string
  /** 面向用户的文案（可直接展示） */
  message: string
}

/** 一次关闭协商的结果 */
export interface CloseOutcome {
  /** 是否允许关闭（`false` = 有 owner 拒绝，未执行任何清理） */
  ok: boolean
  /** 拒绝关闭的原因（prepare 阶段） */
  blockers: CloseIssue[]
  /** 清理失败（dispose 阶段；不阻断关闭，但必须让用户看到） */
  failures: CloseIssue[]
  /** 清理是否超时（超时按失败计入，界面已关闭） */
  timedOut: boolean
}

/** 关闭前询问：返回非空字符串表示拒绝关闭（文案直接展示给用户） */
export type PrepareFn = (
  reason: CloseReason
) => string | null | undefined | Promise<string | null | undefined>

/** 允许关闭后的清理：抛错会被计为该 owner 的清理失败，不会中断其他 owner */
export type DisposeFn = (reason: CloseReason) => void | Promise<void>

/** 工具上下文对外暴露的关闭相关状态（供界面展示「有未保存内容/正在运行」） */
export interface ToolCloseState {
  /** 存在未保存内容（owner 显式标记） */
  dirty: boolean
  /** 存在进行中的任务（owner 显式标记） */
  running: boolean
  /** 已登记的 owner 数量 */
  owners: number
}
