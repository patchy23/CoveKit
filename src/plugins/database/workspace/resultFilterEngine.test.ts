import { expect, it, vi } from 'vitest'
import { createFilterEngine } from './resultFilterEngine'

it('复用版本、增量行修改和 Unicode 全文筛选保持原算法的顺序', () => {
  const emit = vi.fn()
  const engine = createFilterEngine(emit)
  const rows = [['İ K Σ'], ['i\u0307 k ος'], ['ABC 2026-09'], ['中文']]
  engine.receive({ type: 'begin', version: 1, length: rows.length, reset: true })
  engine.receive({
    type: 'rows',
    version: 1,
    batch: 1,
    changes: rows.map((row, index) => ({ row, index })),
  })
  for (const [id, term] of ['İ', 'K', 'Σ', 'ΟΣ', '2026-09', '中文'].entries()) {
    const normalized = term.toLowerCase()
    engine.receive({ type: 'query', version: 1, id, term: normalized })
    const result = emit.mock.lastCall![0]
    expect(Array.from(result.indices)).toEqual(
      rows.flatMap((row, index) =>
        row.some((cell) => cell.toLowerCase().includes(normalized)) ? [index] : []
      )
    )
  }
  engine.receive({ type: 'begin', version: 2, length: rows.length, reset: false })
  engine.receive({ type: 'rows', version: 2, batch: 1, changes: [{ index: 3, row: ['updated'] }] })
  engine.receive({ type: 'query', version: 1, id: 99, term: '中文' })
  expect(emit.mock.lastCall![0].type).toBe('rows')
  engine.receive({ type: 'query', version: 2, id: 100, term: 'updated' })
  expect(Array.from(emit.mock.lastCall![0].indices)).toEqual([3])
})

it('分片计算只继续最新关键词，取消后不交付旧结果', async () => {
  let clock = 0
  vi.spyOn(performance, 'now').mockImplementation(() => (clock += 10))
  const yields: Array<() => void> = []
  const emit = vi.fn()
  const engine = createFilterEngine(emit, () => new Promise((resolve) => yields.push(resolve)))
  engine.receive({ type: 'begin', version: 1, length: 2, reset: true })
  engine.receive({
    type: 'rows',
    version: 1,
    batch: 1,
    changes: [
      { index: 0, row: ['a'] },
      { index: 1, row: ['b'] },
    ],
  })
  engine.receive({ type: 'query', version: 1, id: 1, term: 'a' })
  engine.receive({ type: 'query', version: 1, id: 2, term: 'b' })
  yields.shift()!()
  await Promise.resolve()
  engine.receive({ type: 'cancel', version: 1 })
  yields.shift()!()
  await Promise.resolve()
  expect(emit.mock.calls.some(([value]) => value.type === 'result')).toBe(false)
  vi.restoreAllMocks()
})
