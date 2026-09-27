import { afterEach, expect, it, vi } from 'vitest'
import { createResultFilterTask, needsAsyncFilter } from './resultFilterTask'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: ((event: { data: unknown }) => void) | null
  onerror?: (() => void) | null
  onmessageerror?: (() => void) | null
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
  reply() {
    const { id, rows, term } = this.postMessage.mock.lastCall![0] as {
      id: number
      rows: string[][]
      term: string
    }
    this.onmessage?.({
      data: {
        id,
        matches: rows.flatMap((row, i) =>
          row.some((cell) => cell.toLowerCase().includes(term)) ? [i] : []
        ),
      },
    })
  }
}
afterEach(() => {
  vi.unstubAllGlobals()
  FakeWorker.instances = []
})

it('工作批次仅一个在途，全部命中保持顺序与原始行身份', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const rows = Array.from({ length: 15000 }, (_, i) => [`中文 ${i}`, i % 2 ? 'İΣ🙂' : 'other'])
  const task = createResultFilterTask()
  const result = task.run(rows, 'i̇')
  const worker = FakeWorker.instances[0]
  expect(worker.postMessage).toHaveBeenCalledOnce()
  expect(worker.postMessage.mock.lastCall![0].rows.length).toBeLessThan(rows.length)
  while (!worker.terminate.mock.calls.length) worker.reply()
  const actual = await result
  const expected = rows.filter((row) => row.some((cell) => cell.toLowerCase().includes('i̇')))
  expect(actual).toEqual(expected)
  actual.forEach((row, i) => expect(row).toBe(expected[i]))
  expect(worker.terminate).toHaveBeenCalledOnce()
  expect(worker.onmessage).toBeNull()
})

it('单个大单元格完整传递，不截断 Unicode 或限制内容', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const rows = [['x'.repeat(1000000) + '中文🙂']]
  expect(needsAsyncFilter(rows)).toBe(true)
  expect(needsAsyncFilter([['small']])).toBe(false)
  const result = createResultFilterTask().run(rows, '中文🙂')
  const worker = FakeWorker.instances[0]
  expect(worker.postMessage.mock.lastCall![0].rows[0][0]).toBe(rows[0][0])
  worker.reply()
  expect((await result)[0]).toBe(rows[0])
})

it('替换终止旧任务并拒绝等待者，线程故障不会挂起或退回主线程扫描', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const task = createResultFilterTask()
  const first = task.run([['first']], 'first')
  const cancelled = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  const second = task.run([['second']], 'second')
  await cancelled
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce()
  FakeWorker.instances[1].onmessageerror?.()
  await expect(second).rejects.toThrow('无法读取筛选结果')
  expect(FakeWorker.instances[1].terminate).toHaveBeenCalledOnce()
})

it('无效行位置拒绝整项结果并释放线程', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const result = createResultFilterTask().run([['one']], 'one')
  const worker = FakeWorker.instances[0]
  worker.onmessage?.({ data: { id: 1, matches: [1] } })
  await expect(result).rejects.toThrow('行位置无效')
  expect(worker.terminate).toHaveBeenCalledOnce()
})
