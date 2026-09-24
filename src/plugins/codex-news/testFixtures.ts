/** 离线测试源，与第三方字段形状对应，不访问真实服务。 */
export function rawEvent(id = 'one', status = 'announced', at = '2026-09-17T00:00:00Z') {
  return {
    id,
    title: `消息 ${id}`,
    status,
    type: 'direct_reset',
    displayLabel: '额度重置',
    scope: '适用范围未明确',
    createdAt: at,
    updatedAt: at,
    url: `https://aihot.news/codex-resets#${id}`,
    posts: [
      {
        text: '中文来源帖子',
        stage: status,
        publishedAt: at,
        url: 'https://x.com/example/status/1',
      },
    ],
  }
}
export function feeds(events = [rawEvent()]) {
  return {
    resets: {
      schemaVersion: 1,
      checkedAt: '2026-09-18T00:00:00Z' as string | null,
      events,
    },
  }
}
