import { describe, expect, it } from 'vitest'
import { groupRows } from './apiGroups'
import type { ApiRecord } from './contracts'
const record = (id: number, groupName: string) =>
  ({
    id,
    groupName,
    name: `接口${id}`,
    method: 'GET',
    type: 'http',
    url: '',
    params: '[]',
    headers: '[]',
    bodyMode: 'none',
    body: '',
    options: '{}',
    updatedAt: '',
  }) as ApiRecord
describe('接口分组树', () => {
  it('保留空分组，补齐旧路径上级，并把接口放在所属层级', () => {
    const rows = groupRows(
      ['空分组', '开发/用户/查询'],
      [record(1, '开发/用户/查询'), record(2, '')],
      new Set()
    )
    expect(rows).toContainEqual({ kind: 'group', path: '开发', label: '开发', depth: 0, count: 1 })
    expect(rows).toContainEqual({
      kind: 'group',
      path: '空分组',
      label: '空分组',
      depth: 0,
      count: 0,
    })
    expect(rows).toContainEqual({ kind: 'api', api: record(1, '开发/用户/查询'), depth: 3 })
    expect(rows.at(-1)).toEqual({ kind: 'api', api: record(2, ''), depth: 1 })
  })
  it('折叠隐藏整条子树，但不误吞名称前缀相似的兄弟分组', () => {
    const rows = groupRows(
      ['开发/用户', '开发环境'],
      [record(1, '开发/用户'), record(2, '开发环境')],
      new Set(['开发'])
    )
    expect(rows.filter((row) => row.kind === 'api').map((row) => row.api.id)).toEqual([2])
    expect(rows.some((row) => row.kind === 'group' && row.path === '开发/用户')).toBe(false)
  })
})
