/**
 * useDatabase 纯函数单测（filterTreeItems / extractExecSql 等）
 */
import { describe, expect, it } from 'vitest'
import type { UiTreeItem } from '@/core/ui'
import { extractExecSql, filterTreeItems } from './useDatabase'
import { isSystemSchema } from './useDatabaseMeta'

function node(
  id: string,
  label: string,
  depth: number,
  expandable = false,
  expanded = false
): UiTreeItem {
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

describe('extractExecSql', () => {
  const sql = 'SELECT 1;\nSELECT 2;\nSELECT 3;'

  it('有选区时返回选中文本', () => {
    // 选中 "SELECT 2"（第 2 行）
    expect(extractExecSql(sql, 10, 18)).toBe('SELECT 2')
  })

  it('无选区时返回光标所在整行', () => {
    // 光标在第 1 行中间
    expect(extractExecSql(sql, 4, 4)).toBe('SELECT 1;')
    // 光标在第 3 行
    expect(extractExecSql(sql, 20, 20)).toBe('SELECT 3;')
  })

  it('光标在行首与行尾边界正确', () => {
    expect(extractExecSql(sql, 10, 10)).toBe('SELECT 2;')
    expect(extractExecSql(sql, 9, 9)).toBe('SELECT 1;')
    expect(extractExecSql(sql, 0, 0)).toBe('SELECT 1;')
  })

  it('单行文本无换行时返回整行', () => {
    expect(extractExecSql('SELECT 42', 5, 5)).toBe('SELECT 42')
  })
})

describe('isSystemSchema', () => {
  it('mysql 系统库命中（大小写不敏感）', () => {
    for (const name of ['information_schema', 'mysql', 'performance_schema', 'sys', 'MySQL']) {
      expect(isSystemSchema('mysql', name)).toBe(true)
    }
  })

  it('PG 系系统 schema 命中', () => {
    expect(isSystemSchema('postgresql', 'pg_catalog')).toBe(true)
    expect(isSystemSchema('kingbase', 'information_schema')).toBe(true)
  })

  it('业务库不命中；无清单类型一律不命中', () => {
    expect(isSystemSchema('mysql', 'patchybox')).toBe(false)
    expect(isSystemSchema('postgresql', 'public')).toBe(false)
    expect(isSystemSchema('sqlite', 'main')).toBe(false)
    expect(isSystemSchema('redis', 'db0')).toBe(false)
  })
})
