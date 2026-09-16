/**
 * 导入预览分组用例（sync L2）
 *
 * 关键口径：三类结论必须分开（会写入 / 待补全 / 不带入）；只有「会写入」参与条数统计；
 * 待补全的引用要在确认前就看得见，否则用户导入后才发现凭证没跟过来。
 */
import { describe, expect, it } from 'vitest'
import type { ImportPlanItem, ImportPlanResult } from '@/core/ipc/contracts'
import {
  canCommit,
  groupByOutcome,
  outcomeCounts,
  pendingNotes,
  plannedCounts,
} from './importPreview'

function item(id: string, outcome: ImportPlanItem['outcome'], note?: string): ImportPlanItem {
  return {
    dataset: 'ssh.profiles',
    id,
    label: `服务器 ${id}`,
    outcome,
    note: note ?? null,
  }
}

function plan(preview: ImportPlanItem[]): ImportPlanResult {
  return {
    planId: 'imp-1',
    spaceId: '11111111-1111-4111-8111-111111111111',
    spaceName: '工作机副本',
    preview,
    pending: preview
      .filter((entry) => entry.outcome === 'pending-reference')
      .map((entry) => entry.label),
    excluded: [],
    counts: { 'ssh.profiles': 2, 'vault.credentials': 0 },
  }
}

describe('importPreview', () => {
  it('按结论分组，顺序固定为：会写入 → 待补全 → 不带入', () => {
    const groups = groupByOutcome([
      item('p3', 'excluded'),
      item('p1', 'added'),
      item('p2', 'pending-reference', '凭证 cred-9 未随包带出'),
    ])
    expect(groups.map((group) => group.outcome)).toEqual(['added', 'pending-reference', 'excluded'])
    expect(groups[0].items.map((entry) => entry.id)).toEqual(['p1'])
  })

  it('计数与条数统计只看会写入的类别', () => {
    const preview = [item('p1', 'added'), item('p2', 'added'), item('p3', 'excluded')]
    expect(outcomeCounts(preview)).toEqual({ added: 2, excluded: 1 })
    expect(plannedCounts(plan(preview))).toEqual([
      { dataset: 'ssh.profiles', count: 2 },
      { dataset: 'vault.credentials', count: 0 },
    ])
  })

  it('待补全提示保留可核对的原因原文（界面在确认前展示）', () => {
    const p = plan([item('p2', 'pending-reference', '凭证 cred-9 未随包带出')])
    expect(pendingNotes(p)).toEqual(['凭证 cred-9 未随包带出'])
    expect(pendingNotes(null)).toEqual([])
  })

  it('确认导入需要计划与显式确认同时在（无计划不提交）', () => {
    expect(canCommit(null, true)).toBe(false)
    expect(canCommit(plan([item('p1', 'added')]), false)).toBe(false)
    expect(canCommit(plan([item('p1', 'added')]), true)).toBe(true)
  })
})
