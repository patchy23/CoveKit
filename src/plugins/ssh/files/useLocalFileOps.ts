/**
 * 本地文件操作（新建/重命名/删除 + 确认态；从 FileManagerTab 拆出，300 行红线）
 * 两段式：request* 设确认目标 → confirm* 执行；统一 toast 反馈。
 */
import { ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { RemoteFile } from '../contracts'

export function useLocalFileOps(deps: {
  /** 本地当前目录 */
  localDir: () => string
  /** 操作后刷新本地列表并清选择 */
  afterChange: () => void
}) {
  const ui = useUiStore()

  /** 删除确认目标 */
  const deleteTarget = ref<RemoteFile | null>(null)
  /** 重命名目标 */
  const renameTarget = ref<RemoteFile | null>(null)
  /** 新建输入框：'file' | 'dir' | null */
  const createKind = ref<'file' | 'dir' | null>(null)

  /** 拼接本地路径（跟随当前目录分隔符风格） */
  function join(dir: string, name: string): string {
    const sep = dir.includes('\\') ? '\\' : '/'
    return `${dir.replace(/[\\/]$/, '')}${sep}${name}`
  }

  /* ── 新建文件/目录 ── */
  async function confirmCreate(name: string) {
    const kind = createKind.value
    createKind.value = null
    const trimmed = name.trim()
    if (!kind || !trimmed) return
    if (/[<>:"|?*]/.test(trimmed) || trimmed.includes('/') || trimmed.includes('..')) {
      ui.toast('名称含非法字符')
      return
    }
    const path = join(deps.localDir(), trimmed)
    const r = await ipc.sshLocalCreate(path, kind === 'dir')
    if (r.ok) {
      ui.toast(`已创建${kind === 'dir' ? '目录' : '文件'} ${trimmed}`)
      deps.afterChange()
    } else {
      ui.toast(`创建失败：${r.error ?? '未知错误'}`)
    }
  }

  /* ── 重命名 ── */
  function requestRename(file: RemoteFile) {
    renameTarget.value = file
  }

  async function confirmRename(name: string) {
    const target = renameTarget.value
    renameTarget.value = null
    const trimmed = name.trim()
    if (!target || !trimmed || trimmed === target.name) return
    if (/[<>:"|?*]/.test(trimmed) || trimmed.includes('/') || trimmed.includes('..')) {
      ui.toast('名称含非法字符')
      return
    }
    const dir = target.path.replace(/[\\/][^\\/]+$/, '')
    const r = await ipc.sshLocalRename(target.path, join(dir, trimmed))
    if (r.ok) {
      ui.toast('已重命名')
      deps.afterChange()
    } else {
      ui.toast(`重命名失败：${r.error ?? '未知错误'}`)
    }
  }

  /* ── 删除 ── */
  function requestDelete(file: RemoteFile) {
    deleteTarget.value = file
  }

  async function confirmDelete() {
    const target = deleteTarget.value
    deleteTarget.value = null
    if (!target) return
    const r = await ipc.sshLocalDelete(target.path, target.isDir)
    if (r.ok) {
      ui.toast(`已删除 ${target.name}`)
      deps.afterChange()
    } else {
      ui.toast(`删除失败：${r.error ?? '未知错误'}`)
    }
  }

  return {
    deleteTarget,
    renameTarget,
    createKind,
    confirmCreate,
    requestRename,
    confirmRename,
    requestDelete,
    confirmDelete,
  }
}
