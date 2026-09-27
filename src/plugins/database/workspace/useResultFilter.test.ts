import { markRaw, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useResultFilter } from './useResultFilter'

afterEach(() => vi.restoreAllMocks())

it('收窄筛选只复查命中行，扩展范围、清空和结果替换仍搜索完整结果', () => {
  const original = markRaw([['Alpha', '中文'], ['Alpine'], ['Beta']])
  const visits = original.map((row) => vi.spyOn(row, 'some'))
  const rows = ref(original)
  const query = ref('al')
  const filtered = useResultFilter(
    () => rows.value,
    () => query.value
  )
  expect(filtered.value).toEqual([original[0], original[1]])
  visits.forEach((visit) => visit.mockClear())
  query.value = 'ALPha'
  expect(filtered.value).toEqual([original[0]])
  expect(visits.map((visit) => visit.mock.calls.length)).toEqual([1, 1, 0])
  query.value = 'a'
  expect(filtered.value).toEqual(original)
  query.value = ' 中文 '
  expect(filtered.value).toEqual([original[0]])
  query.value = ''
  expect(filtered.value).toBe(original)
  query.value = 'beta'
  expect(filtered.value).toEqual([original[2]])
  rows.value = markRaw([['new BETA'], ['Beta two']])
  expect(filtered.value).toEqual(rows.value)
})

it('大小写和边缘空白变化不重复扫描，收窄无匹配结果后退格能恢复', () => {
  const rows = markRaw([['alphabet'], ['other']])
  const visit = vi.spyOn(rows[0], 'some')
  const query = ref('alpha')
  const filtered = useResultFilter(
    () => rows,
    () => query.value
  )
  const first = filtered.value
  query.value = ' ALPHA '
  expect(filtered.value).toBe(first)
  expect(visit).toHaveBeenCalledTimes(1)
  query.value = 'alphaz'
  expect(filtered.value).toEqual([])
  visit.mockClear()
  query.value = 'alphazz'
  expect(filtered.value).toEqual([])
  expect(visit).not.toHaveBeenCalled()
  query.value = 'alpha'
  expect(filtered.value).toEqual([rows[0]])
})

it('可原地编辑的响应式结果不复用旧候选，原先不匹配的单元格修改后进入结果', () => {
  const rows = ref([['alpha'], ['beta']])
  const query = ref('alpha')
  const filtered = useResultFilter(
    () => rows.value,
    () => query.value
  )
  expect(filtered.value).toEqual([rows.value[0]])
  rows.value[1][0] = 'alphabet'
  query.value = 'alphab'
  expect(filtered.value).toEqual([rows.value[1]])
  rows.value[0][0] = 'alphabet too'
  expect(filtered.value).toEqual(rows.value)
})

it('连续收窄、替换、退格的结果与原全量算法一致且保留原始行身份', () => {
  const rows = markRaw(
    Array.from({ length: 1000 }, (_, i) => [`行 ${i}`, i % 3 ? 'BETA' : 'alpha'])
  )
  const query = ref('')
  const filtered = useResultFilter(
    () => rows,
    () => query.value
  )
  for (const term of ['行', '行 1', '行 12', '行 123', '行 1', 'beta', '中文', '', 'ALPHA']) {
    query.value = term
    const normalized = term.trim().toLowerCase()
    const expected = rows.filter((row) =>
      row.some((cell) => cell.toLowerCase().includes(normalized))
    )
    expect(filtered.value).toEqual(expected)
    filtered.value.forEach((row, i) => expect(row).toBe(expected[i]))
  }
})

it('数字与标点查询直接匹配正文，Unicode 大小写及混合关键词保持原语义', () => {
  const rows = markRaw([
    ['ABC 2026-09-27T12:34:56', '中文'],
    ['İ K Σ', '123\t456'],
    ['ΟΣ', '[]{}:/_@`~!'],
    ['i\u0307 k ος', '123\u0000456'],
  ])
  const query = ref('')
  const filtered = useResultFilter(
    () => rows,
    () => query.value
  )
  for (const term of [
    '2026-09',
    '12:34',
    '123',
    '123\t456',
    '123\u0000456',
    '[]{}:/_@`~!',
    'ABC 2026',
    'İ',
    '\u0307',
    'K',
    'Σ',
    'ΟΣ',
    '中文',
  ]) {
    query.value = term
    const normalized = term.trim().toLowerCase()
    expect(filtered.value).toEqual(
      rows.filter((row) => row.some((cell) => cell.toLowerCase().includes(normalized)))
    )
  }
  const lower = vi.spyOn(String.prototype, 'toLowerCase')
  query.value = '2026-09'
  expect(filtered.value).toEqual([rows[0]])
  // 只归一化关键词，不为每个单元格建立小写副本。
  expect(lower).toHaveBeenCalledTimes(1)
})
