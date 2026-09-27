import { afterEach, expect, it, vi } from 'vitest'
import { createResultFilterTask } from './resultFilterTask'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: (event: { data: unknown }) => void
  onerror?: () => void
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
  reply(data: unknown) {
    this.onmessage?.({ data })
  }
  last() {
    return this.postMessage.mock.lastCall![0]
  }
  upload() {
    const begin = this.last()
    this.reply({ type: 'ready', version: begin.version })
    while (this.last().type === 'rows') {
      const batch = this.last()
      this.reply({ type: 'rows', version: batch.version, batch: batch.batch })
    }
  }
}
afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  FakeWorker.instances = []
})

it('一次传输后只发关键词；编辑快照只发送变化的行，结果按索引返回', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const task = createResultFilterTask()
  const rows = [['Alpha'], ['Beta']]
  const first = task.run(rows, 'a')
  const worker = FakeWorker.instances[0]
  worker.upload()
  const query = worker.last()
  worker.reply({ ...query, type: 'result', indices: new Uint32Array([0, 1]) })
  expect(await first).toEqual(new Uint32Array([0, 1]))
  worker.postMessage.mockClear()
  const next = task.run(rows, 'beta')
  expect(worker.postMessage).toHaveBeenCalledOnce()
  expect(worker.last()).toMatchObject({ type: 'query', term: 'beta', version: query.version })
  worker.reply({ ...worker.last(), type: 'result', indices: new Uint32Array([1]) })
  await next
  worker.postMessage.mockClear()
  const changed = task.run([rows[0], ['Gamma']], 'gamma')
  expect(worker.last()).toMatchObject({ type: 'begin', reset: false })
  worker.upload()
  const sent = worker.postMessage.mock.calls
    .map((call) => call[0])
    .filter((item) => item.type === 'rows')
  expect(sent).toHaveLength(1)
  expect(sent[0].changes).toEqual([{ index: 1, row: ['Gamma'] }])
  worker.reply({ ...worker.last(), type: 'result', indices: new Uint32Array([1]) })
  await changed
  task.destroy()
})

it('初始化期间替换关键词不重复传输；旧结果和旧错误不能覆盖新请求', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const task = createResultFilterTask()
  const rows = [['a'], ['b']]
  const first = task.run(rows, 'a')
  const rejected = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  const second = task.run(rows, 'b')
  await rejected
  const worker = FakeWorker.instances[0]
  expect(worker.postMessage).toHaveBeenCalledOnce()
  worker.upload()
  expect(worker.last().term).toBe('b')
  worker.reply({ ...worker.last(), type: 'result', id: 1, indices: new Uint32Array([0]) })
  worker.reply({ ...worker.last(), type: 'result', indices: new Uint32Array([1]) })
  await second
  const oldError = worker.onerror!
  task.destroy()
  const third = task.run(rows, 'a')
  oldError()
  const current = FakeWorker.instances[1]
  expect(current.terminate).not.toHaveBeenCalled()
  current.upload()
  current.reply({ ...current.last(), type: 'result', indices: new Uint32Array([0]) })
  await third
  task.destroy()
})

it('失败和乱序索引可见，取消与空闲过期释放数据线程', async () => {
  vi.useFakeTimers()
  vi.stubGlobal('Worker', FakeWorker)
  const task = createResultFilterTask()
  const bad = task.run([['a'], ['b']], 'a')
  const rejected = expect(bad).rejects.toThrow('行索引无效')
  const worker = FakeWorker.instances[0]
  worker.upload()
  worker.reply({ ...worker.last(), type: 'result', indices: new Uint32Array([1, 0]) })
  await rejected
  expect(worker.terminate).toHaveBeenCalledOnce()
  const result = task.run([['ok']], 'ok')
  const current = FakeWorker.instances[1]
  current.upload()
  current.reply({ ...current.last(), type: 'result', indices: new Uint32Array([0]) })
  await result
  vi.advanceTimersByTime(60_000)
  expect(current.terminate).toHaveBeenCalledOnce()
  const cancelled = task.run([['pending']], 'pending')
  const cancellation = expect(cancelled).rejects.toMatchObject({ name: 'AbortError' })
  task.cancel()
  await cancellation
  expect(FakeWorker.instances[2].terminate).toHaveBeenCalledOnce()
})
