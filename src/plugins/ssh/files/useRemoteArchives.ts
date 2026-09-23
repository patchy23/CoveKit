/** 归档任务属于文件工作区，弹窗关闭只取消它自己的扫描。 */
import { computed, onUnmounted, ref, watch } from 'vue'
import { ipc } from '../ipc'
import type { ArchiveEntry, ArchiveEvent, ArchiveRequest } from '../contracts'
import type { TransferItem } from './useFileTransfer'
import { archiveFormat } from './archiveFiles'
import { useUiStore } from '@/stores/ui'
export function useRemoteArchives(session: () => string | undefined, refresh: () => void) {
  const ui = useUiStore()
  const tasks = ref(new Map<string, TransferItem>())
  const preview = ref<{
    path: string
    id: string
    entries: ArchiveEntry[]
    busy: boolean
    error: string
    minimized: boolean
  }>()
  const active = computed(() => [...tasks.value.values()].filter((item) => !item.done).length)
  let disposed = false
  const requests = new Map<string, ArchiveRequest>()
  const registered = new Set<string>()
  function clearEnded() {
    tasks.value = new Map([...tasks.value].filter(([, item]) => !item.done))
    for (const id of requests.keys()) if (!tasks.value.has(id)) requests.delete(id)
  }
  function trim() {
    const ended = [...tasks.value.values()].filter((item) => item.done)
    for (const item of ended.slice(0, Math.max(0, ended.length - 200))) {
      tasks.value.delete(item.id)
      requests.delete(item.id)
    }
  }
  async function cancel(id: string) {
    const item = tasks.value.get(id)
    if (!item || item.done) return
    item.cancelling = true
    if (!registered.has(id)) return
    try {
      await ipc.sshArchiveCancel(id)
    } catch (error) {
      item.cancelling = false
      item.error = '取消请求失败：' + String(error)
    }
  }
  function apply(id: string, event: ArchiveEvent) {
    const item = tasks.value.get(id)
    if (!item || item.done || disposed) return
    if (!registered.has(id)) {
      registered.add(id)
      if (item.cancelling) void cancel(id)
    }
    item.preparing = event.kind === 'queued'
    if (event.kind === 'progress') {
      item.transferred = event.transferred ?? 0
      item.total = event.total ?? 0
    }
    if (event.error) item.error = event.error
    const current = preview.value
    if (current?.id === id && event.entries) {
      const remaining = 100000 - current.entries.length
      current.entries.push(...event.entries.slice(0, remaining))
      if (event.entries.length > remaining) {
        current.error = '条目达到 10 万条上限，已停止扫描'
        void cancel(id)
      }
    }
    // 流中的 result 仍需等待进程退出和清理完成，IPC 返回才提交 UI 终态。
  }
  function start(request: ArchiveRequest, previewPath?: string) {
    const connectionId = session()
    if (!connectionId) {
      ui.toast('SSH 未连接')
      return
    }
    const id = 'archive-' + crypto.randomUUID()
    tasks.value.set(id, {
      id,
      kind: request.operation,
      label:
        request.operation === 'compress'
          ? request.output.split('/').pop() || '压缩包'
          : request.paths[0].split('/').pop() || '压缩包',
      localPath: '',
      remotePath: request.operation === 'preview' ? request.paths[0] : request.output,
      transferred: 0,
      total: 0,
      done: false,
      speed: 0,
      updatedAt: Date.now(),
      preparing: true,
    })
    requests.set(id, request)
    if (previewPath)
      preview.value = {
        path: previewPath,
        id,
        entries: [],
        busy: true,
        error: '',
        minimized: false,
      }
    void ipc
      .sshArchiveRun(connectionId, id, request, (event) => apply(id, event))
      .then((result) => {
        const item = tasks.value.get(id)
        if (!item || disposed) return
        item.done = true
        item.preparing = false
        item.cancelling = result.status === 'cancelled'
        item.error = result.error || (result.status === 'failed' ? '归档任务失败' : undefined)
        if (
          result.status === 'succeeded' &&
          request.operation !== 'preview' &&
          session() === connectionId
        )
          refresh()
      })
      .catch((error) => {
        const item = tasks.value.get(id)
        if (item) {
          item.done = true
          item.preparing = false
          item.cancelling = false
          item.error = String(error)
        }
      })
      .finally(() => {
        registered.delete(id)
        if (preview.value?.id === id) {
          preview.value.busy = false
          const item = tasks.value.get(id)
          preview.value.error =
            item?.error || (item?.cancelling ? '扫描已取消，当前为已读取部分' : '')
        }
        trim()
      })
    return id
  }
  function openPreview(path: string) {
    const format = archiveFormat(path)
    if (!format) return
    if (preview.value?.path === path) {
      preview.value.minimized = false
      return
    }
    closePreview()
    start({ operation: 'preview', format, paths: [path], output: '' }, path)
  }
  function retry(id: string) {
    const request = requests.get(id)
    if (!request) return
    if (request.operation === 'preview') {
      closePreview()
      start(request, request.paths[0])
    } else start(request)
  }
  function closePreview() {
    if (preview.value?.busy) void cancel(preview.value.id)
    preview.value = undefined
  }
  watch(session, () => {
    for (const item of tasks.value.values()) if (!item.done) void cancel(item.id)
    closePreview()
  })
  onUnmounted(() => {
    for (const item of tasks.value.values()) if (!item.done) void cancel(item.id)
    disposed = true
  })
  return { tasks, preview, active, start, cancel, clearEnded, openPreview, closePreview, retry }
}
