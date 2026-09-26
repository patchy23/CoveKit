import { afterEach, expect, it, vi } from 'vitest'
import { createSqlFormatter, SQL_WORKER_THRESHOLD } from './asyncSql'
import { formatSql } from './sql'

class FakeWorker {
  static instances: FakeWorker[] = []
  onmessage?: (event: { data: unknown }) => void
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() {
    FakeWorker.instances.push(this)
  }
  reply() {
    const { id, text } = this.postMessage.mock.lastCall![0]
    this.onmessage?.({ data: { id, result: formatSql(text) } })
  }
}
afterEach(() => {
  vi.unstubAllGlobals()
  FakeWorker.instances = []
})

it('小 SQL 保持同步，后台大 SQL 与原格式化语义一致，完成立即释放', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createSqlFormatter()
  expect(formatter.run('select  1;')).toBe('select 1;')
  expect(FakeWorker.instances).toHaveLength(0)
  const sql = `select '${'x'.repeat(SQL_WORKER_THRESHOLD)}'; -- comment\nselect 2;`
  const result = formatter.run(sql)
  FakeWorker.instances[0].reply()
  expect(await result).toBe(formatSql(sql))
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce()
})

it('改输入与销毁都会终止在途 SQL，旧响应不能完成新任务', async () => {
  vi.stubGlobal('Worker', FakeWorker)
  const formatter = createSqlFormatter()
  const sql = 'x'.repeat(SQL_WORKER_THRESHOLD + 1)
  const first = expect(formatter.run(sql)).rejects.toMatchObject({ name: 'AbortError' })
  formatter.run('select 1')
  await first
  const second = expect(formatter.run(sql)).rejects.toMatchObject({ name: 'AbortError' })
  FakeWorker.instances[0].reply()
  formatter.destroy()
  await second
  expect(FakeWorker.instances[1].terminate).toHaveBeenCalledOnce()
})
