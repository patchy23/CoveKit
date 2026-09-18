/** 消息工具契约；NewsFeeds 对应 Rust codex_news::NewsFeeds，缓存为本模块版本化格式。 */
export interface NewsFeeds {
  status: unknown
  timeline: unknown
}
export interface NewsEvent {
  id: string
  headline: string
  state: string
  kind: string
  outcome: string
  publishedAt: string
  updatedAt: string
  url: string
  sources: string[]
  updates: { headline: string; state: string; at: string }[]
}
export interface NewsSnapshot {
  events: NewsEvent[]
  state: string
  summary: string
  generatedAt: string
  freshUntil: string
  stale: boolean
}
export interface NewsCache {
  version: 1
  snapshot: NewsSnapshot | null
  read: Record<string, string>
  auto: boolean
  checkedAt: string
}
