import { afterEach, expect, it, vi } from 'vitest'
import { createEditorBridge } from './bridge'
import { acceptSnapshot, type EditorSnapshot } from './protocol'
afterEach(() => vi.useRealTimers())
it('双向请求回执与远端错误正确关联', async () => {
  const left = createEditorBridge(
    async (message) => right.receive(message),
    () => 'main'
  )
  const right = createEditorBridge(
    async (message) => left.receive(message),
    (type, value) => {
      if (type === 'fail') throw new Error('保存失败')
      return value
    }
  )
  expect(await left.request('snapshot', { active: '/a' })).toEqual({ active: '/a' })
  expect(await right.request('main')).toBe('main')
  await expect(left.request('fail')).rejects.toThrow('保存失败')
  left.dispose()
  right.dispose()
})
it('超时及窗口销毁会释放等待，不伪装操作成功', async () => {
  vi.useFakeTimers()
  const bridge = createEditorBridge(
    async () => {},
    () => {},
    20
  )
  const timeout = expect(bridge.request('dock')).rejects.toThrow('超时')
  await vi.advanceTimersByTimeAsync(21)
  await timeout
  const closed = expect(bridge.request('save')).rejects.toThrow('通信已关闭')
  bridge.dispose()
  await closed
})
it('迟到快照不能覆盖移回时的较新内容', () => {
  expect(acceptSnapshot(3, { sequence: 2 } as EditorSnapshot)).toBe(false)
  expect(acceptSnapshot(3, { sequence: 3 } as EditorSnapshot)).toBe(false)
  expect(acceptSnapshot(3, { sequence: 4 } as EditorSnapshot)).toBe(true)
})
