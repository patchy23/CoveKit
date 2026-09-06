/**
 * SSH 远程文件的编辑/删除/重命名操作（从 FileManagerTab 拆出，300 行红线）
 * 统一 toast 反馈；操作成功后的选中清理与目录刷新由调用方回调注入。
 */
import { ref, type Ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from './ipc'
import { canEditRemoteFile } from './useSsh'
import type { RemoteFile } from './contracts'

/** 远程编辑目标（打开的文件内容） */
export interface RemoteEditing {
  connectionId: string
  path: string
  content: string
  /** 打开时的远端 mtime（毫秒）；保存时做乐观锁比对 */
  modifiedAt?: number
  /** 保存冲突后置位：提示远端已变化，用户可选择强制覆盖 */
  conflict?: boolean
}

export function useRemoteFileOps(deps: {
  /** 当前连接 sessionId（无连接返回 undefined） */
  sessionId: () => string | undefined
  /** 当前选中文件（成功后清空） */
  selectedFile: Ref<RemoteFile | null>
  /** 刷新当前目录 */
  refresh: () => void
}) {
  const ui = useUiStore()

  /* ── 远程编辑 ── */
  const editing = ref<RemoteEditing | null>(null)
  const savingEdit = ref(false)

  /** 打开文件进编辑器（类型/大小不支持时 toast 拦截） */
  async function openFile(file: RemoteFile) {
    const connectionId = deps.sessionId()
    if (!connectionId) return
    try {
      const r = await ipc.sshEditOpen(connectionId, file.path)
      if (deps.sessionId() !== connectionId) return
      if (r.ok) {
        editing.value = {
          connectionId,
          path: r.path,
          content: r.content,
          modifiedAt: r.modifiedAt,
        }
      } else {
        ui.toast(`打开文件失败：${r.error ?? '未知错误'}`)
      }
    } catch (e) {
      ui.toast(`打开文件失败：${e}`)
    }
  }

  /** 双击打开：目录导航由调用方处理，这里只管文件编辑入口 */
  async function openFileGuarded(file: RemoteFile): Promise<'dir' | 'opened' | 'rejected'> {
    if (file.isDir) return 'dir'
    if (!canEditRemoteFile(file)) {
      ui.toast('该文件类型或大小不支持在线编辑')
      return 'rejected'
    }
    await openFile(file)
    return 'opened'
  }

  /**
   * 保存：回写服务器（ssh_edit_save）。
   * force=false 时携带打开时的 mtime 做乐观锁；远端已变化则标记 conflict 并保留编辑器
   * 由用户决定「强制覆盖」或「放弃」；force=true 跳过校验直接写入。
   */
  async function onSave(content: string, force = false) {
    const target = editing.value
    if (!target || savingEdit.value) return
    savingEdit.value = true
    try {
      const expectedMtime = force ? undefined : target.modifiedAt
      const r = await ipc.sshEditSave(target.connectionId, target.path, content, expectedMtime)
      if (r.ok) {
        ui.toast(`已保存 ${target.path}（${content.length} 字符）`)
        editing.value = null
      } else if (r.conflict) {
        // 回填远端当前 mtime 为新基线：再次保存走正常乐观锁（而非永远冲突）
        editing.value = { ...target, content, conflict: true, modifiedAt: r.currentMtime ?? target.modifiedAt }
        ui.toast('远端文件已被修改，保存被阻止')
      } else {
        ui.toast(`保存失败：${r.error ?? '未知错误'}`)
      }
    } catch (e) {
      ui.toast(`保存失败：${e}`)
    } finally {
      savingEdit.value = false
    }
  }

  /* ── 删除（两段式确认） ── */
  const deleteTarget = ref<RemoteFile | null>(null)

  function requestDelete(file: RemoteFile | null = deps.selectedFile.value) {
    if (!file) {
      ui.toast('请先选择文件')
      return
    }
    deleteTarget.value = file
  }

  async function confirmDelete() {
    const file = deleteTarget.value
    deleteTarget.value = null
    const sessionId = deps.sessionId()
    if (!file || !sessionId) return
    try {
      const r = await ipc.sshFileDelete(sessionId, file.path, file.isDir)
      if (r.ok) {
        ui.toast(`已删除 ${file.name}`)
        deps.selectedFile.value = null
        deps.refresh()
      } else {
        ui.toast(`删除失败：${r.error ?? '未知错误'}`)
      }
    } catch (e) {
      ui.toast(`删除失败：${e}`)
    }
  }

  /* ── 重命名（弹窗输入新名） ── */
  const renameTarget = ref<RemoteFile | null>(null)

  function requestRename(file: RemoteFile | null = deps.selectedFile.value) {
    if (!file) {
      ui.toast('请先选择文件')
      return
    }
    renameTarget.value = file
  }

  async function confirmRename(name: string) {
    const file = renameTarget.value
    renameTarget.value = null
    const sessionId = deps.sessionId()
    if (!file || !sessionId || name === file.name) return
    const dir = file.path.slice(0, file.path.lastIndexOf('/') + 1)
    const newPath = `${dir}${name}`
    try {
      const r = await ipc.sshFileRename(sessionId, file.path, newPath)
      if (r.ok) {
        ui.toast(`已重命名为 ${name}`)
        deps.selectedFile.value = null
        deps.refresh()
      } else {
        ui.toast(`重命名失败：${r.error ?? '未知错误'}`)
      }
    } catch (e) {
      ui.toast(`重命名失败：${e}`)
    }
  }

  return {
    editing,
    savingEdit,
    openFile,
    openFileGuarded,
    onSave,
    deleteTarget,
    requestDelete,
    confirmDelete,
    renameTarget,
    requestRename,
    confirmRename,
  }
}
