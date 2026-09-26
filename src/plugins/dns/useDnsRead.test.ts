import { effectScope } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { useDnsRead } from './useDnsRead'

const mock = vi.hoisted(() => ({ prepare: vi.fn(), cancel: vi.fn(async () => {}) }))
vi.mock('./ipc', () => ({ ipc: { dnsReadPrepare: mock.prepare, dnsReadCancel: mock.cancel } }))
afterEach(() => vi.resetAllMocks())

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}

it('登记迟到时仅释放登记，不启动已关闭页面的网络查询', async () => {
  const pending = deferred<string>()
  mock.prepare.mockReturnValue(pending.promise)
  const scope = effectScope(),
    read = vi.fn(),
    error = vi.fn()
  const request = scope.run(() => useDnsRead(error))!
  const result = request.run(read)
  scope.stop()
  pending.resolve('late')
  expect(await result).toBeUndefined()
  expect(read).not.toHaveBeenCalled()
  expect(mock.cancel).toHaveBeenCalledExactlyOnceWith('late')
  expect(error).not.toHaveBeenCalled()
})

it('新请求取消旧请求，旧失败不改变新请求的 busy 或结果', async () => {
  mock.prepare.mockResolvedValueOnce('old').mockResolvedValueOnce('new')
  const scope = effectScope(),
    error = vi.fn()
  const request = scope.run(() => useDnsRead(error))!
  const old = deferred<string>(),
    next = deferred<string>()
  const first = request.run(() => old.promise)
  await flushPromises()
  const second = request.run(() => next.promise)
  await flushPromises()
  expect(mock.cancel).toHaveBeenCalledExactlyOnceWith('old')
  old.reject(new Error('旧请求已取消'))
  expect(await first).toBeUndefined()
  expect(request.busy.value).toBe(true)
  next.resolve('新内容')
  expect(await second).toBe('新内容')
  expect(request.busy.value).toBe(false)
  expect(mock.cancel.mock.calls).toEqual([['old'], ['new']])
  scope.stop()
  expect(error).not.toHaveBeenCalled()
})

it('当前读错误仍抛出，取消 IPC 失败不会被静默吞掉', async () => {
  mock.prepare.mockResolvedValue('active')
  mock.cancel.mockRejectedValue(new Error('取消失败'))
  const scope = effectScope(),
    error = vi.fn()
  const request = scope.run(() => useDnsRead(error))!
  await expect(
    request.run(async () => {
      throw new Error('读取失败')
    })
  ).rejects.toThrow('读取失败')
  await flushPromises()
  expect(error).toHaveBeenCalledWith(expect.objectContaining({ message: '取消失败' }))
  scope.stop()
})
