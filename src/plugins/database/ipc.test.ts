import { afterEach, expect, it, vi } from 'vitest'
import { invokeCommand } from '@/core/ipc/ipc'
import { draftIpc, queryIpc } from './ipc'
import type { QueryResult } from './contracts'

vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: vi.fn() }))
afterEach(() => vi.clearAllMocks())

it('草稿增量通过原命令携带完整顺序，空顺序用于原子清空', async () => {
  await draftIpc.save([], ['a', 'b'])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', {
    drafts: [],
    order: ['a', 'b'],
  })
  await draftIpc.save([], [])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', { drafts: [], order: [] })
  await draftIpc.save([])
  expect(invokeCommand).toHaveBeenLastCalledWith('dbc_drafts_save', { drafts: [] })
})

function result(overrides: Partial<QueryResult> = {}): QueryResult {
  return {
    ok: true,
    columns: [],
    rows: [],
    rowsAffected: 0,
    isQuery: true,
    durationMs: 1,
    truncated: false,
    ...overrides,
  }
}

it('多语句根展示引用完整末语句数据，保留脚本汇总状态', async () => {
  const last = result({
    columns: ['value'],
    rows: [['large text']],
    values: [[{ kind: 'text', value: 'large text' }]],
    columnTypes: ['TEXT'],
  })
  const payload = result({
    statements: [result({ ok: false }), last],
    displayStatement: 1,
    ok: false,
    truncated: true,
    durationMs: 123,
    transactionActive: true,
  })
  vi.mocked(invokeCommand).mockResolvedValueOnce(payload)
  const received = await queryIpc.execute('c', 'select 1; select 2', 100, 'r')
  expect(received.rows).toBe(last.rows)
  expect(received.values).toBe(last.values)
  expect(received.columns).toBe(last.columns)
  expect(received.columnTypes).toBe(last.columnTypes)
  expect(received).toMatchObject({
    ok: false,
    truncated: true,
    durationMs: 123,
    transactionActive: true,
  })
  expect(received.statements?.[0]?.ok).toBe(false)
})

it('单语句及原有完整响应直接返回，不重新复制数组', async () => {
  const payload = result({ rows: [['old-compatible']] })
  vi.mocked(invokeCommand).mockResolvedValueOnce(payload)
  expect(await queryIpc.execute('c', 'select 1', 100, 'r')).toBe(payload)
})

it('缺失的数据引用显式失败，不能伪装成空查询成功', async () => {
  vi.mocked(invokeCommand).mockResolvedValueOnce(result({ displayStatement: 2, statements: [] }))
  await expect(queryIpc.execute('c', 'select 1', 100, 'r')).rejects.toThrow('缺少对应语句')
})
