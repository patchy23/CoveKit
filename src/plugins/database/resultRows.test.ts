import { computed, markRaw, reactive, ref } from 'vue'
import { expect, it, vi } from 'vitest'
import {
  indexResultRows,
  resultGridRows,
  resultPageAction,
  selectResultValues,
  sliceResultPage,
} from './resultRows'
import { rowsToTsv } from './resultText'
import type { DbValue } from './contracts'

it('显示页先切原始行并保留筛选行身份，未满页时仍继续读取数据', () => {
  const rows = markRaw(Array.from({ length: 250 }, (_, index) => [`row-${index}`]))
  const filtered = [rows[2], rows[100], rows[201]]
  const visible = sliceResultPage(filtered, 2, 1)
  const values: DbValue[][] = rows.map((row) => [{ kind: 'text', value: row[0] }])
  const index = indexResultRows(rows)
  expect(resultGridRows(['name'], visible, values, index)).toEqual([
    { __row: '100', c0: 'row-100' },
  ])
  expect(sliceResultPage(rows, 2, 100)).toEqual(rows.slice(100, 200))
  expect(resultPageAction(1, 100, 50, true)).toBe('load-next')
  expect(resultPageAction(1, 100, 100, true)).toBe('load-next')
  expect(resultPageAction(1, 100, 200, true)).toBe('advance')
  expect(resultPageAction(2, 100, 150, true)).toBe('load-next')
  expect(resultPageAction(2, 100, 150, false)).toBe('end')
})

it.each([2, 128, 129])('原始快照与响应式可见行混用时，%i 行结果仍定位到原值', (length) => {
  const rows = Array.from({ length }, (_, index) => [`row-${index}`])
  const visible = reactive(rows).slice().reverse()
  const values: DbValue[][] = rows.map((row) => [{ kind: 'text', value: row[0] }])
  const index = indexResultRows(markRaw(rows))
  expect(index(visible[0])).toBe(length - 1)
  expect(resultGridRows(['name'], visible.slice(0, 1), values, index)).toEqual([
    { __row: String(length - 1), c0: `row-${length - 1}` },
  ])
  expect(selectResultValues(visible.slice(0, 1), values, index)[0]).toBe(values[length - 1])
})

it('筛选和重排行沿用原类型，保留 NULL、二进制、空文本及缺少类型时的回退', () => {
  const rows = [[''], ['00ff'], [''], ['fallback']]
  const values: DbValue[][] = [
    [{ kind: 'null', value: null }],
    [{ kind: 'binary', value: '00ff' }],
    [{ kind: 'text', value: '' }],
  ]
  const index = indexResultRows(rows)
  const selected = [rows[2], rows[0], rows[1], rows[3]]
  const typed = selectResultValues(selected, values, index)
  expect(typed[0]).toBe(values[2])
  expect(typed[1]).toBe(values[0])
  expect(typed[2]).toBe(values[1])
  expect(typed[3]).toEqual([{ kind: 'text', value: 'fallback' }])
  expect(rowsToTsv(typed)).toBe('\r\n\\N\r\n00ff\r\nfallback')
  expect(resultGridRows(['column', 'missing'], selected, values, index)).toEqual([
    { __row: '2', c0: '', c1: '' },
    { __row: '0', c0: null, c1: '' },
    { __row: '1', c0: '00ff', c1: '' },
    { __row: '3', c0: 'fallback', c1: '' },
  ])
})

it('以行身份区分同值行，重复引用取首项，响应式行和原始行均能定位', () => {
  const a = ['same'],
    b = ['same']
  const rows = reactive([a, b, a, ...Array.from({ length: 200 }, () => ['other'])])
  const index = indexResultRows(rows)
  expect(index(a)).toBe(0)
  expect(index(rows[0])).toBe(0)
  expect(index(b)).toBe(1)
  expect(index(rows[2])).toBe(0)
  expect(index(['same'])).toBe(-1)
})

it('快照替换、响应式增删及排序重建索引，修改单元格不需要重建', () => {
  const rows = ref(Array.from({ length: 200 }, (_, i) => [`row-${i}`]))
  const index = computed(() => indexResultRows(rows.value))
  const first = index.value
  rows.value[0][0] = 'edited'
  expect(index.value).toBe(first)
  rows.value.reverse()
  expect(index.value).not.toBe(first)
  expect(index.value(rows.value[0])).toBe(0)
  const removed = rows.value.shift()!
  expect(index.value(removed)).toBe(-1)
  rows.value.push(['new'])
  expect(index.value(rows.value.at(-1)!)).toBe(rows.value.length - 1)
  const old = rows.value[0]
  rows.value = markRaw([['replacement']])
  expect(index.value(old)).toBe(-1)
  expect(index.value(rows.value[0])).toBe(0)
})

it('宽表每行只定位一次，结果与原逐列扫描一致', () => {
  const columns = Array.from({ length: 200 }, (_, i) => `column-${i}`)
  const rows = Array.from({ length: 1_000 }, (_, i) => [`${i}`])
  const values: DbValue[][] = rows.map((row) => [{ kind: 'text', value: row[0] }])
  const selected = rows.slice(-50).reverse()
  const index = vi.fn(indexResultRows(rows))
  const grid = resultGridRows(columns, selected, values, index)
  expect(index).toHaveBeenCalledTimes(50)
  expect(grid.map((row) => row.__row)).toEqual(selected.map((row) => String(rows.indexOf(row))))
  expect(grid[0].c0).toBe('999')
  expect(grid[0].c199).toBe('')
  const original = vi.spyOn(rows, 'indexOf')
  selectResultValues(rows, values, index)
  expect(original).not.toHaveBeenCalled()
  original.mockRestore()
})
