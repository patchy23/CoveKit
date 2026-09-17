/**
 * 导入预览分组用例（sync L3）
 *
 * 关键口径：处置结论分组展示（冲突决策排最前）；只有「会写入」的处置参与提交判定；
 * 待补录的引用要在确认前就看得见，否则用户导入后才发现凭证没跟过来。
 */
import { describe, expect, it } from 'vitest'
import type { ImportPlanItem, ImportPlanResult, ItemDecision } from '@/core/ipc/contracts'
import {
  canCommit,
  conflictItems,
  decisionCounts,
  groupByDecision,
  pendingNotes,
  plannedCounts,
} from './importPreview'

function item(id: string, decision: ItemDecision, note?: string, conflict = false): ImportPlanItem {
  return {
    dataset: 'ssh.profiles',
    id,
    label: `服务器 ${id}`,
    decision,
    conflict,
    note: note ?? null,
  }
}

function plan(
  preview: ImportPlanItem[],
  mode: 'newSpace' | 'merge' = 'newSpace'
): ImportPlanResult {
  return {
    planId: 'imp-1',
    mode,
    spaceId: '11111111-1111-4111-8111-111111111111',
    spaceName: '工作机副本',
    preview,
    pending: preview
      .filter((entry) => entry.decision === 'pendingReference')
      .map((entry) => entry.label),
    excluded: [],
    counts: { 'ssh.profiles': 2, 'vault.credentials': 0 },
  }
}

describe('importPreview', () => {
  it('按处置分组，顺序固定：冲突类决策在前，不动的放最后', () => {
    const groups = groupByDecision([
      item('p4', 'excluded'),
      item('p1', 'insert'),
      item('p2', 'replace', '与本地同 id 记录内容不同', true),
      item('p3', 'identical'),
      item('p5', 'pendingReference', '凭证 cred-9 未随包带出'),
    ])
    expect(groups.map((group) => group.decision)).toEqual([
      'replace',
      'insert',
      'pendingReference',
      'identical',
      'excluded',
    ])
    expect(groups[0].items.map((entry) => entry.id)).toEqual(['p2'])
  })

  it('计数按决策归集；计划条数来自后端声明', () => {
    const preview = [item('p1', 'insert'), item('p2', 'insert'), item('p3', 'excluded')]
    expect(decisionCounts(preview)).toEqual({ insert: 2, excluded: 1 })
    expect(plannedCounts(plan(preview))).toEqual([
      { dataset: 'ssh.profiles', count: 2 },
      { dataset: 'vault.credentials', count: 0 },
    ])
  })

  it('冲突条目单独可列（合并模式的决策列表）', () => {
    const preview = [item('p1', 'skip', '本地已存在同名记录', true), item('p2', 'insert')]
    expect(conflictItems(preview).map((entry) => entry.id)).toEqual(['p1'])
  })

  it('待补录提示保留可核对的原因原文（界面在确认前展示）', () => {
    const p = plan([item('p2', 'pendingReference', '凭证 cred-9 未随包带出')])
    expect(pendingNotes(p)).toEqual(['凭证 cred-9 未随包带出'])
    expect(pendingNotes(null)).toEqual([])
  })

  it('确认导入需要计划与显式确认同时在（无计划不提交）', () => {
    expect(canCommit(null, true)).toBe(false)
    expect(canCommit(plan([item('p1', 'insert')]), false)).toBe(false)
    expect(canCommit(plan([item('p1', 'insert')]), true)).toBe(true)
  })

  it('合并模式按「有写入决策」判定可提交（全跳过则不可提交）', () => {
    expect(canCommit(plan([item('p1', 'skip')], 'merge'), true)).toBe(false)
    expect(canCommit(plan([item('p1', 'insert')], 'merge'), true)).toBe(true)
    expect(canCommit(plan([item('p1', 'replace')], 'merge'), true)).toBe(true)
  })
})
