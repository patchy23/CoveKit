import { afterEach, expect, it, vi } from 'vitest'
import {
  createRequestValidator,
  needsJsonWorker,
  REQUEST_JSON_WORKER_THRESHOLD,
} from './requestValidation'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: (event: { data: unknown }) => void
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  FakeWorker.instances = []
})

it('普通 JSON 保持同步校验，不创建线程，不修改请求正文', () => {
  vi.stubGlobal('Worker', FakeWorker)
  const validator = createRequestValidator()
  expect(validator.run('  {"中文": [1, true, null]}\n')).toBe(true)
  expect(validator.run('123')).toBe(true)
  expect(validator.run('{invalid')).toBe(false)
  expect(FakeWorker.instances).toHaveLength(0)
  validator.destroy()
})

it('超大正文复用空闲线程，只接收结论；取消或空闲过期后释放线程', async () => {
  vi.useFakeTimers()
  vi.stubGlobal('Worker', FakeWorker)
  const validator = createRequestValidator()
  const text = '[0' + ',0'.repeat(300) + ']' + ' '.repeat(REQUEST_JSON_WORKER_THRESHOLD)
  const first = validator.run(text)
  const worker = FakeWorker.instances[0]
  const payload = worker.postMessage.mock.lastCall![0]
  expect(payload.text).toBe(text)
  worker.onmessage?.({ data: { id: payload.id, result: true } })
  expect(await first).toBe(true)
  expect(worker.terminate).not.toHaveBeenCalled()
  const next = validator.run(text)
  expect(FakeWorker.instances).toHaveLength(1)
  const cancelled = expect(next).rejects.toMatchObject({ name: 'AbortError' })
  validator.cancel()
  await cancelled
  expect(worker.terminate).toHaveBeenCalledOnce()
  const last = validator.run(text)
  const current = FakeWorker.instances[1]
  current.onmessage?.({ data: { id: current.postMessage.mock.lastCall![0].id, result: false } })
  expect(await last).toBe(false)
  vi.advanceTimersByTime(60_000)
  expect(current.terminate).toHaveBeenCalledOnce()
})

it('长字段直接校验，复杂大正文和更大的原子值进入后台，路径选择不改变校验结论', () => {
  const long = JSON.stringify({ text: '中,{}[]:'.repeat(2 * 1024 * 1024) })
  expect(needsJsonWorker(long)).toBe(false)
  expect(
    needsJsonWorker('[0' + ',0'.repeat(300) + ']' + ' '.repeat(REQUEST_JSON_WORKER_THRESHOLD))
  ).toBe(true)
  expect(needsJsonWorker(' '.repeat(32 * 1024 * 1024))).toBe(true)
  expect(needsJsonWorker('{"value":"a\\"b"}' + ' '.repeat(REQUEST_JSON_WORKER_THRESHOLD))).toBe(
    false
  )
})
