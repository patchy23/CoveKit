/** 连接内传输快照；保留最近 200 个已结束任务，活动任务不裁剪。 */
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { ipc, onTransferProgress } from '../ipc'
import type { FileTransferProgress } from '../contracts'
export interface TransferItem {
  id: string
  kind: 'upload' | 'download'
  label: string
  transferred: number
  total: number
  error?: string
  done: boolean
  localPath: string
  remotePath: string
  speed: number
  updatedAt: number
  cancelling?: boolean
  preparing?: boolean
}
export function useFileTransfer(
  currentConnectionId: () => string | undefined,
  onUploadDone: () => void
) {
  const transferStatus = ref(''),
    transfers = ref(new Map<string, TransferItem>())
  let unlisten: (() => void) | undefined,
    disposed = false
  function trimHistory() {
    const ended = [...transfers.value.values()].filter((v) => v.done)
    for (const stale of ended.slice(0, Math.max(0, ended.length - 200)))
      transfers.value.delete(stale.id)
  }
  function clearEnded() {
    transfers.value = new Map([...transfers.value].filter(([, v]) => !v.done))
  }
  function applyProgress(p: FileTransferProgress) {
    if (p.connectionId !== currentConnectionId() || disposed) return
    const old = transfers.value.get(p.transferId),
      now = Date.now()
    if (old?.done || (old && !p.done && p.transferred < old.transferred)) return
    const item: TransferItem = {
      id: p.transferId,
      kind: p.transferId.startsWith('up') ? 'upload' : 'download',
      label:
        p.remotePath.split('/').pop() ||
        p.localPath.replaceAll(String.fromCharCode(92), '/').split('/').pop() ||
        '文件',
      localPath: p.localPath,
      remotePath: p.remotePath,
      transferred: p.transferred,
      total: p.total,
      done: p.done,
      error: p.error,
      updatedAt: now,
      speed:
        old && now > old.updatedAt
          ? (Math.max(0, p.transferred - old.transferred) * 1000) / (now - old.updatedAt)
          : 0,
      cancelling: old?.cancelling,
    }
    const next = new Map(transfers.value)
    next.set(item.id, item)
    const ended = [...next.values()].filter((v) => v.done)
    for (const stale of ended.slice(0, Math.max(0, ended.length - 200))) next.delete(stale.id)
    transfers.value = next
    transferStatus.value = [...next.values()].some((v) => !v.done) ? '文件传输中' : ''
    if (p.done && !p.error && item.kind === 'upload') onUploadDone()
  }
  async function startTransfer(
    kind: 'upload' | 'download',
    localPath: string,
    remotePath: string,
    overwrite = false
  ) {
    const connectionId = currentConnectionId()
    if (!connectionId) throw new Error('SSH 未连接')
    const id = 'preparing-' + crypto.randomUUID()
    const entry: TransferItem = {
      id,
      kind,
      label: remotePath.split('/').pop() || '文件',
      localPath,
      remotePath,
      transferred: 0,
      total: 0,
      done: false,
      speed: 0,
      updatedAt: Date.now(),
      preparing: true,
    }
    transfers.value.set(id, entry)
    transferStatus.value = '正在准备传输'
    try {
      const result =
        kind === 'upload'
          ? await ipc.sshFileUpload({ connectionId, localPath, remotePath })
          : await ipc.sshFileDownloadRecursive({ connectionId, localPath, remotePath, overwrite })
      const cancelled = transfers.value.get(id)?.cancelling
      transfers.value.delete(id)
      applyProgress(result)
      if (cancelled) await cancelTransfer(result.transferId)
      return result
    } catch (e) {
      const item = transfers.value.get(id)
      if (item) {
        item.done = true
        item.preparing = false
        item.error = String(e)
        trimHistory()
        transferStatus.value = [...transfers.value.values()].some((v) => !v.done)
          ? '文件传输中'
          : ''
      }
      throw e
    }
  }
  async function cancelTransfer(id: string) {
    const item = transfers.value.get(id)
    if (!item || item.done) return
    item.cancelling = true
    if (item.preparing) return
    try {
      await ipc.sshTransferCancel(id)
    } catch (e) {
      const latest = transfers.value.get(id)
      if (latest) {
        latest.cancelling = false
        latest.error = '取消失败：' + String(e)
      }
    }
  }
  watch(currentConnectionId, () => {
    for (const item of transfers.value.values())
      if (!item.done) {
        item.done = true
        item.preparing = false
        item.error = '连接已变化，传输状态中断'
      }
    trimHistory()
    transferStatus.value = ''
  })
  onMounted(async () => {
    try {
      const stop = await onTransferProgress((p) => {
        if (p.connectionId === currentConnectionId()) applyProgress(p)
      })
      if (disposed) stop()
      else unlisten = stop
    } catch (e) {
      transferStatus.value = '无法订阅传输进度：' + String(e)
    }
  })
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
  return { transferStatus, transfers, cancelTransfer, applyProgress, clearEnded, startTransfer }
}
