import { onMounted, onUnmounted, ref, type Ref } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { useUiStore } from '@/stores/ui'
import type { RemoteFile } from '../contracts'
import { useFileTransfer } from './useFileTransfer'
import { createDownloadTargets } from './downloadTargets'
import { join } from '@tauri-apps/api/path'
import { ipc } from '../ipc'

export function useSftpTransfers(options: {
  connectionId: () => string | undefined
  active: () => boolean | undefined
  currentPath: Ref<string>
  selectedFile: Ref<RemoteFile | null>
  /** 下载落盘目录（本地栏当前目录） */
  localDir: () => string
  preferredDirectory: () => string
  page: Ref<HTMLElement | null>
  refresh: () => void
  downloadDone: (path: string) => void
}) {
  const ui = useUiStore()
  const dragActive = ref(false)
  let disposed = false
  let stopDragDrop: (() => void) | null = null
  const { transferStatus, transfers, cancelTransfer, applyProgress, clearEnded, startTransfer } =
    useFileTransfer(options.connectionId, options.refresh, options.downloadDone)

  async function uploadLocalPaths(
    localPaths: string[],
    remoteDirectory = options.currentPath.value
  ) {
    const connectionId = options.connectionId()
    if (!connectionId || !localPaths.length) return
    let started = 0
    for (const localPath of localPaths) {
      const name = localPath.split(/[\\/]/).pop() ?? 'file'
      const remote = remoteDirectory.endsWith('/')
        ? `${remoteDirectory}${name}`
        : `${remoteDirectory}/${name}`
      try {
        await startTransfer('upload', localPath, remote)
        started += 1
      } catch (error) {
        ui.toast(`上传失败（${name}）：${error}`)
      }
    }
    if (started) ui.toast(started === 1 ? '已开始上传' : `已开始上传 ${started} 个文件`)
  }

  async function upload(remoteDirectory = options.currentPath.value, selectDirectory = false) {
    try {
      const localPath = await dialogOpen({ multiple: false, directory: selectDirectory })
      if (typeof localPath === 'string') await uploadLocalPaths([localPath], remoteDirectory)
    } catch (error) {
      ui.toast(`打开本地文件选择器失败：${error}`)
    }
  }

  const targets = createDownloadTargets({
    session: options.connectionId,
    defaultDirectory: async () => {
      const [path, warning] = await ipc.sshLocalDefaultDirectory(options.preferredDirectory())
      if (warning) ui.toast(warning)
      return path
    },
    chooseDirectory: (path) =>
      dialogOpen({
        directory: true,
        multiple: false,
        title: '下载到本地',
        defaultPath: path || undefined,
      }),
    join,
    submit: (localPath, remotePath) => startTransfer('download', localPath, remotePath),
    notify: (message) => ui.toast(message),
  })
  const downloadSelection = targets.choose
  const downloadToPane = (items: RemoteFile[]) => targets.direct(items, options.localDir())
  const download = (file: RemoteFile | null = options.selectedFile.value) =>
    targets.choose(file ? [file] : [])

  function dropIsInside(position: { x: number; y: number }) {
    const bounds = options.page.value?.getBoundingClientRect()
    if (!bounds) return false
    const scale = window.devicePixelRatio || 1
    const x = position.x / scale
    const y = position.y / scale
    return x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom
  }

  onMounted(async () => {
    try {
      stopDragDrop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
        if (!options.active()) {
          dragActive.value = false
          return
        }
        if (payload.type === 'leave') {
          dragActive.value = false
          return
        }
        const inside = dropIsInside(payload.position)
        dragActive.value = inside && payload.type !== 'drop'
        if (payload.type === 'drop' && inside) void uploadLocalPaths(payload.paths)
      })
      if (disposed) stopDragDrop()
    } catch {
      // 浏览器预览没有 Tauri 原生文件拖放事件。
    }
  })
  onUnmounted(() => {
    disposed = true
    stopDragDrop?.()
  })

  return {
    startTransfer,
    clearEnded,
    applyProgress,
    dragActive,
    transferStatus,
    transfers,
    cancelTransfer,
    uploadLocalPaths,
    upload,
    download,
    downloadSelection,
    downloadToPane,
  }
}
