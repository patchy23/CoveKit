/** 连接内传输快照；保留最近 200 个已结束任务，活动任务不裁剪。 */
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { ipc, onTransferProgress } from '../ipc'
import type { FileTransferProgress } from '../contracts'
export interface TransferItem {
  id: string
  kind: 'upload' | 'download' | 'compress' | 'extract' | 'preview'
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
  queued?: boolean
  preparing?: boolean
}
export function useFileTransfer(
  currentConnectionId: () => string | undefined,
  onUploadDone: () => void,
  onDownloadDone?: (path: string) => void
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
        old?.label ||
        p.remotePath.split('/').pop() ||
        p.localPath.replaceAll(String.fromCharCode(92), '/').split('/').pop() ||
        '文件',
      localPath: old?.localPath ?? p.localPath,
      remotePath: old?.remotePath ?? p.remotePath,
      transferred: p.transferred,
      total: p.total,
      done: p.done,
      error: p.error,
      updatedAt: now,
      speed:
        old && now > old.updatedAt
          ? (Math.max(0, p.transferred - old.transferred) * 1000) / (now - old.updatedAt)
          : 0,
      cancelling: p.done ? p.error === '已取消' : old?.cancelling,
    }
    const next = new Map(transfers.value)
    next.set(item.id, item)
    const ended = [...next.values()].filter((v) => v.done)
    for (const stale of ended.slice(0, Math.max(0, ended.length - 200))) next.delete(stale.id)
    transfers.value = next
    transferStatus.value = [...next.values()].some((v) => !v.done) ? '文件传输中' : ''
    if (p.done && !p.error && item.kind === 'upload') onUploadDone()
    if (p.done && !p.error && item.kind === 'download') onDownloadDone?.(item.localPath)
  }
  async function launchTransfer(
    kind: 'upload' | 'download',
    localPath: string,
    remotePath: string,
    overwrite: boolean,
    id: string
  ) {
    const connectionId = currentConnectionId()
    if (!connectionId) throw new Error('SSH 未连接')
    if (transfers.value.get(id)?.done) return
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
      if (disposed || connectionId !== currentConnectionId()) {
        await ipc.sshTransferCancel(result.transferId)
        return
      }
      const cancelled = transfers.value.get(id)?.cancelling
      transfers.value.delete(id)
      const early = transfers.value.get(result.transferId)
      if (early) Object.assign(early, { label: entry.label, localPath, remotePath })
      else
        transfers.value.set(result.transferId, {
          ...entry,
          id: result.transferId,
          preparing: false,
        })
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

  // 所有任务先入队，最多两项执行；相同或相互包含的目标串行。
  const queue: {
    id: string
    kind: 'upload' | 'download'
    localPath: string
    remotePath: string
    overwrite: boolean
    session: string
  }[] = []
  const active = new Map<string, { kind: string; path: string }>()
  let stopping = false
  function targetPath(path: string) {
    const normalized = path.replaceAll(String.fromCharCode(92), '/').replace(/\/$/, '')
    return /^[A-Za-z]:/.test(normalized) || normalized.startsWith('//')
      ? normalized.toLowerCase()
      : normalized
  }
  function pump() {
    if (stopping || disposed) return
    while (active.size < 2) {
      const index = queue.findIndex(
        (job) =>
          ![...active.values()].some((v) => {
            const path = targetPath(job.kind === 'download' ? job.localPath : job.remotePath)
            return (
              v.kind === job.kind &&
              (v.path === path || v.path.startsWith(path + '/') || path.startsWith(v.path + '/'))
            )
          })
      )
      if (index < 0) return
      const job = queue.splice(index, 1)[0]
      if (transfers.value.get(job.id)?.done || job.session !== currentConnectionId()) continue
      active.set(job.id, {
        kind: job.kind,
        path: targetPath(job.kind === 'download' ? job.localPath : job.remotePath),
      })
      void (async () => {
        try {
          const result = await launchTransfer(
            job.kind,
            job.localPath,
            job.remotePath,
            job.overwrite,
            job.id
          )
          if (result && !transfers.value.get(result.transferId)?.done) {
            await new Promise<void>((resolve) => {
              const stop = watch(
                () =>
                  !transfers.value.has(result.transferId) ||
                  transfers.value.get(result.transferId)?.done,
                (done) => {
                  if (done) {
                    stop()
                    resolve()
                  }
                },
                { flush: 'sync' }
              )
              if (disposed || job.session !== currentConnectionId()) {
                stop()
                resolve()
              }
            })
          }
        } catch {
          // launchTransfer 已将错误保留在可重试任务行。
        } finally {
          active.delete(job.id)
          pump()
        }
      })()
    }
  }
  async function startTransfer(
    kind: 'upload' | 'download',
    localPath: string,
    remotePath: string,
    overwrite = true
  ) {
    const session = currentConnectionId()
    if (!session || disposed) throw new Error('SSH 未连接')
    const id = 'queued-' + crypto.randomUUID()
    transfers.value.set(id, {
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
      queued: true,
    })
    queue.push({ id, kind, localPath, remotePath, overwrite, session })
    transferStatus.value = '文件传输中'
    pump()
  }
  function cancelAll() {
    stopping = true
    for (const item of transfers.value.values()) if (!item.done) void cancelTransfer(item.id)
    stopping = false
    pump()
  }

  async function cancelTransfer(id: string) {
    const item = transfers.value.get(id)
    if (!item || item.done) return
    item.cancelling = true
    if (item.queued) {
      item.done = true
      item.queued = false
      item.error = '已取消'
      const index = queue.findIndex((job) => job.id === id)
      if (index >= 0) queue.splice(index, 1)
      trimHistory()
      transferStatus.value = [...transfers.value.values()].some((v) => !v.done) ? '文件传输中' : ''
      return
    }
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
    queue.splice(0)
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
    cancelAll()
    disposed = true
    for (const item of transfers.value.values()) if (!item.done) item.done = true
    unlisten?.()
  })
  return {
    transferStatus,
    transfers,
    cancelTransfer,
    applyProgress,
    clearEnded,
    startTransfer,
    cancelAll,
  }
}
