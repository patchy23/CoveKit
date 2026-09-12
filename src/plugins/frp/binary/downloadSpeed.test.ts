import { describe, expect, it } from 'vitest'
import { estimateSpeed, pushSample, type SpeedSample } from './downloadSpeed'

/** 构造采样点 */
function sample(atMs: number, bytes: number): SpeedSample {
  return { at: atMs, received: bytes }
}

describe('estimateSpeed', () => {
  it('窗口内按字节增量与时间跨度算出每秒字节数', () => {
    // 1 秒内多收 2MB → 2MB/s
    const samples = [sample(1000, 0), sample(1500, 1_000_000), sample(2000, 2_000_000)]
    expect(estimateSpeed(samples, 2000)).toBe(2_000_000)
  })

  it('只有一个采样点时返回 null（避免闪一个假数字）', () => {
    expect(estimateSpeed([sample(1000, 0)], 1000)).toBeNull()
    expect(estimateSpeed([], 1000)).toBeNull()
  })

  it('传输停滞时返回 0 而不是 null（让用户看出卡住）', () => {
    const samples = [sample(1000, 500), sample(2000, 500)]
    expect(estimateSpeed(samples, 2000)).toBe(0)
  })

  it('忽略窗口外的旧样本（基于最近的速率而非全程均值）', () => {
    // 早期 1 秒内收了 10MB，随后 2 秒几乎停滞：窗口只看最后 2 秒
    const samples = [
      sample(0, 0),
      sample(1000, 10_000_000),
      sample(2000, 10_000_000),
      sample(3000, 10_100_000),
    ]
    // 仅 [1000,3000] 两个点在窗口内（now - at <= 2000）：2 秒 100KB → 50KB/s
    expect(estimateSpeed(samples, 3000)).toBe(50_000)
  })

  it('时间跨度为 0 时返回 null（同毫秒事件不参与计算）', () => {
    const samples = [sample(1000, 0), sample(1000, 999)]
    expect(estimateSpeed(samples, 1000)).toBeNull()
  })
})

describe('pushSample', () => {
  it('按时间升序追加并裁掉窗口外的旧点', () => {
    let samples: SpeedSample[] = []
    samples = pushSample(samples, sample(0, 0))
    samples = pushSample(samples, sample(1000, 100))
    samples = pushSample(samples, sample(4000, 400))
    // 4000 时刻的窗口只覆盖 >= 2000，前两点都被裁掉
    expect(samples).toEqual([sample(4000, 400)])
  })

  it('同一毫秒的重复事件只保留最后一个', () => {
    let samples: SpeedSample[] = [sample(1000, 0)]
    samples = pushSample(samples, sample(1000, 500))
    expect(samples).toEqual([sample(1000, 500)])
  })

  it('不修改入参数组', () => {
    const original: SpeedSample[] = [sample(0, 0)]
    const next = pushSample(original, sample(500, 10))
    expect(original).toEqual([sample(0, 0)])
    expect(next.length).toBe(2)
  })
})
