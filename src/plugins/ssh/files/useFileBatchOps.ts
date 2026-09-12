/**
 * SSH 文件双栏批量操作（批量下载/删除/上传/本地删除）
 * 两段式：request* 打开确认弹窗 → confirm* 执行；单项失败不中断其余（结束后汇总 toast）。
 */
import { ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { RemoteFile } from '../contracts'

/** 待确认的批量操作载荷 */
export interface BatchConfirm {
  /** 操作类型 */
  kind: 'download' | 'deleteRemote' | 'upload' | 'deleteLocal'
  /** 目标项 */
  items: RemoteFile[]
  /** 弹窗标题 */
  title: string
  /** 弹窗正文（含前 5 项预览） */
  message: string
  /** 危险操作标红 */
  danger: boolean
}

export function useFileBatchOps(deps: {
  sessionId: () => string | undefined
  /** 远端当前目录（下载目的地/上传目标在弹窗里另行确认） */
  refreshRemote: () => void
  refreshLocal: () => void
  uploadLocalPaths: (paths: string[], remoteDir?: string) => Promise<void>
  /** 远端当前目录 */
  remoteDir: () => string
  /** 本地当前目录 */
  localDir: () => string
  /** 本地文件是否目录探测（本地列表自带 isDir） */
  localFiles: () => RemoteFile[]
}) {
  const ui = useUiStore()

  /** 待确认弹窗（非空即显示） */
  const batchConfirm = ref<BatchConfirm | null>(null)
  /** 执行中（防重复提交） */
  const batchRunning = ref(false)

  /** 名称预览：前 5 个 + 等 N 项 */
  function previewNames(items: RemoteFile[]): string {
    const names = items.slice(0, 5).map((f) => f.name)
    return names.join('、') + (items.length > 5 ? ` 等 ${items.length} 项` : '')
  }

  /* ── 远程批量下载：确认后直接落本地栏当前目录（不弹目录选择器） ── */
  function requestBatchDownload(items: RemoteFile[]) {
    if (!items.length) return
    batchConfirm.value = {
      kind: 'download',
      items,
      title: `批量下载 ${items.length} 项`,
      message: `将把 ${previewNames(items)} 下载到本地目录 ${deps.localDir()}。`,
      danger: false,
    }
  }

  /* ── 远程批量删除：危险档确认 ── */
  function requestBatchDeleteRemote(items: RemoteFile[]) {
    if (!items.length) return
    batchConfirm.value = {
      kind: 'deleteRemote',
      items,
      title: `批量删除 ${items.length} 项`,
      message: `将删除 ${previewNames(items)}（目录递归删除，不可恢复）。`,
      danger: true,
    }
  }

  /* ── 本地批量上传 ── */
  function requestBatchUpload(items: RemoteFile[]) {
    if (!items.length) return
    batchConfirm.value = {
      kind: 'upload',
      items,
      title: `批量上传 ${items.length} 项`,
      message: `将把 ${previewNames(items)} 上传到远端目录 ${deps.remoteDir()}。`,
      danger: false,
    }
  }

  /** 中列箭头：上传本地选中项到远端当前目录（免确认，单选也走这里） */
  async function uploadSelection(items: RemoteFile[]) {
    if (!items.length) {
      ui.toast('请先在本地列表中选择文件')
      return
    }
    await deps.uploadLocalPaths(
      items.map((f) => f.path),
      deps.remoteDir()
    )
  }

  /* ── 本地批量删除 ── */
  function requestBatchDeleteLocal(items: RemoteFile[]) {
    if (!items.length) return
    batchConfirm.value = {
      kind: 'deleteLocal',
      items,
      title: `批量删除 ${items.length} 项`,
      message: `将从本地删除 ${previewNames(items)}（目录递归删除，不进回收站，不可恢复）。`,
      danger: true,
    }
  }

  /** 确认执行 */
  async function confirmBatch() {
    const job = batchConfirm.value
    if (!job || batchRunning.value) return
    batchRunning.value = true
    batchConfirm.value = null
    try {
      if (job.kind === 'download') await execDownload(job.items)
      else if (job.kind === 'deleteRemote') await execDeleteRemote(job.items)
      else if (job.kind === 'upload') await execUpload(job.items)
      else await execDeleteLocal(job.items)
    } finally {
      batchRunning.value = false
    }
  }

  async function execDownload(items: RemoteFile[]) {
    const connectionId = deps.sessionId()
    if (!connectionId) return
    // 落盘目录：本地栏当前目录（与单选下载一致，不弹选择器）
    const targetDir = deps.localDir()
    if (!targetDir) {
      ui.toast('本地目录尚未就绪，无法下载')
      return
    }
    const sep = targetDir.includes('\\') ? '\\' : '/'
    let failed = 0
    for (const item of items) {
      const localPath = `${targetDir.replace(/[\\/]$/, '')}${sep}${item.name}`
      try {
        // 传输命令入队即返回（结果走传输队列面板），异常才算提交失败
        if (item.isDir) {
          await ipc.sshFileDownloadRecursive({ connectionId, remotePath: item.path, localPath })
        } else {
          await ipc.sshFileDownload({ connectionId, remotePath: item.path, localPath })
        }
      } catch {
        failed++
      }
    }
    ui.toast(
      failed
        ? `批量下载已提交到 ${targetDir}，${failed} 项失败`
        : `已开始批量下载 ${items.length} 项 → ${targetDir}`
    )
    deps.refreshLocal()
  }

  async function execDeleteRemote(items: RemoteFile[]) {
    const connectionId = deps.sessionId()
    if (!connectionId) return
    let failed = 0
    for (const item of items) {
      const r = await ipc.sshFileDelete(connectionId, item.path, item.isDir)
      if (!r.ok) {
        failed++
        ui.toast(`删除失败 ${item.name}：${r.error ?? '未知错误'}`)
      }
    }
    if (!failed) ui.toast(`已删除 ${items.length} 项`)
    deps.refreshRemote()
  }

  async function execUpload(items: RemoteFile[]) {
    await deps.uploadLocalPaths(
      items.map((f) => f.path),
      deps.remoteDir()
    )
  }

  async function execDeleteLocal(items: RemoteFile[]) {
    let failed = 0
    for (const item of items) {
      const r = await ipc.sshLocalDelete(item.path, item.isDir)
      if (!r.ok) {
        failed++
        ui.toast(`删除失败 ${item.name}：${r.error ?? '未知错误'}`)
      }
    }
    if (!failed) ui.toast(`已删除 ${items.length} 项`)
    deps.refreshLocal()
  }

  return {
    batchConfirm,
    batchRunning,
    requestBatchDownload,
    requestBatchDeleteRemote,
    requestBatchUpload,
    requestBatchDeleteLocal,
    uploadSelection,
    confirmBatch,
  }
}
