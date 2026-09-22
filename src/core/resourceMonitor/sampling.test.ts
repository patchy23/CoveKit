import { afterEach, describe, expect, it, vi } from 'vitest'
import { createResourcePoller, memoryLabel, resourceTotals, type TimedSnapshot } from './sampling'

function sample(time: number, cpu: number, identity = 'one'): TimedSnapshot {
  return {
    time,
    value: {
      partial: false,
      coverage: '',
      missingProcesses: 0,
      logicalCpus: 4,
      processes: [
        {
          pid: 12,
          identity,
          kind: 'main',
          cpuSeconds: cpu,
          residentBytes: 100,
          privateBytes: null,
          threads: 3,
          handles: null,
        },
      ],
    },
  }
}
afterEach(() => vi.useRealTimers())
describe('资源统计口径', () => {
  it('区分首次采样、整机 CPU、不可用字段与 PID 复用', () => {
    expect(resourceTotals(sample(0, 1)).cpu).toBeNull()
    const next = resourceTotals(sample(1000, 2), sample(0, 1))
    expect(next.cpu).toBe(25)
    expect(next.resident).toBe(100)
    expect(next.privateBytes).toBeNull()
    expect(next.handles).toBeNull()
    expect(resourceTotals(sample(1000, 100, 'reused'), sample(0, 1)).cpu).toBeNull()
    expect(memoryLabel(null)).toBe('—')
    expect(memoryLabel(1024 ** 2)).toBe('1 MiB')
  })
  it('停止期间的迟到请求不回填，重开也不并发采样', async () => {
    vi.useFakeTimers()
    let resolve!: (value: number) => void
    const read = vi.fn(
      () =>
        new Promise<number>((done) => {
          resolve = done
        })
    )
    const receive = vi.fn()
    const poller = createResourcePoller(read, receive, vi.fn())
    poller.start()
    poller.stop()
    poller.start()
    expect(read).toHaveBeenCalledTimes(1)
    resolve(1)
    await vi.advanceTimersByTimeAsync(0)
    expect(receive).not.toHaveBeenCalled()
    expect(read).toHaveBeenCalledTimes(2)
    resolve(2)
    await Promise.resolve()
    expect(receive).toHaveBeenCalledWith(2)
    poller.stop()
    await vi.advanceTimersByTimeAsync(5000)
    expect(read).toHaveBeenCalledTimes(2)
  })
  it('采样失败显式报告并串行重试', async () => {
    vi.useFakeTimers()
    const read = vi.fn().mockRejectedValueOnce(new Error('读取失败')).mockResolvedValue(2)
    const fail = vi.fn(),
      receive = vi.fn()
    const poller = createResourcePoller(read, receive, fail)
    poller.start()
    await vi.advanceTimersByTimeAsync(1000)
    expect(fail).toHaveBeenCalledTimes(1)
    expect(receive).toHaveBeenCalledWith(2)
    poller.stop()
  })
})
