/**
 * 导出选择与闭包预览用例（sync L2）
 *
 * 关键口径：只有 selectable 数据集能被勾选；勾中的档案把凭证/分组/隧道/书签按引用带出
 * （同一 id 只算一次）；没勾任何东西时「下一步」不可用；「是否含秘密」只看凭证真的进包。
 */
import { describe, expect, it } from 'vitest'
import type { ExportCatalog, ExportCatalogEntry } from '@/core/ipc/contracts'
import {
  buildSelection,
  carriesSecret,
  chosenNotes,
  closurePreview,
  initialChoice,
  profileEntries,
} from './packSelection'

function entry(id: string, deps: [string, string][], note?: string): ExportCatalogEntry {
  return {
    dataset: 'ssh.profiles',
    id,
    label: `服务器 ${id}`,
    detail: `root@host-${id}:22`,
    dependencies: deps.map(([kind, toId]) => ({ kind, fromId: id, toId })),
    note: note ?? null,
  }
}

function catalog(): ExportCatalog {
  return {
    sourceSpaceId: 'default',
    sourceSpaceName: '默认空间',
    datasets: [
      {
        name: 'ssh.profiles',
        label: '服务器档案',
        owner: 'ssh',
        policy: 'portable',
        schemaVersion: 1,
        selectable: true,
        containsSecret: false,
        defaultSelected: false,
        note: null,
        recordCount: 2,
      },
      {
        name: 'vault.credentials',
        label: '凭证',
        owner: 'vault',
        policy: 'secret',
        schemaVersion: 1,
        selectable: false,
        containsSecret: true,
        defaultSelected: false,
        note: '按所选档案的引用自动带出',
        recordCount: 1,
      },
      {
        name: 'core.favorites',
        label: '收藏',
        owner: 'core',
        policy: 'portable',
        schemaVersion: 1,
        selectable: true,
        containsSecret: false,
        defaultSelected: true,
        note: null,
        recordCount: 3,
      },
    ],
    entries: [
      entry(
        'p1',
        [
          ['credential', 'cred-1'],
          ['group', 'g1'],
          ['tunnel', 't1'],
        ],
        '未绑定凭证'
      ),
      entry('p2', [
        ['credential', 'cred-1'],
        ['bookmark', 'b1'],
      ]),
      // 非 selectable 数据集的条目不该出现在勾选列表里
      {
        dataset: 'vault.credentials',
        id: 'cred-1',
        label: '跳板机',
        detail: '密码',
        dependencies: [],
        note: null,
      },
    ],
    defaults: {
      entries: [],
      datasets: ['core.favorites'],
      includeCredentials: true,
    },
    warnings: [],
  }
}

describe('packSelection', () => {
  it('默认选择取自目录：收藏勾上、最近使用不勾', () => {
    const choice = initialChoice(catalog())
    expect(choice.includeFavorites).toBe(true)
    expect(choice.includeRecentTools).toBe(false)
    expect(choice.includeCredentials).toBe(true)
    expect(choice.profileIds).toEqual([])
  })

  it('勾选列表只含 selectable 数据集的条目', () => {
    const items = profileEntries(catalog())
    expect(items.map((item) => item.id)).toEqual(['p1', 'p2'])
  })

  it('选择集按目录口径翻译：条目 id、类别名、是否带凭证', () => {
    const choice = { ...initialChoice(catalog()), profileIds: ['p1'] }
    expect(buildSelection(catalog(), choice)).toEqual({
      entries: [{ dataset: 'ssh.profiles', ids: ['p1'] }],
      datasets: ['core.favorites'],
      includeCredentials: true,
    })
  })

  it('闭包预览：同一凭证只算一次，各类别分别计数（按类别名排序）', () => {
    const choice = { ...initialChoice(catalog()), profileIds: ['p1', 'p2'] }
    const preview = closurePreview(catalog(), choice)
    expect(preview).toEqual([
      { kind: 'bookmark', count: 1 },
      { kind: 'credential', count: 1 },
      { kind: 'group', count: 1 },
      { kind: 'tunnel', count: 1 },
    ])
  })

  it('是否含秘密：勾凭证但没有任何引用时不算含秘密', () => {
    const noRefs = { ...initialChoice(catalog()), profileIds: [] }
    expect(carriesSecret(catalog(), noRefs)).toBe(false)
    const withRefs = { ...initialChoice(catalog()), profileIds: ['p1'] }
    expect(carriesSecret(catalog(), withRefs)).toBe(true)
    expect(carriesSecret(catalog(), { ...withRefs, includeCredentials: false })).toBe(false)
  })

  it('已选条目的提示只来自被勾中的条目', () => {
    const choice = { ...initialChoice(catalog()), profileIds: ['p1'] }
    expect(chosenNotes(catalog(), choice).map((item) => item.id)).toEqual(['p1'])
    expect(chosenNotes(catalog(), choice)[0].note).toBe('未绑定凭证')
  })
})
