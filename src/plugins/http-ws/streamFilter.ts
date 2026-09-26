import type { StreamEntry } from './useRequestSession'

/** 每个历史对象只缓存最近一次匹配结论；追加消息不重复扫描全部旧正文。 */
export function createStreamFilter() {
  let cache = new WeakMap<
    StreamEntry,
    { text: string; kind: string; eventId: string; query: string; matches: boolean }
  >()
  return (entries: StreamEntry[], search: string, direction: string) => {
    const query = search.toLowerCase()
    if (!query) {
      cache = new WeakMap()
      return direction === 'all'
        ? entries
        : entries.filter((entry) => entry.direction === direction)
    }
    return entries.filter((entry) => {
      if (direction !== 'all' && entry.direction !== direction) return false
      const previous = cache.get(entry)
      if (
        previous &&
        previous.text === entry.content &&
        previous.kind === entry.kind &&
        previous.eventId === entry.eventId
      ) {
        if (previous.query === query) return previous.matches
        if (!previous.matches && query.includes(previous.query)) return false
      }
      const matches = `${entry.kind} ${entry.eventId} ${entry.content}`
        .toLowerCase()
        .includes(query)
      cache.set(entry, {
        text: entry.content,
        kind: entry.kind,
        eventId: entry.eventId,
        query,
        matches,
      })
      return matches
    })
  }
}
