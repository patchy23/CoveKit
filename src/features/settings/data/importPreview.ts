/**
 * 导入预览分组（sync L2 · 纯函数）
 *
 * 为什么单独一层：预览要回答三个不同的问题——「会写入什么」「什么要我自己补」「什么没带进来」，
 * 三者混在一张列表里用户就分不清哪些是成功、哪些是待办。这里只按结论分组并计数，
 * 文案交给界面（纯函数不碰 i18n）。
 */
import type { ImportPlanItem, ImportPlanResult } from '@/core/ipc/contracts'

/** 导入结论（与 Rust 侧 `ImportOutcome` 逐字对应） */
export type ImportOutcome = ImportPlanItem['outcome']

/** 结论分组 */
export interface OutcomeGroup {
  outcome: ImportOutcome
  items: ImportPlanItem[]
}

/** 结论展示顺序：能直接用的排前面，待补全紧跟，被排除的放最后 */
const OUTCOME_ORDER: ImportOutcome[] = ['added', 'pending-reference', 'excluded']

/** 按结论分组（空组不返回，避免界面上出现「待补全 0 条」这种噪音） */
export function groupByOutcome(items: ImportPlanItem[]): OutcomeGroup[] {
  return OUTCOME_ORDER.map((outcome) => ({
    outcome,
    items: items.filter((item) => item.outcome === outcome),
  })).filter((group) => group.items.length > 0)
}

/** 每条结论的条数（界面上的汇总行） */
export function outcomeCounts(items: ImportPlanItem[]): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const item of items) counts[item.outcome] = (counts[item.outcome] ?? 0) + 1
  return counts
}

/** 计划里各数据集将被写入的条数（来自后端计划，不是前端估算） */
export function plannedCounts(plan: ImportPlanResult | null): { dataset: string; count: number }[] {
  if (!plan) return []
  const rows = Object.entries(plan.counts).map(([dataset, count]) => ({ dataset, count }))
  return rows.sort((a, b) => a.dataset.localeCompare(b.dataset))
}

/** 计划是否处于「可以提交」的状态：有东西要写、且用户确认过覆盖/新空间语义 */
export function canCommit(plan: ImportPlanResult | null, acknowledged: boolean): boolean {
  if (!plan) return false
  if (!acknowledged) return false
  return Object.values(plan.counts).some((count) => count > 0)
}

/** 待补全条目的提示文案（供界面逐条展示，不做去重合并） */
export function pendingNotes(plan: ImportPlanResult | null): string[] {
  if (!plan) return []
  return plan.preview
    .filter((item) => item.outcome === 'pending-reference')
    .map((item) => item.note ?? item.label)
}
