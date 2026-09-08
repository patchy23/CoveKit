import { describe, expect, it } from 'vitest'
import { shiftRange } from './useFileSelection'

describe('shiftRange（Shift 范围选）', () => {
  const paths = ['a', 'b', 'c', 'd', 'e']

  it('锚点在前向后选', () => {
    expect(shiftRange(paths, 'b', 'd')).toEqual(['b', 'c', 'd'])
  })

  it('锚点在后向前选', () => {
    expect(shiftRange(paths, 'e', 'b')).toEqual(['b', 'c', 'd', 'e'])
  })

  it('同点', () => {
    expect(shiftRange(paths, 'c', 'c')).toEqual(['c'])
  })

  it('锚点不在列表（目录已切换）退化为单选目标', () => {
    expect(shiftRange(paths, 'gone', 'd')).toEqual(['d'])
  })
})
