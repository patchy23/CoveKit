/** 第三方源归一化、状态文案与事件版本；不以抓取时间制造新消息。 */
import type { UiTone } from '@/core/ui/types'
import type { NewsCache, NewsEvent, NewsFeeds, NewsSnapshot } from './contracts'

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('消息数据结构无效')
  return value as Record<string, unknown>
}
function text(value: unknown): string {
  return typeof value === 'string' ? value : ''
}
function date(value: unknown): string {
  const s = text(value)
  if (!s || !Number.isFinite(Date.parse(s))) throw new Error('消息时间格式无效')
  return s
}
export function safeUrl(value: unknown): string {
  try {
    const url = new URL(text(value))
    return url.protocol === 'https:' && !url.username && !url.password ? url.href : ''
  } catch {
    return ''
  }
}
function event(value: unknown): NewsEvent {
  const row = object(value)
  if (!text(row.event_id) || !text(row.headline) || !text(row.state))
    throw new Error('消息缺少必要字段')
  return {
    id: text(row.event_id),
    headline: text(row.headline),
    state: text(row.state),
    kind: text(row.kind),
    outcome: text(row.outcome),
    publishedAt: date(row.first_published_at),
    updatedAt: date(row.updated_at),
    url: safeUrl(row.event_url),
    sources: Array.isArray(row.receipt_urls)
      ? [...new Set(row.receipt_urls.map(safeUrl).filter(Boolean))]
      : [],
    updates: Array.isArray(row.lifecycle)
      ? row.lifecycle.slice(-20).map((item) => {
          const update = object(item)
          return {
            headline: text(update.headline),
            state: text(update.state),
            at: date(update.published_at),
          }
        })
      : [],
  }
}

export function parseFeeds(feeds: NewsFeeds): NewsSnapshot {
  const status = object(feeds.status),
    timeline = object(feeds.timeline)
  const current = object(object(status.providers).codex)
  const history = object(object(timeline.providers).codex)
  const freshness = object(status.freshness)
  if (!Array.isArray(history.published) || !Array.isArray(status.events))
    throw new Error('消息列表格式无效')
  const events = new Map<string, NewsEvent>()
  for (const raw of [
    ...history.published,
    ...status.events.filter((item) => object(item).provider === 'codex'),
  ]) {
    const next = event(raw),
      previous = events.get(next.id)
    if (!previous || Date.parse(next.updatedAt) >= Date.parse(previous.updatedAt))
      events.set(next.id, next)
  }
  return {
    events: [...events.values()]
      .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
      .slice(0, 200),
    state: text(current.state),
    summary: text(current.what_changed),
    generatedAt: date(status.generated_at),
    freshUntil: date(freshness.fresh_until),
    stale: freshness.stale === true || freshness.outage === true || current.stale === true,
  }
}

/** 忽略 updatedAt 等抓取元数据；原文、状态、证据或后续记录改变才视为更新。 */
export function revision(item: NewsEvent): string {
  return JSON.stringify([
    item.headline,
    item.state,
    item.kind,
    item.outcome,
    item.sources,
    item.updates,
  ])
}
export function changedEvents(previous: NewsSnapshot, next: NewsSnapshot): NewsEvent[] {
  const versions = new Map(previous.events.map((item) => [item.id, revision(item)]))
  return next.events.filter((item) => versions.get(item.id) !== revision(item))
}
export function statusLabel(state: string): { label: string; tone: UiTone } {
  switch (state) {
    case 'landed':
      return { label: '已生效', tone: 'success' }
    case 'confirmed':
      return { label: '已确认', tone: 'success' }
    case 'watch':
      return { label: '观察中', tone: 'warning' }
    case 'quiet':
      return { label: '暂无新动向', tone: 'neutral' }
    case 'retracted':
      return { label: '已撤回', tone: 'danger' }
    case 'corrected':
      return { label: '已更正', tone: 'warning' }
    default:
      return { label: state || '待确认', tone: 'neutral' }
  }
}
export function localTime(value: string): string {
  return value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '尚未检查'
}

/** 缓存恢复也校验源结构；损坏时交由调用方显示错误，不覆盖原缓存。 */
export function validateCache(value: unknown): NewsCache {
  const cache = object(value)
  if (cache.version !== 1 || typeof cache.auto !== 'boolean')
    throw new Error('消息缓存版本或设置无效')
  const read = object(cache.read)
  if (Object.values(read).some((v) => typeof v !== 'string')) throw new Error('消息已读记录无效')
  if (cache.checkedAt !== '') date(cache.checkedAt)
  if (cache.snapshot !== null) {
    const snapshot = object(cache.snapshot)
    date(snapshot.generatedAt)
    date(snapshot.freshUntil)
    if (
      !Array.isArray(snapshot.events) ||
      typeof snapshot.state !== 'string' ||
      typeof snapshot.summary !== 'string' ||
      typeof snapshot.stale !== 'boolean'
    )
      throw new Error('消息缓存内容无效')
    for (const raw of snapshot.events) {
      const row = object(raw)
      if (
        ['id', 'headline', 'state', 'kind', 'outcome', 'url'].some(
          (key) => typeof row[key] !== 'string'
        ) ||
        !Array.isArray(row.sources) ||
        !Array.isArray(row.updates)
      )
        throw new Error('消息缓存条目无效')
      date(row.publishedAt)
      date(row.updatedAt)
      if (row.sources.some((url) => !safeUrl(url)) || (row.url && !safeUrl(row.url)))
        throw new Error('消息来源链接无效')
      for (const update of row.updates) {
        const item = object(update)
        if (typeof item.headline !== 'string' || typeof item.state !== 'string')
          throw new Error('消息更新记录无效')
        date(item.at)
      }
    }
  }
  return cache as unknown as NewsCache
}
