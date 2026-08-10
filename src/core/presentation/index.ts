/**
 * Presentation 双载体（架构 §4.2，2026-08-02 用户决策更新）
 * workspace=多页签工作区（工具以子页面打开，第一批起全部工具使用）；
 * modal=轻量弹窗（备用载体，未来按需重建 ToolModal）。
 * 工具不感知载体：框架按 manifest.presentation 路由挂载。
 */
import type { Presentation } from '@/core/registry/types'

export type { Presentation }

/** 载体路由：按工具声明返回挂载方式 */
export function resolvePresentation(p: Presentation): 'workspace' | 'modal' {
  return p
}

/** 打开工具的载荷 */
export interface OpenToolPayload {
  id: string
}
