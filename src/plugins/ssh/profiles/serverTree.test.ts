import { expect, it } from 'vitest'
import { serverTreeItems, serverTreeDestination, UNGROUPED_DROP_KEY } from './serverTree'
import type { ServerProfile } from '../contracts'
const profiles = [
  { id: 'p1', name: 'Z', groupId: 'g1' },
  { id: 'p2', name: 'A', groupId: 'g1' },
  { id: 'p3', name: 'B' },
] as ServerProfile[]
it('服务器采用已保存顺序，折叠只影响可见性，未分组固定末尾', () => {
  const groups = [{ id: 'g1', name: '组', sortOrder: 0 }]
  expect(serverTreeItems(groups, profiles, new Set(['g1'])).map((row) => row.id)).toEqual([
    'group:g1',
    'profile:p1',
    'profile:p2',
    `group:${UNGROUPED_DROP_KEY}`,
  ])
  expect(serverTreeItems(groups, profiles, new Set())).toHaveLength(2)
})
it('分组仅能同级排序，服务器可跨组插入或移回未分组', () => {
  expect(
    serverTreeDestination({ id: 'group:g1', targetId: 'group:g2', position: 'inside' }, profiles)
  ).toBeNull()
  expect(
    serverTreeDestination(
      { id: 'profile:p1', targetId: 'profile:p3', position: 'before' },
      profiles
    )
  ).toEqual({ kind: 'profile', id: 'p1', parent: null, anchor: 'p3', after: false })
  expect(
    serverTreeDestination(
      { id: 'profile:p1', targetId: `group:${UNGROUPED_DROP_KEY}`, position: 'inside' },
      profiles
    )?.parent
  ).toBeNull()
})
