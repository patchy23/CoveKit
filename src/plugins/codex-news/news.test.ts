import { expect, it } from 'vitest'
import { changedEvents, parseFeeds, revision, safeUrl, validateCache } from './news'
import { feeds, rawEvent } from './testFixtures'

it('按 ID 合并历史与详情并按更新时间排序，限制为 Codex', () => {
  const raw = feeds([rawEvent('old'), rawEvent('new', 'landed', '2026-09-18T12:00:00Z')])
  raw.status.events = [
    {
      ...rawEvent('old'),
      lifecycle: [{ headline: 'Detail', state: 'watch', published_at: '2026-09-17T00:00:00Z' }],
    },
    { ...rawEvent('other'), provider: 'other' },
  ]
  const snapshot = parseFeeds(raw)
  expect(snapshot.events.map((item) => item.id)).toEqual(['new', 'old'])
  expect(snapshot.events[1]!.updates[0]!.headline).toBe('Detail')
})
it('抓取时间变化不算新消息，状态与更正会形成新版本', () => {
  const first = parseFeeds(feeds())
  const fresh = parseFeeds(feeds([{ ...rawEvent('one'), updated_at: '2026-09-18T12:00:00Z' }]))
  expect(changedEvents(first, fresh)).toEqual([])
  const corrected = parseFeeds(feeds([rawEvent('one', 'corrected')]))
  expect(changedEvents(first, corrected)).toHaveLength(1)
  expect(revision(first.events[0]!)).not.toBe(revision(corrected.events[0]!))
})
it('拒绝无效响应与损坏缓存，来源链接不允许执行脚本', () => {
  expect(() => parseFeeds({ status: {}, timeline: {} })).toThrow()
  expect(safeUrl('javascript:alert(1)')).toBe('')
  expect(safeUrl('https://user:secret@example.com')).toBe('')
  expect(() => validateCache({ version: 2 })).toThrow()
  const cache = { version: 1, snapshot: parseFeeds(feeds()), read: {}, auto: true, checkedAt: '' }
  expect(validateCache(cache)).toEqual(cache)
  cache.snapshot.events[0]!.sources = ['file:///etc/passwd']
  expect(() => validateCache(cache)).toThrow()
})
