import { onMounted, onUnmounted, ref, type Ref } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import { useUiStore } from '@/stores/ui'
import { useSettingsStore } from '@/stores/settings'
import type { RemoteFile } from './contracts'
import { ipc } from './ipc'
import { useFileTransfer } from './useFileTransfer'

export function useSftpTransfers(options: {
  connectionId: () => string | undefined
  active: () => boolean | undefined
  currentPath: Ref<string>
  selectedFile: Ref<RemoteFile | null>
  page: Ref<HTMLElement | null>
  refresh: () => void
}) {
  const ui = useUiStore()
  const settings = useSettingsStore()
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

  async function download(file: RemoteFile | null = options.selectedFile.value) {
    const connectionId = options.connectionId()
    if (!file || !connectionId) {
      if (!file) ui.toast('请先选择文件')
      return
    }
    try {
      const directory = settings.settings.defaultDownloadDirectory.trim()
      const separator = directory.includes('\\') ? '\\' : '/'
      const defaultPath = directory
        ? `${directory.replace(/[\\/]$/, '')}${separator}${file.name}`
        : file.name
      const localPath = await dialogSave({ defaultPath })
      if (!localPath) return
      await ipc.sshFileDownload({ connectionId, remotePath: file.path, localPath })
      transferStatus.value = `正在下载 ${file.name}`
      ui.toast(`已开始下载：${file.name}`)
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

  return { dragActive, transferStatus, transfers, cancelTransfer, uploadLocalPaths, upload, download }
}
