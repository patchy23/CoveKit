/** 离线测试源，与第三方字段形状对应，不访问真实服务。 */
export function rawEvent(id = 'one', state = 'watch', at = '2026-09-17T00:00:00Z') {
  return {
    event_id: id,
    headline: `Message ${id}`,
    provider: 'codex',
    state,
    kind: 'global_auto_reset',
    outcome: null,
    first_published_at: at,
    updated_at: at,
    event_url: `https://savemetibo.com/events/${id}/`,
    receipt_urls: ['https://x.com/example/status/1'],
    lifecycle: [] as { headline: string; state: string; published_at: string }[],
  }
}
export function feeds(events = [rawEvent()]) {
  return {
    status: {
      generated_at: '2026-09-18T00:00:00Z',
      providers: { codex: { state: 'quiet', what_changed: 'Nothing new' } },
      freshness: { fresh_until: '2026-09-18T00:30:00Z', stale: false },
      events,
    },
    timeline: { providers: { codex: { published: events } } },
  }
}
