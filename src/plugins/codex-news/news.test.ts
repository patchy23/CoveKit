import { expect, it } from 'vitest'
import { changedEvents, parseFeeds, revision, safeUrl, statusLabel, validateCache } from './news'
import { feeds, rawEvent } from './testFixtures'

it('读取完整重置快照并按更新时间排序，保留中文来源帖子', () => {
  const raw = feeds([rawEvent('old'), rawEvent('new', 'confirmed', '2026-09-18T12:00:00Z')])
  const snapshot = parseFeeds(raw)
  expect(snapshot.events.map((item) => item.id)).toEqual(['new', 'old'])
  expect(snapshot.events[1]!.updates[0]!.headline).toBe('中文来源帖子')
})
it('抓取时间变化不算新消息，状态与更正会形成新版本', () => {
  const first = parseFeeds(feeds())
  const fresh = parseFeeds(feeds([{ ...rawEvent('one'), updatedAt: '2026-09-18T12:00:00Z' }]))
  expect(changedEvents(first, fresh)).toEqual([])
  const corrected = parseFeeds(feeds([rawEvent('one', 'confirmed')]))
  expect(changedEvents(first, corrected)).toHaveLength(1)
  expect(revision(first.events[0]!)).not.toBe(revision(corrected.events[0]!))
})
it('拒绝无效响应与损坏缓存，来源链接不允许执行脚本', () => {
  expect(() => parseFeeds({ resets: {} })).toThrow()
  expect(safeUrl('javascript:alert(1)')).toBe('')
  expect(safeUrl('https://user:secret@example.com')).toBe('')
  expect(() => validateCache({ version: 2 })).toThrow()
  const cache = { version: 1, snapshot: parseFeeds(feeds()), read: {}, auto: true, checkedAt: '' }
  expect(validateCache(cache)).toEqual(cache)
  cache.snapshot.events[0]!.sources = ['file:///etc/passwd']
  expect(() => validateCache(cache)).toThrow()
})

it('展示限定状态和适用范围，不把过期预告当作已完成', () => {
  const item = {
    ...rawEvent(),
    label: '全员重置',
    displayLabel: '重置形式未明确',
    presentation: { status: 'expired_unconfirmed', scopeLabel: 'Pro 用户', timeInferred: true },
    schedule: { label: '明天' },
  }
  const snapshot = parseFeeds({ resets: { ...feeds().resets, events: [item] } })
  expect(snapshot.events[0]!.kind).toBe('重置形式未明确')
  expect(snapshot.events[0]!.outcome).toContain('Pro 用户')
  expect(statusLabel(snapshot.events[0]!.state).label).toBe('预告已过期，未确认')
})

it('核验时间未知或监控延迟时标记旧数据，未知时间可持久化', () => {
  const raw = feeds([])
  raw.resets.checkedAt = null
  const snapshot = parseFeeds(raw)
  expect(snapshot.stale).toBe(true)
  expect(snapshot.generatedAt).toBe('')
  expect(
    validateCache({ version: 1, snapshot, read: {}, auto: true, checkedAt: '' }).snapshot
  ).toEqual(snapshot)
  expect(parseFeeds({ resets: { ...feeds().resets, monitor: { status: 'delayed' } } }).stale).toBe(
    true
  )
  expect(() => parseFeeds(feeds([rawEvent(), rawEvent()]))).toThrow('重复')
})
