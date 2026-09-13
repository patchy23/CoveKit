/**
 * 作用域副作用的行为测试（T10-5/T10-7）
 *
 * 三个必须成立的语义：卸载早于订阅建立 → 立刻解绑；dispose 之后不再创建定时器；
 * 清理回调逆序执行且逐个失败都被收集。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

/** 事件订阅的可控替身：`holdListen` 打开时订阅不会立即 resolve，用于复现「卸载早于 resolve」 */
const listeners: Array<{ event: string; deliver: (payload: unknown) => void }> = []
let listenRecords: Array<{ unlistenCalls: number }> = []
let heldResolvers: Array<() => void> = []
let holdListen = false
let listenRejects: Error | null = null

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (event: { payload: unknown }) => void) => {
    if (listenRejects) return Promise.reject(listenRejects)
    const record = { unlistenCalls: 0 }
    const register = () => {
      const entry = { event, deliver: (payload: unknown) => handler({ payload }) }
      listeners.push(entry)
      listenRecords.push(record)
      return () => {
        record.unlistenCalls += 1
        // 解绑即停止投递：与真实 listen 返回的 unlisten 语义一致
        const index = listeners.indexOf(entry)
        if (index >= 0) listeners.splice(index, 1)
      }
    }
    if (holdListen) {
      return new Promise<() => void>((resolve) => {
        heldResolvers.push(() => resolve(register()))
      })
    }
    return Promise.resolve(register())
  },
}))

import { createScope, throttledInterval } from './scope'

describe('作用域副作用', () => {
  beforeEach(() => {
    listeners.length = 0
    heldResolvers = []
    listenRecords = []
    holdListen = false
    listenRejects = null
  })

  it('卸载早于订阅 resolve 时立刻解绑，不留悬挂监听', async () => {
    const scope = createScope('frp')
    const handler = vi.fn()
    holdListen = true
    const pending = scope.listenEvent('frp://status', handler)
    const dispose = scope.dispose()

    // 订阅此刻才 resolve：作用域已释放，必须立刻解绑且不加入投递列表
    heldResolvers.forEach((resolve) => resolve())
    await pending
    await dispose

    expect(listenRecords[0].unlistenCalls).toBe(1)
    expect(listeners).toHaveLength(0)
    expect(handler).not.toHaveBeenCalled()
  })

  it('正常订阅在 dispose 时解绑且回调期间可用', async () => {
    const scope = createScope('ssh')
    const handler = vi.fn()
    await scope.listenEvent('ssh://session', handler)
    listeners.forEach((entry) => entry.deliver({ id: 's1' }))
    expect(handler).toHaveBeenCalledWith({ id: 's1' })

    await scope.dispose()

    expect(listenRecords[0].unlistenCalls).toBe(1)
    expect(listeners).toHaveLength(0)
  })

  it('订阅建立失败记录下来，不影响其他订阅与后续清理', async () => {
    const scope = createScope('database')
    listenRejects = new Error('事件通道不可用')
    await scope.listenEvent('db://state', vi.fn())
    listenRejects = null
    const okHandler = vi.fn()
    await scope.listenEvent('db://query', okHandler)

    const result = await scope.dispose()

    expect(result.failures[0].message).toContain('db://state')
    expect(listeners).toHaveLength(0)
    expect(listenRecords).toHaveLength(1)
    expect(listenRecords[0].unlistenCalls).toBe(1)
  })

  it('dispose 之后拒绝创建定时器并留痕', async () => {
    vi.useFakeTimers()
    const scope = createScope('ssh')
    const tick = vi.fn()
    expect(scope.interval(tick, 1000)).toBe(true)
    await scope.dispose()

    expect(scope.interval(tick, 1000)).toBe(false)
    expect(scope.timeout(tick, 1000)).toBe(false)
    vi.advanceTimersByTime(5000)
    expect(tick).not.toHaveBeenCalled()
    const result = await scope.dispose()
    expect(result.failures.filter((f) => f.message.includes('拒绝创建定时器'))).toHaveLength(2)
    vi.useRealTimers()
  })

  it('dispose 逆序执行清理回调，逐个失败都被收集且不中断', async () => {
    const order: string[] = []
    const scope = createScope('http-ws')
    scope.addDispose(() => {
      order.push('first')
    })
    scope.addDispose(() => {
      throw new Error('第二个清理失败')
    })
    scope.addDispose(() => {
      order.push('third')
    })

    const result = await scope.dispose()

    expect(order).toEqual(['third', 'first'])
    expect(result.failures).toEqual([{ owner: 'http-ws', message: '第二个清理失败' }])
  })

  it('重复 dispose 幂等，不重复执行清理', async () => {
    const cleanup = vi.fn()
    const scope = createScope('frp')
    scope.addDispose(cleanup)

    await scope.dispose()
    await scope.dispose()

    expect(cleanup).toHaveBeenCalledTimes(1)
  })

  it('隐藏时降频但仍保留可见间隔作为下限', () => {
    expect(throttledInterval(5000, 30000, false)).toBe(5000)
    expect(throttledInterval(5000, 30000, true)).toBe(30000)
    // 非法值回退到可见间隔，避免 setInterval(0) 变成忙等
    expect(throttledInterval(5000, 0, true)).toBe(5000)
  })
})
