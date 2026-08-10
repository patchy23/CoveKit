import { describe, expect, it } from 'vitest'
import { dateToTimestamp, parseTimestamp, timestampToResult } from './useConverter'

describe('useConverter', () => {
  it('秒级时间戳自动识别（10 位）', () => {
    const r = timestampToResult('1700000000')
    expect(r).not.toBeNull()
    expect(r!.sec).toBe(1700000000)
    expect(r!.ms).toBe(1700000000000)
  })

  it('毫秒级时间戳自动识别（13 位）', () => {
    const r = timestampToResult('1700000000000')
    expect(r!.sec).toBe(1700000000)
    expect(r!.ms).toBe(1700000000000)
  })

  it('非法输入返回 null', () => {
    expect(timestampToResult('abc')).toBeNull()
    expect(timestampToResult('')).toBeNull()
  })

  it('日期字符串 → 时间戳', () => {
    const ts = dateToTimestamp('2023-11-14T22:13:20Z')
    expect(ts).toBe(1700000000000)
  })

  it('parseTimestamp 边界：0 视为秒', () => {
    expect(parseTimestamp('0')).toBe(0)
  })
})
