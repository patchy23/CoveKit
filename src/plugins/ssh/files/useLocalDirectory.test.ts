import { effectScope } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { useLocalDirectory } from './useLocalDirectory'

const mock = vi.hoisted(() => ({ prepare: vi.fn(), cancel: vi.fn(async () => {}), list: vi.fn() }))
vi.mock('../ipc', () => ({
  ipc: {
    sshLocalListPrepare: mock.prepare,
    sshLocalListCancel: mock.cancel,
    sshLocalList: mock.list,
  },
}))
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

it('关闭发生在登记返回前时释放登记且不发起扫描', async () => {
  const pending = deferred<string>()
  mock.prepare.mockReturnValue(pending.promise)
  const scope = effectScope()
  const read = scope.run(() => useLocalDirectory(vi.fn()))!
  const result = read.read('old')
  scope.stop()
  pending.resolve('late')
  expect(await result).toBeUndefined()
  expect(mock.list).not.toHaveBeenCalled()
  expect(mock.cancel).toHaveBeenCalledExactlyOnceWith('late')
})

it('快速切换取消旧扫描，旧失败不覆盖新结果，正常完成和关闭都释放', async () => {
  const old = deferred<unknown>()
  mock.prepare.mockResolvedValueOnce('old').mockResolvedValueOnce('new')
  mock.list.mockReturnValueOnce(old.promise).mockResolvedValueOnce({ path: 'new', files: [] })
  const scope = effectScope(),
    error = vi.fn()
  const read = scope.run(() => useLocalDirectory(error))!
  const first = read.read('old')
  await flushPromises()
  expect(await read.read('new')).toEqual({ path: 'new', files: [] })
  old.reject(new Error('旧扫描取消'))
  expect(await first).toBeUndefined()
  expect(mock.list.mock.calls).toEqual([
    ['old', 'old'],
    ['new', 'new'],
  ])
  expect(mock.cancel.mock.calls).toEqual([['old'], ['new']])
  scope.stop()
  expect(error).not.toHaveBeenCalled()
})

it('各页面独立释放；当前错误与取消失败仍可见', async () => {
  mock.prepare.mockResolvedValueOnce('a').mockResolvedValueOnce('b')
  const a = deferred<unknown>(),
    b = deferred<unknown>()
  mock.list.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise)
  const firstScope = effectScope(),
    secondScope = effectScope(),
    error = vi.fn()
  const first = firstScope.run(() => useLocalDirectory(error))!.read('a')
  const second = secondScope.run(() => useLocalDirectory(error))!.read('b')
  await flushPromises()
  firstScope.stop()
  expect(mock.cancel.mock.calls).toEqual([['a']])
  a.resolve({ path: 'a', files: [] })
  expect(await first).toBeUndefined()
  mock.cancel.mockRejectedValueOnce(new Error('取消失败'))
  const failure = expect(second).rejects.toThrow('目录无权限')
  b.reject(new Error('目录无权限'))
  await failure
  await flushPromises()
  expect(error).toHaveBeenCalledWith(expect.objectContaining({ message: '取消失败' }))
  secondScope.stop()
})
