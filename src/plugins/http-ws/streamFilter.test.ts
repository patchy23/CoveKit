import { expect, it, vi } from 'vitest'
import { createStreamFilter } from './streamFilter'
import type { StreamEntry } from './useRequestSession'
const entry = (content: string, seq = 1): StreamEntry => ({
  seq,
  time: 0,
  direction: 'received',
  kind: 'text',
  eventId: '',
  content,
})

it('追加时只扫描新正文；收窄条件跳过已不匹配项，反向搜索仍重新核对', () => {
  const filter = createStreamFilter(),
    first = entry('large Needle'),
    other = entry('OTHER', 2)
  const lower = vi.spyOn(String.prototype, 'toLowerCase')
  try {
    expect(filter([first, other], 'needle', 'all')).toEqual([first])
    lower.mockClear()
    const next = entry('new needle', 3)
    expect(filter([first, other, next], 'needle', 'all')).toEqual([first, next])
    expect(lower).toHaveBeenCalledTimes(2)
    expect(filter([first, other], 'needle extra', 'all')).toEqual([])
    expect(filter([first, other], 'other', 'all')).toEqual([other])
  } finally {
    lower.mockRestore()
  }
})

it('原地内容修改、同序号替换、方向切换与清空搜索保持完整语义', () => {
  const filter = createStreamFilter(),
    first = entry('needle'),
    entries = [first]
  expect(filter(entries, 'needle', 'all')).toEqual(entries)
  first.content = 'changed'
  expect(filter(entries, 'needle', 'all')).toEqual([])
  first.eventId = 'needle'
  expect(filter(entries, 'needle', 'all')).toEqual(entries)
  expect(filter([entry('different')], 'needle', 'all')).toEqual([])
  expect(filter(entries, 'needle', 'sent')).toEqual([])
  expect(filter(entries, '', 'all')).toBe(entries)
})
