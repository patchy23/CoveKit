/** 分组路径展开为轻量列表；持久化空分组和旧接口路径共用同一棵树。 */
import type { ApiRecord } from './contracts'
export type ApiSidebarRow =
  | { kind: 'group'; path: string; label: string; depth: number; count: number }
  | { kind: 'api'; api: ApiRecord; depth: number }

export function groupRows(
  paths: string[],
  apis: ApiRecord[],
  collapsed: Set<string>
): ApiSidebarRow[] {
  const nodes = new Map<string, { path: string; label: string; parent: string }>()
  for (const path of [...paths, ...apis.map((api) => api.groupName)]) {
    if (!path) continue
    const parts = path.split('/')
    for (let i = 1; i <= parts.length; i++) {
      const key = parts.slice(0, i).join('/')
      if (key)
        nodes.set(key, {
          path: key,
          label: parts[i - 1] || '/',
          parent: parts.slice(0, i - 1).join('/'),
        })
    }
  }
  const result: ApiSidebarRow[] = []
  function visit(parent: string, depth: number) {
    for (const node of [...nodes.values()]
      .filter((node) => node.parent === parent)
      .sort((a, b) => a.label.localeCompare(b.label, 'zh-CN'))) {
      result.push({
        kind: 'group',
        path: node.path,
        label: node.label,
        depth,
        count: apis.filter(
          (api) => api.groupName === node.path || api.groupName.startsWith(`${node.path}/`)
        ).length,
      })
      if (collapsed.has(node.path)) continue
      visit(node.path, depth + 1)
      result.push(
        ...apis
          .filter((api) => api.groupName === node.path)
          .map((api) => ({ kind: 'api' as const, api, depth: depth + 1 }))
      )
    }
  }
  visit('', 0)
  const ungrouped = apis.filter((api) => !api.groupName)
  if (ungrouped.length || paths.length || apis.length) {
    result.push({ kind: 'group', path: '', label: '未分组', depth: 0, count: ungrouped.length })
    if (!collapsed.has(''))
      result.push(...ungrouped.map((api) => ({ kind: 'api' as const, api, depth: 1 })))
  }
  return result
}
