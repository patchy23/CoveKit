/**
 * SSH 文件传输事件状态：统一处理进度、完成/失败反馈、队列展示与取消。
 * transfers 为当前连接的传输任务表（事件驱动）；done 任务保留 10 秒后自动清除。
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc, onTransferProgress } from './ipc'
import type { FileTransferProgress } from './contracts'

/** 队列中的单条传输任务展示 */
export interface TransferItem {
  id: string
  /** 上传 / 下载 */
  kind: 'upload' | 'download'
  /** 展示名（当前文件名） */
  label: string
  transferred: number
  total: number
  error?: string
  done: boolean
}

/** 已完成任务在队列中的保留时长 */
const DONE_TTL_MS = 10_000

/** 订阅文件传输事件，维护任务队列，并在上传完成后触发目录刷新。 */
export function useFileTransfer(
  currentConnectionId: () => string | undefined,
  onUploadDone: () => void
) {
  const ui = useUiStore()
  const transferStatus = ref('')
  /** 当前连接的传输队列（transferId → 任务） */
  const transfers = ref<Map<string, TransferItem>>(new Map())
  const clearTimers = new Map<string, ReturnType<typeof setTimeout>>()
  let unlisten: (() => void) | null = null
  let disposed = false

  /** 任务完成后延迟清理（成功与失败一视同仁，toast 已给出结果） */
  function scheduleClear(id: string) {
    const existing = clearTimers.get(id)
    if (existing) clearTimeout(existing)
    clearTimers.set(
      id,
      setTimeout(() => {
        const next = new Map(transfers.value)
        next.delete(id)
        transfers.value = next
        clearTimers.delete(id)
      }, DONE_TTL_MS)
    )
  }

  /** 取消传输（后端协作式：循环内检查并清理临时文件） */
  async function cancelTransfer(id: string) {
    try {
      await ipc.sshTransferCancel(id)
      ui.toast('已请求取消传输')
    } catch (error) {
      ui.toast(`取消失败：${error}`)
    }
  }

  onMounted(async () => {
    try {
      const stop = await onTransferProgress((progress) => {
        if (progress.connectionId !== currentConnectionId()) return
        applyProgress(progress)
      })
      if (disposed) stop()
      else unlisten = stop
    } catch {
      /* 浏览器预览没有 Tauri 事件系统。 */
    }
  })

  /** 事件 → 队列/状态（独立函数便于单测） */
  function applyProgress(progress: FileTransferProgress) {
    const upload = progress.transferId.startsWith('up')
    const name =
      progress.remotePath.split('/').pop() || progress.localPath.split(/[\\/]/).pop() || '文件'
    if (!progress.done) {
      const next = new Map(transfers.value)
      next.set(progress.transferId, {
        id: progress.transferId,
        kind: upload ? 'upload' : 'download',
        label: name,
        transferred: progress.transferred,
        total: progress.total,
        done: false,
      })
      transfers.value = next
      const percent =
        progress.total > 0
          ? Math.min(100, Math.round((progress.transferred / progress.total) * 100))
          : 0
      transferStatus.value = `${upload ? '上传' : '下载'} ${percent}%`
      return
    }

    transferStatus.value = ''
    scheduleClear(progress.transferId)
    if (progress.error) {
      const next = new Map(transfers.value)
      const item = next.get(progress.transferId)
      if (item) next.set(progress.transferId, { ...item, error: progress.error, done: true })
      transfers.value = next
      ui.toast(`文件传输失败：${progress.error}`)
    } else {
      const next = new Map(transfers.value)
      const item = next.get(progress.transferId)
      if (item) next.set(progress.transferId, { ...item, done: true })
      transfers.value = next
      ui.toast(`${upload ? '上传' : '下载'}完成：${name}`)
      if (upload) onUploadDone()
    }
  }

  onUnmounted(() => {
    disposed = true
    unlisten?.()
    for (const timer of clearTimers.values()) clearTimeout(timer)
  })

  return { transferStatus, transfers, cancelTransfer, applyProgress }
}
