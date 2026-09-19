/** 接口树的业务落点：目录优先、未分组固定末尾，目录与接口各自排序。 */
import type { UiCollectionMove, UiTreeItem } from '@/core/ui'
import type { ApiRecord, Payloads } from './contracts'
import { groupRows } from './apiGroups'

export function apiTreeItems(
  groups: string[],
  apis: ApiRecord[],
  collapsed: Set<string>,
  identities: Record<string, string> = {}
): UiTreeItem[] {
  const collapsedPaths = new Set(
    [...collapsed].map((key) => Object.entries(identities).find(([, id]) => id === key)?.[0] ?? key)
  )
  return groupRows(groups, apis, collapsedPaths).map((row) =>
    row.kind === 'group'
      ? {
          id: `group:${identities[row.path] ?? row.path}`,
          label: row.label,
          kind: 'group',
          depth: row.depth,
          expandable: true,
          expanded: !collapsedPaths.has(row.path),
          badge: row.count,
          draggable: !!row.path,
        }
      : { id: `api:${row.api.id}`, label: row.api.name, kind: 'api', depth: row.depth }
  )
}
export function apiTreeDestination(
  move: UiCollectionMove,
  apis: ApiRecord[],
  identities: Record<string, string> = {}
): Payloads['api_tree_move'] | null {
  const groupPath = (key: string) =>
    Object.entries(identities).find(([, value]) => value === key)?.[0] ?? key
  const group = move.id.startsWith('group:')
  const rawId = move.id.slice(move.id.indexOf(':') + 1)
  const id = group ? groupPath(rawId) : rawId
  const source = group ? undefined : apis.find((api) => String(api.id) === id)
  if (group ? !id : !source) return null
  const targetGroup = move.targetId?.startsWith('group:')
  const rawTarget = move.targetId?.slice(move.targetId.indexOf(':') + 1)
  const target = targetGroup ? groupPath(rawTarget ?? '') : rawTarget
  let parent = '',
    anchor: string | null = null
  if (move.targetId !== null) {
    if (move.position === 'inside') {
      if (!targetGroup || (group && !target)) return null
      parent = target ?? ''
    } else if (group) {
      if (!targetGroup || (target === '' && move.position === 'after')) return null
      parent = target?.split('/').slice(0, -1).join('/') ?? ''
      anchor = target || null
    } else {
      if (targetGroup) return null
      const api = apis.find((api) => String(api.id) === target)
      if (!api) return null
      parent = api.groupName
      anchor = String(api.id)
    }
  }
  if (group && (parent === id || parent.startsWith(`${id}/`))) return null
  if (anchor === id) return null
  return { kind: group ? 'group' : 'api', id, parent, anchor, after: move.position === 'after' }
}
