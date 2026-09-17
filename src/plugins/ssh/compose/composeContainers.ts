import type { ComposeContainer } from '../contracts'

export function parseComposeContainers(raw: string): ComposeContainer[] {
  const text = raw.trim()
  if (!text) return []
  const rows: unknown = text.startsWith('[')
    ? JSON.parse(text)
    : text
        .split('\n')
        .filter(Boolean)
        .map((line) => JSON.parse(line))
  if (!Array.isArray(rows)) throw new Error('容器列表格式无效')
  return rows.map((row) => {
    if (
      !row ||
      typeof row !== 'object' ||
      typeof row.ID !== 'string' ||
      typeof row.Name !== 'string'
    )
      throw new Error('容器列表缺少 ID 或名称')
    const ports = Array.isArray(row.Publishers)
      ? row.Publishers.map(
          (p: { URL?: string; PublishedPort?: number; TargetPort?: number; Protocol?: string }) =>
            p.PublishedPort
              ? `${p.URL || '*'}:${p.PublishedPort} → ${p.TargetPort}/${p.Protocol}`
              : `${p.TargetPort}/${p.Protocol}`
        ).join(', ')
      : ''
    return {
      id: row.ID,
      name: row.Name,
      service: String(row.Service ?? ''),
      image: String(row.Image ?? ''),
      state: String(row.State ?? ''),
      health: String(row.Health ?? ''),
      ports,
    }
  })
}
