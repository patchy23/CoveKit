import { effectScope, markRaw, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { useAsyncResultFilter } from './useAsyncResultFilter'

const task = vi.hoisted(() => ({ run: vi.fn(), destroy: vi.fn() }))
vi.mock('./resultFilterTask', () => ({
  createResultFilterTask: () => task,
  needsAsyncFilter: (rows: string[][]) => rows.length > 512,
}))
afterEach(() => vi.resetAllMocks())

it('小表立即返回并跟踪原地修改，无需线程', async () => {
  const scope = effectScope()
  const rows = ref([['alpha'], ['beta']])
  const filter = scope.run(() =>
    useAsyncResultFilter(
      () => rows.value,
      () => 'alpha'
    )
  )!
  expect(filter.filteredRows.value).toEqual([rows.value[0]])
  rows.value[1][0] = 'alphabet'
  expect(await filter.waitForFilter()).toEqual(rows.value)
  expect(task.run).not.toHaveBeenCalled()
  scope.stop()
})

it('大结果异步交付完整命中，收窄只复查候选，清空恢复原快照', async () => {
  const scope = effectScope()
  const rows = markRaw(Array.from({ length: 600 }, (_, i) => [`alpha ${i}`]))
  const query = ref('alpha')
  let finish!: (rows: string[][]) => void
  task.run.mockImplementation(
    () =>
      new Promise<string[][]>((resolve) => {
        finish = resolve
      })
  )
  const filter = scope.run(() =>
    useAsyncResultFilter(
      () => rows,
      () => query.value
    )
  )!
  expect(filter.filtering.value).toBe(true)
  const waiting = filter.waitForFilter()
  finish([rows[0], rows[1]])
  expect(await waiting).toEqual([rows[0], rows[1]])
  query.value = 'alpha 1'
  expect(task.run).toHaveBeenLastCalledWith([rows[0], rows[1]], 'alpha 1')
  finish([rows[1]])
  await flushPromises()
  expect(filter.filteredRows.value[0]).toBe(rows[1])
  query.value = ''
  expect(filter.filteredRows.value).toBe(rows)
  expect(filter.filtering.value).toBe(false)
  scope.stop()
})

it('条件变化或关闭后忽略迟到结果，等待中的导出明确取消', async () => {
  const scope = effectScope()
  const rows = markRaw(Array.from({ length: 600 }, () => ['original']))
  const query = ref('original')
  let finish!: (rows: string[][]) => void
  task.run.mockImplementation(
    () =>
      new Promise<string[][]>((resolve) => {
        finish = resolve
      })
  )
  const filter = scope.run(() =>
    useAsyncResultFilter(
      () => rows,
      () => query.value
    )
  )!
  const waiting = expect(filter.waitForFilter()).rejects.toMatchObject({ name: 'AbortError' })
  const late = finish
  query.value = ''
  late([rows[0]])
  await waiting
  expect(filter.filteredRows.value).toBe(rows)
  query.value = 'again'
  scope.stop()
  finish([rows[0]])
  await flushPromises()
  expect(filter.filteredRows.value).toEqual([])
  expect(task.destroy).toHaveBeenCalled()
})

it('筛选失败可见，复制和导出不能把失败当作空结果成功', async () => {
  const scope = effectScope()
  task.run.mockRejectedValueOnce(new Error('线程失败'))
  const rows = markRaw(Array.from({ length: 600 }, () => ['original']))
  const filter = scope.run(() =>
    useAsyncResultFilter(
      () => rows,
      () => 'original'
    )
  )!
  await expect(filter.waitForFilter()).rejects.toThrow('线程失败')
  expect(filter.filterError.value).toBe('线程失败')
  expect(filter.filtering.value).toBe(false)
  scope.stop()
})
