/**
 * useFileContextMenu 单测：单选/多选/空白三种场景的菜单项组装
 * 回归点：列表占满无空白处时，单选右键菜单仍提供「新建文件/新建目录」。
 */
import { describe, expect, it } from 'vitest'
import { useFileContextMenu } from './useFileContextMenu'
import type { RemoteFile } from '../contracts'

function noop() {
  /* 测试桩 */
}

function makeActions(selection: RemoteFile[]) {
  const paths = new Set(selection.map((f) => f.path))
  return {
    selection: () => selection,
    isSelected: (path: string) => paths.has(path),
    selectOnly: noop,
    refresh: noop,
    newFile: noop,
    mkdir: noop,
    upload: noop,
    uploadDirectory: noop,
    download: noop,
    batchDownload: noop,
    edit: noop,
    rename: noop,
    chmod: noop,
    addBookmark: noop,
    deleteFile: noop,
    batchDelete: noop,
  }
}

const file = (name: string, isDir = false): RemoteFile => ({
  name,
  path: `/home/${name}`,
  isDir,
  size: isDir ? 0 : 12,
  modifiedAt: 1_700_000_000,
  permissions: isDir ? 'drwxr-xr-x' : '-rw-r--r--',
  owner: 'root',
  group: 'root',
})

function openMenu(menu: ReturnType<typeof useFileContextMenu>, target: RemoteFile | null) {
  menu.openMenu(new MouseEvent('contextmenu', { clientX: 100, clientY: 100 }), target)
  return menu.menuItems.value.map((item) => (item.label ? item.label : '<separator>'))
}

describe('useFileContextMenu 菜单项组装', () => {
  it('单选文件：行操作后排新建文件/新建目录（分隔线隔开）', () => {
    const menu = useFileContextMenu(makeActions([file('a.txt')]))
    const labels = openMenu(menu, file('a.txt'))
    expect(labels).toEqual([
      '下载',
      '编辑',
      '重命名',
      '修改权限',
      '删除',
      '<separator>',
      '新建文件',
      '新建目录',
      '刷新',
    ])
  })

  it('单选目录：含添加书签，同样提供新建入口', () => {
    const menu = useFileContextMenu(makeActions([file('docs', true)]))
    const labels = openMenu(menu, file('docs', true))
    expect(labels).toContain('添加书签')
    expect(labels.indexOf('新建文件')).toBeGreaterThan(labels.indexOf('添加书签'))
    expect(labels).toContain('新建目录')
  })

  it('多选：批量菜单不加新建入口', () => {
    const sel = [file('a.txt'), file('b.txt')]
    const menu = useFileContextMenu(makeActions(sel))
    const labels = openMenu(menu, sel[0])
    expect(labels).toEqual(['批量下载（2 项）', '批量删除（2 项）', '刷新'])
    expect(labels).not.toContain('新建文件')
  })

  it('空白处右键：保留既有新建/上传入口（不回归）', () => {
    const menu = useFileContextMenu(makeActions([]))
    const labels = openMenu(menu, null)
    expect(labels).toEqual(['新建文件', '新建目录', '上传文件', '上传目录', '刷新'])
  })
})
