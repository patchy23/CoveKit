import { expect, it, vi } from 'vitest'
import { createWorkerTask } from './workerTask'

function setup() {
  const instances: Worker[] = []
  const create = vi.fn(() => {
    const worker = { postMessage: vi.fn(), terminate: vi.fn() } as unknown as Worker
    instances.push(worker)
    return worker
  })
  return { instances, create, task: createWorkerTask<{ text: string }, string>(create, '计算失败') }
}

it.each(['decode', 'null', 'missing'] as const)(
  '%s 异常结束 Promise、释放线程和回调，随后能重试',
  async (mode) => {
    const { instances, task } = setup()
    const result = task.run({ text: 'source' })
    const rejected = expect(result).rejects.toThrow('计算失败')
    const worker = instances[0]
    if (mode === 'decode') worker.onmessageerror!(new MessageEvent('messageerror'))
    else
      worker.onmessage!(new MessageEvent('message', { data: mode === 'null' ? null : { id: 1 } }))
    await rejected
    expect(worker.terminate).toHaveBeenCalledOnce()
    expect(worker.onmessage).toBeNull()
    expect(worker.onerror).toBeNull()
    expect(worker.onmessageerror).toBeNull()
    const next = task.run({ text: 'new' })
    const id = vi.mocked(instances[1].postMessage).mock.calls[0][0].id
    instances[1].onmessage!(new MessageEvent('message', { data: { id, result: 'done' } }))
    expect(await next).toBe('done')
    expect(instances[1].terminate).toHaveBeenCalledOnce()
  }
)

it('替换任务拒绝旧等待者，已经排队的旧错误不能结束新任务', async () => {
  const { instances, task } = setup()
  const first = task.run({ text: 'old' })
  const cancelled = expect(first).rejects.toMatchObject({ name: 'AbortError' })
  const lateError = instances[0].onmessageerror!
  const next = task.run({ text: 'new' })
  await cancelled
  lateError.call(instances[0], new MessageEvent('messageerror'))
  const current = instances[1]
  expect(current.terminate).not.toHaveBeenCalled()
  const id = vi.mocked(current.postMessage).mock.calls[0][0].id
  current.onmessage!(new MessageEvent('message', { data: { id, result: 'new result' } }))
  expect(await next).toBe('new result')
})

it('postMessage 同步失败仍释放已创建的线程', async () => {
  const worker = {
    postMessage: vi.fn(() => {
      throw new DOMException('无法复制输入', 'DataCloneError')
    }),
    terminate: vi.fn(),
  } as unknown as Worker
  const task = createWorkerTask(() => worker, '计算失败')
  await expect(task.run({ text: 'source' })).rejects.toMatchObject({ name: 'DataCloneError' })
  expect(worker.terminate).toHaveBeenCalledOnce()
  expect(worker.onmessage).toBeNull()
})

it('复用线程后旧响应不覆盖新任务，空闲回收计时不打断新计算', async () => {
  vi.useFakeTimers()
  try {
    const worker = { postMessage: vi.fn(), terminate: vi.fn() } as unknown as Worker
    const create = vi.fn(() => worker)
    const task = createWorkerTask<{ text: string }, string>(create, '计算失败', 1000)
    const first = task.run({ text: 'first' })
    const lateReply = worker.onmessage!
    lateReply.call(worker, new MessageEvent('message', { data: { id: 1, result: 'first' } }))
    expect(await first).toBe('first')
    const next = task.run({ text: 'next' })
    vi.advanceTimersByTime(1000)
    expect(worker.terminate).not.toHaveBeenCalled()
    expect(create).toHaveBeenCalledOnce()
    lateReply.call(worker, new MessageEvent('message', { data: { id: 1, result: 'stale' } }))
    const id = vi.mocked(worker.postMessage).mock.lastCall![0].id
    worker.onmessage!(new MessageEvent('message', { data: { id, result: 'next' } }))
    expect(await next).toBe('next')
    vi.advanceTimersByTime(1000)
    expect(worker.terminate).toHaveBeenCalledOnce()
  } finally {
    vi.useRealTimers()
  }
})
