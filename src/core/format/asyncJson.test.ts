import { afterEach, expect, it, vi } from 'vitest'
import { createJsonFormatter, JSON_WORKER_THRESHOLD } from './asyncJson'
import { formatJson } from './json'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: (event: { data: unknown }) => void
  onerror?: (event: { message: string }) => void
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
  reply() {
    const { id, text, indent } = this.postMessage.mock.lastCall![0]
    this.onmessage?.({ data: { id, result: formatJson(text, indent) } })
  }
}
afterEach(() => {
  vi.unstubAllGlobals()
  FakeWorker.instances = []
})

it('小文本不启动线程，格式化与压缩沿用相同 JSON 语义', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createJsonFormatter()
  expect(await formatter.run('{"中文":1}', 0)).toEqual(formatJson('{"中文":1}', 0))
  expect(await formatter.run('{bad')).toEqual(formatJson('{bad'))
  expect(FakeWorker.instances).toHaveLength(0)
  formatter.destroy()
})

it('大文本完整处理；取消不接受旧响应，完成立即释放计算线程', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createJsonFormatter()
  const text = JSON.stringify({ text: '中文🙂'.repeat(JSON_WORKER_THRESHOLD) })
  const first = formatter.run(text)
  const cancelled = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  formatter.cancel()
  await cancelled
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce()
  const next = formatter.run(text, 0)
  FakeWorker.instances[0].reply()
  FakeWorker.instances[1].reply()
  expect(await next).toEqual(formatJson(text, 0))
  expect(FakeWorker.instances[1].terminate).toHaveBeenCalledOnce()
})

it('线程故障明确返回失败，不转回主线程重跑大解析', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createJsonFormatter()
  const result = formatter.run(' '.repeat(JSON_WORKER_THRESHOLD + 1))
  FakeWorker.instances[0].onerror?.({ message: 'worker failed' })
  await expect(result).rejects.toThrow('worker failed')
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce()
})
