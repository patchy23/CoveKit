/**
 * 框架 · 关闭协商的后端桥（AR06 §9.2）
 *
 * 唯一裁决在后端：页面内 owner 的询问结果上报过去，与后端模块的 blockers 合成**一次**裁决；
 * 裁决通过后才允许清理，提交阶段由后端清它那一侧（页签级资源，或发起退出）。
 *
 * 这里只做「IPC 扁平字段 ↔ 界面条目」的翻译，不含业务判断：
 * 业务判断在 `toolContext`（页面内 owner）与 `lifecycle::compose_decision`（裁决）。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { CloseDecision } from '@/core/ipc/contracts'
import type { CloseIssue, CloseReason } from './types'

/** 关闭请求载荷（扁平字段，与 Rust `framework::exit::CloseRequest` 一一对应） */
export interface CloseBridgeRequest {
  /** 关闭原因 */
  reason: CloseReason
  /** 页签关闭时的工具 id（`reason='tab'` 必填，后端不猜要关哪个工具） */
  toolId?: string
  /** 页面内 owner 的拒绝原因（后端与模块 blockers 合成唯一裁决） */
  blockers?: string[]
  /** 用户已确认放弃（页签关闭的「放弃并关闭」；退出走 `forceAppExit`） */
  force?: boolean
}

/** 后端拒绝原因（形如 `owner: 原因`）还原成与页面内 owner 同形的条目，界面展示口径一致 */
export function closeIssuesFromBackend(lines: string[]): CloseIssue[] {
  return lines.map((line) => {
    const at = line.indexOf(':')
    if (at <= 0) return { owner: 'framework', message: line.trim() }
    return { owner: line.slice(0, at).trim(), message: line.slice(at + 1).trim() }
  })
}

/** 页面内 owner 的拒绝原因 → 后端契约里的扁平文本（后端只做合成与展示，不再拆解） */
export function toBlockerLines(issues: CloseIssue[]): string[] {
  return issues.map((issue) => `${issue.owner}: ${issue.message}`)
}

/**
 * 合并两侧拒绝原因。
 *
 * 后端裁决会把上报过去的页面内原因原样并入 `blockers`，直接拼接会出现重复条目，
 * 这里按「owner + 文案」去重，界面只看到每条原因一次。
 */
export function mergeBlockers(frontend: CloseIssue[], backendLines: string[]): CloseIssue[] {
  const seen = new Set<string>()
  return [...frontend, ...closeIssuesFromBackend(backendLines)].filter((issue) => {
    const key = `${issue.owner}\u0000${issue.message}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

/** 请后端裁决（prepare 段：只裁决，不清理、不退出） */
export function requestBackendClose(
  reason: CloseReason,
  toolId: string | null,
  blockers: CloseIssue[],
  force: boolean
): Promise<CloseDecision> {
  const payload: CloseBridgeRequest = { reason, blockers: toBlockerLines(blockers) }
  if (toolId) payload.toolId = toolId
  if (force) payload.force = true
  return invokeCommand<CloseBridgeRequest, CloseDecision>('app_request_close', payload)
}

/** 提交关闭（dispose 段：页签关闭清后端工具级资源；退出则由后端发起退出） */
export function commitBackendClose(
  reason: CloseReason,
  toolId: string | null,
  force: boolean
): Promise<CloseDecision> {
  const payload: CloseBridgeRequest = { reason }
  if (toolId) payload.toolId = toolId
  if (force) payload.force = true
  return invokeCommand<CloseBridgeRequest, CloseDecision>('app_commit_close', payload)
}
