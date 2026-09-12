/**
 * 远程新建文件/目录（从 FileManagerTab 拆出，300 行红线）
 * InputDialog 目标态 + 确认执行；统一 toast 反馈。
 */
import { ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'

export function useRemoteCreateOps(deps: {
  sessionId: () => string | undefined
  /** 新建发生的目录（当前目录） */
  currentDir: () => string
  refresh: () => void
}) {
  const ui = useUiStore()

  /** 新建文件目标目录（非空即弹窗） */
  const newFileTarget = ref<string | null>(null)
  /** 新建目录目标目录（非空即弹窗） */
  const mkdirTarget = ref<string | null>(null)

  /** 名称基础校验（不含分隔符/父级引用） */
  function validName(name: string): boolean {
    return !!name && !name.includes('/') && !name.includes('..')
  }

  async function confirmNewFile(name: string) {
    const dir = newFileTarget.value
    newFileTarget.value = null
    const connectionId = deps.sessionId()
    const trimmed = name.trim()
    if (!dir || !connectionId || !trimmed) return
    if (!validName(trimmed)) {
      ui.toast('文件名不能包含路径分隔符')
      return
    }
    const path = dir.endsWith('/') ? `${dir}${trimmed}` : `${dir}/${trimmed}`
    const r = await ipc.sshFileCreate(connectionId, path)
    if (r.ok) {
      ui.toast(`已创建文件 ${trimmed}`)
      deps.refresh()
    } else {
      ui.toast(`创建文件失败：${r.error ?? '未知错误'}`)
    }
  }

  async function confirmMkdir(name: string) {
    const dir = mkdirTarget.value
    mkdirTarget.value = null
    const connectionId = deps.sessionId()
    const trimmed = name.trim()
    if (!dir || !connectionId || !trimmed) return
    if (!validName(trimmed)) {
      ui.toast('目录名不能包含路径分隔符')
      return
    }
    const path = dir.endsWith('/') ? `${dir}${trimmed}` : `${dir}/${trimmed}`
    const r = await ipc.sshFileMkdir(connectionId, path)
    if (r.ok) {
      ui.toast(`已创建目录 ${trimmed}`)
      deps.refresh()
    } else {
      ui.toast(`创建目录失败：${r.error ?? '未知错误'}`)
    }
  }

  function requestNewFile() {
    newFileTarget.value = deps.currentDir()
  }

  function requestMkdir() {
    mkdirTarget.value = deps.currentDir()
  }

  return { newFileTarget, mkdirTarget, confirmNewFile, confirmMkdir, requestNewFile, requestMkdir }
}
