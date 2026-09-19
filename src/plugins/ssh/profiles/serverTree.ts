/** SSH 分组只有一层，未分组为不可移动的虚拟目录。 */
import type { UiCollectionMove, UiTreeItem } from '@/core/ui'
import type { Payloads, ServerProfile, SshGroup } from '../contracts'
export const UNGROUPED_DROP_KEY = '__ungrouped__'
export function serverTreeItems(
  groups: SshGroup[],
  profiles: ServerProfile[],
  expanded: Set<string>
): UiTreeItem[] {
  const result: UiTreeItem[] = []
  for (const group of [...groups, { id: UNGROUPED_DROP_KEY, name: '未分组', sortOrder: 0 }]) {
    const children = profiles.filter(
      (profile) => (profile.groupId || UNGROUPED_DROP_KEY) === group.id
    )
    result.push({
      id: `group:${group.id}`,
      label: group.name,
      depth: 0,
      kind: 'group',
      expandable: true,
      expanded: expanded.has(group.id),
      badge: children.length,
      draggable: group.id !== UNGROUPED_DROP_KEY,
    })
    if (expanded.has(group.id))
      result.push(
        ...children.map((profile) => ({
          id: `profile:${profile.id}`,
          label: profile.name,
          depth: 1,
          kind: 'profile',
          showIcon: false,
        }))
      )
  }
  return result
}
export function serverTreeDestination(
  move: UiCollectionMove,
  profiles: ServerProfile[]
): Payloads['ssh_tree_move'] | null {
  const group = move.id.startsWith('group:')
  const id = move.id.slice(move.id.indexOf(':') + 1)
  if (id === UNGROUPED_DROP_KEY) return null
  const targetGroup = move.targetId?.startsWith('group:')
  const target = move.targetId?.slice(move.targetId.indexOf(':') + 1)
  let parent: string | null = null,
    anchor: string | null = null
  if (move.targetId !== null) {
    if (group) {
      if (
        !targetGroup ||
        move.position === 'inside' ||
        (target === UNGROUPED_DROP_KEY && move.position === 'after')
      )
        return null
      anchor = target === UNGROUPED_DROP_KEY ? null : (target ?? null)
    } else if (move.position === 'inside') {
      if (!targetGroup) return null
      parent = target === UNGROUPED_DROP_KEY ? null : (target ?? null)
    } else {
      if (targetGroup) return null
      const profile = profiles.find((profile) => profile.id === target)
      if (!profile) return null
      parent = profile.groupId ?? null
      anchor = profile.id
    }
  }
  if (anchor === id) return null
  return { kind: group ? 'group' : 'profile', id, parent, anchor, after: move.position === 'after' }
}
