import { afterEach, expect, it, vi } from 'vitest'
import { useTtsProgress } from './useTtsProgress'

afterEach(() => vi.useRealTimers())

it('连接 30 秒和接收 120 秒无进展只提示，继续等待不改真实进展时间', () => {
  vi.useFakeTimers()
  vi.setSystemTime(1000)
  const progress = useTtsProgress()
  progress.start('job')
  vi.advanceTimersByTime(29_999)
  expect(progress.stalled.value).toBe(false)
  vi.advanceTimersByTime(1)
  expect(progress.stalled.value).toBe(true)
  progress.keepWaiting()
  expect(progress.current.value?.lastProgressAt).toBe(1000)
  expect(progress.stalled.value).toBe(false)
  progress.receive({
    jobId: 'job',
    sequence: 1,
    phase: 'receiving',
    lastProgressAt: Date.now(),
    bytes: 0,
  })
  vi.advanceTimersByTime(119_999)
  expect(progress.stalled.value).toBe(false)
  vi.advanceTimersByTime(1)
  expect(progress.stalled.value).toBe(true)
  progress.receive({
    jobId: 'job',
    sequence: 2,
    phase: 'writing',
    lastProgressAt: Date.now(),
    bytes: 100,
  })
  expect(progress.stalled.value).toBe(false)
  expect(progress.label.value).toBe('正在写入音频')
  progress.stop()
  expect(vi.getTimerCount()).toBe(0)
})

it('重复、乱序和旧请求进展不清除停滞提示或恢复已取消状态', () => {
  vi.useFakeTimers()
  const progress = useTtsProgress()
  progress.start('old')
  const old = {
    jobId: 'old',
    sequence: 3,
    phase: 'receiving' as const,
    lastProgressAt: Date.now(),
    bytes: 8,
  }
  progress.receive(old)
  progress.start('new')
  progress.receive(old)
  expect(progress.current.value?.jobId).toBe('new')
  const current = { ...old, jobId: 'new' }
  progress.receive(current)
  vi.advanceTimersByTime(120_000)
  progress.receive(current)
  progress.receive({ ...current, sequence: 2 })
  expect(progress.stalled.value).toBe(true)
  progress.stop()
  progress.receive({ ...current, sequence: 4 })
  expect(progress.current.value).toBeUndefined()
  expect(vi.getTimerCount()).toBe(0)
})
