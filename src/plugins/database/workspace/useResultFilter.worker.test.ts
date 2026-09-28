import { effectScope, markRaw, ref } from 'vue'
import { beforeEach, expect, it, vi } from 'vitest'
const task = vi.hoisted(() => ({ run: vi.fn(), cancel: vi.fn(), destroy: vi.fn() }))
vi.mock('./resultFilterTask', () => ({
  needsAsyncFilter: (rows: string[][]) => rows.length > 2,
  createResultFilterTask: () => task,
}))
import { useResultFilter } from './useResultFilter'

beforeEach(() => vi.resetAllMocks())
function deferred() {
  let resolve!: (value: Uint32Array) => void
  const promise = new Promise<Uint32Array>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

it('导出等待当前筛选版本，替换关键词后旧结果不能覆盖新结果或完成旧导出', async () => {
  const scope = effectScope()
  const rows = markRaw([['a'], ['b'], ['c']])
  const query = ref('a')
  const first = deferred(),
    second = deferred()
  task.run.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
  const filtered = scope.run(() =>
    useResultFilter(
      () => rows,
      () => query.value
    )
  )!
  expect(filtered.value).toEqual([])
  expect(filtered.busy.value).toBe(true)
  const oldExport = expect(filtered.ready()).rejects.toMatchObject({ name: 'AbortError' })
  query.value = 'b'
  const newExport = filtered.ready()
  second.resolve(new Uint32Array([1]))
  expect(await newExport).toEqual([rows[1]])
  first.resolve(new Uint32Array([0]))
  await oldExport
  expect(filtered.value[0]).toBe(rows[1])
  expect(filtered.busy.value).toBe(false)
  scope.stop()
  expect(task.destroy).toHaveBeenCalledOnce()
})

it('Worker 故障明确显示且导出失败，不在主线程重新扫描正文', async () => {
  const scope = effectScope()
  const rows = markRaw([['a'], ['b'], ['c']])
  const scans = rows.map((row) => vi.spyOn(row, 'some'))
  task.run.mockRejectedValue(new Error('工作器故障'))
  const filtered = scope.run(() =>
    useResultFilter(
      () => rows,
      () => 'a'
    )
  )!
  await expect(filtered.ready()).rejects.toThrow('工作器故障')
  expect(filtered.error.value).toBe('工作器故障')
  expect(filtered.busy.value).toBe(false)
  scans.forEach((scan) => expect(scan).not.toHaveBeenCalled())
  scope.stop()
})

it('清空关键词取消计算并立即恢复完整模型，替换数据使用新快照', async () => {
  const scope = effectScope()
  const rows = ref(markRaw([['a'], ['b'], ['c']]))
  const query = ref('a')
  const first = deferred()
  task.run.mockReturnValueOnce(first.promise).mockResolvedValue(new Uint32Array([2]))
  const filtered = scope.run(() =>
    useResultFilter(
      () => rows.value,
      () => query.value
    )
  )!
  const old = expect(filtered.ready()).rejects.toMatchObject({ name: 'AbortError' })
  query.value = ''
  expect(filtered.value).toBe(rows.value)
  expect(task.destroy).toHaveBeenCalled()
  first.resolve(new Uint32Array([0]))
  await old
  rows.value = markRaw([['x'], ['y'], ['z']])
  query.value = 'z'
  expect(await filtered.ready()).toEqual([rows.value[2]])
  expect(task.run).toHaveBeenLastCalledWith(rows.value, 'z')
  scope.stop()
})
