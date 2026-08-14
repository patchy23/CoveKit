/**
 * useDatabase 纯函数单测（filterTreeItems 等）
 */
import { describe, expect, it } from 'vitest'
import type { UiTreeItem } from '@/core/ui'
import { filterTreeItems } from './useDatabase'

function node(id: string, label: string, depth: number, expandable = false, expanded = false): UiTreeItem {
  return { id, label, depth, kind: 'table', expandable, expanded }
}

describe('filterTreeItems', () => {
  const tree: UiTreeItem[] = [
    node('conn1', '生产 · 订单库', 0, true, true),
    node('conn1::db', 'patchybox', 1, true, true),
    node('conn1::tables', '表', 2, true, true),
    node('conn1::table:users', 'users', 3),
    node('conn1::table:orders', 'orders', 3),
    node('conn2', '本地 SQLite', 0, true, false),
    node('conn2::db', 'main', 1, true, false),
  ]

  it('无关键字时按展开状态裁剪', () => {
    const visible = filterTreeItems(tree, '')
    expect(visible.map((n) => n.id)).toEqual([
      'conn1',
      'conn1::db',
      'conn1::tables',
      'conn1::table:users',
      'conn1::table:orders',
      'conn2',
    ])
  })

  it('关键字命中时保留祖先链', () => {
    const visible = filterTreeItems(tree, 'orders')
    expect(visible.map((n) => n.id)).toEqual([
      'conn1',
      'conn1::db',
      'conn1::tables',
      'conn1::table:orders',
    ])
  })

  it('关键字大小写不敏感', () => {
    const visible = filterTreeItems(tree, 'USERS')
    expect(visible.some((n) => n.id === 'conn1::table:users')).toBe(true)
  })

  it('无命中返回空数组', () => {
    expect(filterTreeItems(tree, '不存在')).toEqual([])
  })
})
