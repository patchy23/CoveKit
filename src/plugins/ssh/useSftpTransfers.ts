import { onMounted, onUnmounted, ref, type Ref } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { useUiStore } from '@/stores/ui'
import type { RemoteFile } from './contracts'
import { ipc } from './ipc'
import { useFileTransfer } from './useFileTransfer'

export function useSftpTransfers(options: {
  connectionId: () => string | undefined
  active: () => boolean | undefined
  currentPath: Ref<string>
  selectedFile: Ref<RemoteFile | null>
  /** 下载落盘目录（本地栏当前目录） */
  localDir: () => string
  page: Ref<HTMLElement | null>
  refresh: () => void
}) {
  const ui = useUiStore()
  const dragActive = ref(false)
  let stopDragDrop: (() => void) | null = null
  const { transferStatus, transfers, cancelTransfer } = useFileTransfer(
    options.connectionId,
    options.refresh
  )

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
        await ipc.sshFileUpload({ connectionId, localPath, remotePath: remote })
        started += 1
        transferStatus.value = `正在上传 ${name}`
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

  /**
   * 下载到本地栏当前目录（不弹保存框；目录由调用方注入）。
   * 同名文件直接覆盖——传输面板有进度与取消，行为与 WinSCP 落盘一致。
   */
  async function download(file: RemoteFile | null = options.selectedFile.value) {
    const connectionId = options.connectionId()
    if (!file || !connectionId) {
      if (!file) ui.toast('请先选择文件')
      return
    }
    const dir = options.localDir()
    if (!dir) {
      ui.toast('本地目录尚未就绪，无法下载')
      return
    }
    try {
      const separator = dir.includes('\\') ? '\\' : '/'
      const localPath = `${dir.replace(/[\\/]$/, '')}${separator}${file.name}`
      if (file.isDir) {
        // 目录走递归下载（与批量下载一致），任务进传输面板
        await ipc.sshFileDownloadRecursive({ connectionId, remotePath: file.path, localPath })
      } else {
        await ipc.sshFileDownload({ connectionId, remotePath: file.path, localPath })
      }
      transferStatus.value = `正在下载 ${file.name}`
      ui.toast(`已开始下载：${file.name} → ${dir}`)
    } catch (error) {
      ui.toast(`下载失败：${error}`)
    }
  }

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
    } catch {
      // 浏览器预览没有 Tauri 原生文件拖放事件。
    }
  })
  onUnmounted(() => stopDragDrop?.())

  return {
    dragActive,
    transferStatus,
    transfers,
    cancelTransfer,
    uploadLocalPaths,
    upload,
    download,
  }
}
