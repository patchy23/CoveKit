import { expect, it } from 'vitest'
import { apiTreeItems, apiTreeDestination, apiGroupOptions } from './apiTree'
import type { ApiRecord } from './contracts'
const apis = [
  { id: 1, name: '一', groupName: 'a' },
  { id: 2, name: '二', groupName: 'b' },
] as ApiRecord[]
it('树形目标保留层级和根入口，排除源组及后代但不排除相似前缀', () => {
  const choices = apiGroupOptions(
    ['开发', '开发/用户', '开发/用户/子组', '开发/用户2'],
    '开发/用户'
  )
  expect(choices[0]).toEqual({ value: '', label: '根目录' })
  expect(choices[1].children).toEqual([{ value: '开发/用户2', label: '用户2' }])
  expect(apiGroupOptions([])).toEqual([{ value: '', label: '未分组' }])
})
it('目录顺序不被后代的排名或名称覆盖，接口保持后端顺序', () => {
  const rows = apiTreeItems(['a/child', 'b', 'a'], apis, new Set())
  expect(rows.filter((row) => row.depth === 0).map((row) => row.id)).toEqual([
    'group:b',
    'group:a',
    'group:',
  ])
})
it('目录移动后身份与折叠状态保持，移动请求转换为真实路径', () => {
  const before = apiTreeItems(['a'], apis, new Set(['stable']), { a: 'stable' })
  const after = apiTreeItems(['b/a'], [], new Set(['stable']), { 'b/a': 'stable' })
  expect(before.find((row) => row.id === 'group:stable')?.expanded).toBe(false)
  expect(after.find((row) => row.id === 'group:stable')?.expanded).toBe(false)
  expect(
    apiTreeDestination({ id: 'group:stable', targetId: null, position: 'inside' }, apis, {
      'b/a': 'stable',
    })
  ).toEqual({ kind: 'group', id: 'b/a', parent: '', anchor: null, after: false })
})
it('接口支持跨目录插入，目录拒绝移入后代，未分组保持末尾', () => {
  expect(apiTreeDestination({ id: 'api:1', targetId: 'api:2', position: 'before' }, apis)).toEqual({
    kind: 'api',
    id: '1',
    parent: 'b',
    anchor: '2',
    after: false,
  })
  expect(
    apiTreeDestination({ id: 'group:a', targetId: 'group:a/child', position: 'inside' }, apis)
  ).toBeNull()
  expect(
    apiTreeDestination({ id: 'group:a', targetId: 'group:', position: 'after' }, apis)
  ).toBeNull()
})
