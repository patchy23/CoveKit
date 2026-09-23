/**
 * SSH 远程文件删除、重命名与权限操作。编辑状态由 useRemoteEditor 管理。
 * 统一 toast 反馈；操作成功后的选中清理与目录刷新由调用方回调注入。
 */
import { ref, type Ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { RemoteFile } from '../contracts'

export function useRemoteFileOps(deps: {
  /** 当前连接 sessionId（无连接返回 undefined） */
  sessionId: () => string | undefined
  /** 当前选中文件（成功后清空） */
  selectedFile: Ref<RemoteFile | null>
  /** 刷新当前目录 */
  refresh: () => void
  onRename?: (oldPath: string, newPath: string) => void
}) {
  const ui = useUiStore()

  /** chmod 目标（单选；弹窗在 ChmodDialog） */
  const chmodTarget = ref<RemoteFile | null>(null)

  /* ── chmod：弹窗确认后调后端（安全策略后端强制） ── */
  function requestChmod(file: RemoteFile) {
    chmodTarget.value = file
  }

  async function confirmChmod(mode: number, recursive: boolean, acknowledgeRisk: boolean) {
    const target = chmodTarget.value
    chmodTarget.value = null
    const connectionId = deps.sessionId()
    if (!target || !connectionId) return
    const r = await ipc.sshFileChmod({
      connectionId,
      remotePath: target.path,
      mode,
      recursive,
      acknowledgeRisk,
    })
    if (r.ok) {
      ui.toast(`已修改权限 ${target.name}`)
      deps.refresh()
    } else {
      ui.toast(`修改权限失败：${r.error ?? '未知错误'}`)
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
        deps.onRename?.(file.path, newPath)
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
    chmodTarget,
    requestChmod,
    confirmChmod,
    deleteTarget,
    requestDelete,
    confirmDelete,
    renameTarget,
    requestRename,
    confirmRename,
  }
}
