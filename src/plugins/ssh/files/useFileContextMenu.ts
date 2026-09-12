/**
 * SSH 远程文件右键菜单：按 单选文件/单选目录/多选/空白 四种场景组装（设计 v2 矩阵）
 * 右键未选中项时先对齐为单选（业界惯例）；菜单项可见性受 sshPolicy 安全策略约束。
 */
import { computed, ref } from 'vue'
import type { ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import type { RemoteFile } from '../contracts'
import { canEditRemoteFile } from '../connection/useSsh'
import { chmodMenuVisible, deleteMenuVisible } from '../connection/sshPolicy'

interface RemoteMenuActions {
  /** 当前多选列表（顺序与列表一致） */
  selection: () => RemoteFile[]
  /** 右键项是否在多选集合内 */
  isSelected: (path: string) => boolean
  /** 右键未选中项时对齐为单选 */
  selectOnly: (file: RemoteFile) => void
  refresh: () => void
  newFile: (dir: string) => void
  mkdir: (dir: string) => void
  upload: (target: RemoteFile | null) => void
  uploadDirectory: (target: RemoteFile | null) => void
  download: (file: RemoteFile) => void
  batchDownload: (files: RemoteFile[]) => void
  edit: (file: RemoteFile) => void
  rename: (file: RemoteFile) => void
  chmod: (file: RemoteFile) => void
  addBookmark: (dir: RemoteFile) => void
  deleteFile: (file: RemoteFile) => void
  batchDelete: (files: RemoteFile[]) => void
}

/** 管理远程文件列表右键菜单坐标、目标与动态操作项。 */
export function useFileContextMenu(actions: RemoteMenuActions) {
  const menu = ref<{ target: RemoteFile | null; x: number; y: number; multi: boolean } | null>(null)

  function openMenu(event: MouseEvent, target: RemoteFile | null) {
    event.preventDefault()
    // 右键未选中项：对齐为单选（多选集合内右键则保持多选）
    let multi = false
    if (target) {
      if (actions.isSelected(target.path) && actions.selection().length >= 2) multi = true
      else actions.selectOnly(target)
    }
    menu.value = { target, multi, x: event.clientX, y: event.clientY }
    // 坐标夹取在打开后按实际项数二次校正（项数依赖 computed，故直接估算上限）
    menu.value.x = Math.max(8, Math.min(menu.value.x, window.innerWidth - 170 - 8))
    menu.value.y = Math.max(8, Math.min(menu.value.y, window.innerHeight - 10 * 36 - 16))
  }

  const menuItems = computed<ContextMenuItem[]>(() => {
    const state = menu.value
    const refreshItem: ContextMenuItem = { label: '刷新', onClick: actions.refresh }
    if (!state) return [refreshItem]
    const target = state.target

    // 空白处：新建/上传/刷新
    if (!target) {
      return [
        { label: '新建文件', onClick: () => actions.newFile('/') },
        { label: '新建目录', onClick: () => actions.mkdir('/') },
        { label: '上传文件', onClick: () => actions.upload(null) },
        { label: '上传目录', onClick: () => actions.uploadDirectory(null) },
        refreshItem,
      ]
    }

    // 多选：批量下载/批量删除
    if (state.multi) {
      const sel = actions.selection()
      const items: ContextMenuItem[] = [
        { label: `批量下载（${sel.length} 项）`, onClick: () => actions.batchDownload(sel) },
      ]
      const deletable = sel.filter((f) => deleteMenuVisible(f.path))
      if (deletable.length === sel.length) {
        items.push({
          label: `批量删除（${sel.length} 项）`,
          onClick: () => actions.batchDelete(sel),
        })
      }
      items.push(refreshItem)
      return items
    }

    // 单选：行操作 + 同目录新建（列表占满无空白处时仍可新建）
    const items: ContextMenuItem[] = [{ label: '下载', onClick: () => actions.download(target) }]
    if (!target.isDir && canEditRemoteFile(target)) {
      items.push({ label: '编辑', onClick: () => actions.edit(target) })
    }
    items.push({ label: '重命名', onClick: () => actions.rename(target) })
    if (chmodMenuVisible(target.path)) {
      items.push({ label: '修改权限', onClick: () => actions.chmod(target) })
    }
    if (target.isDir) {
      items.push({ label: '添加书签', onClick: () => actions.addBookmark(target) })
    }
    if (deleteMenuVisible(target.path)) {
      items.push({ label: '删除', onClick: () => actions.deleteFile(target) })
    }
    items.push(
      { label: '', separator: true },
      { label: '新建文件', onClick: () => actions.newFile('/') },
      { label: '新建目录', onClick: () => actions.mkdir('/') },
      refreshItem
    )
    return items
  })

  return { menu, menuItems, openMenu }
}
