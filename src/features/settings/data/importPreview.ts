/**
 * 导入预览分组（sync L3 · 纯函数）
 *
 * 为什么单独一层：预览要回答几个不同的问题——「会写入什么」「什么是冲突要我决策」
 * 「什么已识别跳过」「什么要我自己补」「什么没带进来」，混在一张列表里用户就分不清。
 * 这里只按处置结论分组并计数，文案交给界面（纯函数不碰 i18n）。
 */
import type { ImportPlanItem, ImportPlanResult, ItemDecision } from '@/core/ipc/contracts'

/** 结论分组 */
export interface DecisionGroup {
  decision: ItemDecision
  items: ImportPlanItem[]
}

/** 结论展示顺序：冲突（要决策）最前，然后是会写入的、待补录的、已识别的，最后是不动的 */
const DECISION_ORDER: ItemDecision[] = [
  'replace',
  'keepBoth',
  'insert',
  'pendingReference',
  'restorePrompt',
  'identical',
  'skip',
  'excluded',
]

/** 会写入目标空间的处置（计数与确认按钮的依据） */
const WRITING: ItemDecision[] = ['insert', 'replace', 'keepBoth', 'pendingReference']

/** 按处置结论分组（空组不返回，避免界面上出现「待补录 0 条」这种噪音） */
export function groupByDecision(items: ImportPlanItem[]): DecisionGroup[] {
  return DECISION_ORDER.map((decision) => ({
    decision,
    items: items.filter((item) => item.decision === decision),
  })).filter((group) => group.items.length > 0)
}

/** 每条结论的条数（界面上的汇总行） */
export function decisionCounts(items: ImportPlanItem[]): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const item of items) counts[item.decision] = (counts[item.decision] ?? 0) + 1
  return counts
}

/** 冲突条目（合并模式下用户可逐条改处置的项） */
export function conflictItems(items: ImportPlanItem[]): ImportPlanItem[] {
  return items.filter((item) => item.conflict)
}

/** 计划里各数据集将被写入的条数（来自后端计划，不是前端估算） */
export function plannedCounts(plan: ImportPlanResult | null): { dataset: string; count: number }[] {
  if (!plan) return []
  const rows = Object.entries(plan.counts).map(([dataset, count]) => ({ dataset, count }))
  return rows.sort((a, b) => a.dataset.localeCompare(b.dataset))
}

/** 计划是否处于「可以提交」的状态：有东西要写、且用户确认过覆盖/合并语义 */
export function canCommit(plan: ImportPlanResult | null, acknowledged: boolean): boolean {
  if (!plan) return false
  if (!acknowledged) return false
  // 合并/覆盖模式按「有写入决策」判断；隔离导入按后端声明条数判断
  if (plan.mode && plan.mode !== 'newSpace') {
    return plan.preview.some((item) => WRITING.includes(item.decision))
  }
  return Object.values(plan.counts).some((count) => count > 0)
}

/** 待补录条目的提示文案（供界面逐条展示，不做去重合并） */
export function pendingNotes(plan: ImportPlanResult | null): string[] {
  if (!plan) return []
  return plan.preview
    .filter((item) => item.decision === 'pendingReference')
    .map((item) => item.note ?? item.label)
}
