import { expect, it, vi } from 'vitest'
import { sseConsumer } from './sseConsumer'
import type { SseUpdate } from './contracts'

const batch = (sequence: number): SseUpdate => ({
  type: 'events',
  sequence,
  events: [{ event: 'message', id: String(sequence), data: `中文-${sequence}` }],
})
const flush = async () => {
  for (let count = 0; count < 10; count++) await Promise.resolve()
}

it('立即交付完整事件，在状态更新后合并同轮确认，无刷新定时器', async () => {
  const receive = vi.fn()
  const acknowledge = vi.fn().mockResolvedValue(undefined)
  const consume = sseConsumer(receive, acknowledge, vi.fn())
  consume(batch(1))
  consume(batch(2))
  expect(receive.mock.calls.map(([update]) => update.event.data)).toEqual(['中文-1', '中文-2'])
  expect(acknowledge).not.toHaveBeenCalled()
  await flush()
  expect(acknowledge.mock.calls).toEqual([[2]])
})

it('确认在途时只保留最新序号，后续事件不等待前一次确认', async () => {
  let resolve!: () => void
  const acknowledge = vi
    .fn()
    .mockImplementationOnce(() => new Promise<void>((done) => (resolve = done)))
    .mockResolvedValue(undefined)
  const receive = vi.fn()
  const consume = sseConsumer(receive, acknowledge, vi.fn())
  consume(batch(1))
  await flush()
  consume(batch(2))
  consume(batch(3))
  await flush()
  expect(receive).toHaveBeenCalledTimes(3)
  expect(acknowledge.mock.calls).toEqual([[1]])
  resolve()
  await flush()
  expect(acknowledge.mock.calls).toEqual([[1], [3]])
})

it('确认失败停止本连接，并报告停止失败，不留下静默挂起的生产者', async () => {
  const receive = vi.fn()
  const stop = vi.fn().mockRejectedValue(new Error('stop failed'))
  const acknowledge = vi.fn().mockRejectedValue(new Error('ack failed'))
  const consume = sseConsumer(receive, acknowledge, stop)
  consume(batch(1))
  await flush()
  expect(stop).toHaveBeenCalledOnce()
  expect(receive.mock.calls.at(-1)?.[0]).toEqual({
    type: 'error',
    message: expect.stringContaining('stop failed'),
  })
  const count = receive.mock.calls.length
  consume(batch(2))
  expect(receive).toHaveBeenCalledTimes(count)
})

it('终态取消尚未发出的确认并忽略迟到事件', async () => {
  const receive = vi.fn()
  const acknowledge = vi.fn()
  const consume = sseConsumer(receive, acknowledge, vi.fn())
  consume(batch(1))
  consume({ type: 'error', message: 'invalid stream' })
  consume(batch(2))
  await flush()
  expect(acknowledge).not.toHaveBeenCalled()
  expect(receive).toHaveBeenCalledTimes(2)
})
