/**
 * SSH 本地文件右键菜单：单选/多选/空白三种场景（设计 v2 矩阵；全部带刷新）
 */
import { computed, ref } from 'vue'
import type { UiContextMenuItem } from '@/core/ui'
import type { RemoteFile } from '../contracts'

interface LocalMenuActions {
  selection: () => RemoteFile[]
  isSelected: (path: string) => boolean
  selectOnly: (file: RemoteFile) => void
  refresh: () => void
  newFile: () => void
  mkdir: () => void
  upload: (files: RemoteFile[]) => void
  rename: (file: RemoteFile) => void
  deleteFile: (file: RemoteFile) => void
  batchDelete: (files: RemoteFile[]) => void
}

/** 管理本地文件列表右键菜单坐标、目标与动态操作项。 */
export function useLocalContextMenu(actions: LocalMenuActions) {
  const menu = ref<{ target: RemoteFile | null; x: number; y: number; multi: boolean } | null>(null)

  function openMenu(event: MouseEvent, target: RemoteFile | null) {
    event.preventDefault()
    let multi = false
    if (target) {
      if (actions.isSelected(target.path) && actions.selection().length >= 2) multi = true
      else actions.selectOnly(target)
    }
    menu.value = {
      target,
      multi,
      x: event.clientX,
      y: event.clientY,
    }
  }

  const menuItems = computed<UiContextMenuItem[]>(() => {
    const state = menu.value
    const refreshItem: UiContextMenuItem = { label: '刷新', onClick: actions.refresh }
    if (!state) return [refreshItem]
    const target = state.target

    if (!target) {
      return [
        { label: '新建文件', onClick: actions.newFile },
        { label: '新建目录', onClick: actions.mkdir },
        refreshItem,
      ]
    }

    if (state.multi) {
      const sel = actions.selection()
      return [
        { label: `批量上传（${sel.length} 项）`, onClick: () => actions.upload(sel) },
        { label: `批量删除（${sel.length} 项）`, onClick: () => actions.batchDelete(sel) },
        refreshItem,
      ]
    }

    return [
      { label: '上传', onClick: () => actions.upload([target]) },
      { label: '重命名', onClick: () => actions.rename(target) },
      { label: '删除', onClick: () => actions.deleteFile(target) },
      { label: '', separator: true },
      { label: '新建文件', onClick: actions.newFile },
      { label: '新建目录', onClick: actions.mkdir },
      refreshItem,
    ]
  })

  return { menu, menuItems, openMenu }
}
