/**
 * 框架长任务镜像的行为测试（T11-1/T11-2）
 *
 * 关键口径：增量事件进活跃列表、终态进已完成并截断到 20 条；取消失败要把后端错误码带出来；
 * 重复 start 不会建第二条订阅（否则事件会被处理两次）。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const invokeCommand = vi.fn()
vi.mock('@/core/ipc/ipc', () => ({ invokeCommand: (...args: unknown[]) => invokeCommand(...args) }))

/** 事件订阅替身：scope 内部动态 import 这个模块 */
const listeners: Array<(payload: unknown) => void> = []
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (event: string, handler: (frame: { payload: unknown }) => void) => {
    if (event !== 'framework://task') throw new Error(`未预期的事件：${event}`)
    const adapted = (payload: unknown) => handler({ payload })
    listeners.push(adapted)
    return () => {
      const index = listeners.indexOf(adapted)
      if (index >= 0) listeners.splice(index, 1)
    }
  },
}))

import { useTasksStore } from '@/stores/tasks'
import type { TaskSnapshot } from '@/core/ipc/contracts'

/** 造一个任务快照 */
function snapshot(overrides: Partial<TaskSnapshot> = {}): TaskSnapshot {
  return {
    id: 't1',
    owner: 'storage',
    kind: 'storage.migrate',
    state: 'running',
    progress: 10,
    cancellable: false,
    cancellableReason: null,
    error: null,
    startedAt: 1,
    updatedAt: 2,
    ...overrides,
  }
}

describe('框架长任务镜像', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeCommand.mockReset()
    listeners.length = 0
  })

  it('进入界面即拉全量：活跃与已完成分别落位', async () => {
    invokeCommand.mockResolvedValueOnce({
      active: [snapshot()],
      finished: [snapshot({ id: 't0', state: 'succeeded', progress: 100 })],
    })
    const store = useTasksStore()

    await store.start()

    expect(store.activeCount).toBe(1)
    expect(store.finished).toHaveLength(1)
    expect(store.tasks).toHaveLength(2)
  })

  it('增量事件把任务从活跃移到已完成，并同步失败原因', async () => {
    invokeCommand.mockResolvedValueOnce({ active: [snapshot()], finished: [] })
    const store = useTasksStore()
    await store.start()

    listeners.forEach((deliver) =>
      deliver(
        snapshot({
          state: 'failed',
          error: { code: 'storage.migrate.failed', message: '校验失败' },
          progress: 40,
        })
      )
    )

    expect(store.activeCount).toBe(0)
    expect(store.finished[0]?.state).toBe('failed')
    expect(store.finished[0]?.error?.code).toBe('storage.migrate.failed')
  })

  it('已完成列表保留最近 20 条', async () => {
    invokeCommand.mockResolvedValueOnce({ active: [], finished: [] })
    const store = useTasksStore()
    await store.start()

    for (let index = 0; index < 25; index += 1) {
      listeners.forEach((deliver) => deliver(snapshot({ id: `t${index}`, state: 'succeeded' })))
    }

    expect(store.finished).toHaveLength(20)
    expect(store.finished[0]?.id).toBe('t24')
  })

  it('重复 start 不会建立第二条订阅', async () => {
    invokeCommand.mockResolvedValue({ active: [], finished: [] })
    const store = useTasksStore()

    await store.start()
    await store.start()

    expect(listeners).toHaveLength(1)
    expect(invokeCommand).toHaveBeenCalledTimes(1)
  })
})
