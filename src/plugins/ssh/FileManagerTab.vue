<script setup lang="ts">
/** SSH 远程文件管理：导航、传输、危险操作与弹窗编辑。 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import type { ServerConnection, ServerProfile, RemoteFile } from './contracts'
import { canEditRemoteFile } from './useSsh'
import { useUiStore } from '@/stores/ui'
import { useSettingsStore } from '@/stores/settings'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import InputDialog from '@/core/ui/InputDialog.vue'
import EditorDialog from './EditorDialog.vue'
import FileBrowser from './FileBrowser.vue'
import { ipc } from './ipc'
import { useFileContextMenu } from './useFileContextMenu'
import { useFileTransfer } from './useFileTransfer'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  active?: boolean
}>()

const ui = useUiStore()
const settings = useSettingsStore()

const currentPath = ref('/')
const filePage = ref<HTMLElement | null>(null)
const dragActive = ref(false)
let stopDragDrop: (() => void) | null = null
const directoryHistory = ref<string[]>([])
const DIRECTORY_HISTORY_LIMIT = 20
const files = ref<RemoteFile[]>([])
const sortKey = ref<'name' | 'modifiedAt'>('name')
const sortDirection = ref<'asc' | 'desc'>('asc')
const directoryCache = new Map<string, RemoteFile[]>()
const selectedFile = ref<RemoteFile | null>(null)
const transferStatus = useFileTransfer(
  () => props.connection?.sessionId,
  () => void refreshCurrent()
)

const editing = ref<{ connectionId: string; path: string; content: string } | null>(null)
const savingEdit = ref(false)
const renameTarget = ref<RemoteFile | null>(null)
const deleteTarget = ref<RemoteFile | null>(null)

const parentPath = computed(() => {
  const p = currentPath.value
  if (p === '/') return null
  const idx = p.lastIndexOf('/')
  return idx <= 0 ? '/' : p.slice(0, idx)
})

const sortedFiles = computed(() =>
  [...files.value].sort((left, right) => {
    if (left.isDir !== right.isDir) return left.isDir ? -1 : 1
    const comparison =
      sortKey.value === 'name'
        ? left.name.localeCompare(right.name, 'zh-CN', { numeric: true, sensitivity: 'base' })
        : left.modifiedAt - right.modifiedAt
    return sortDirection.value === 'asc' ? comparison : -comparison
  })
)

function cacheKey(connectionId: string, path: string) {
  return `${connectionId}\u0000${path}`
}

async function navigate(path: string, force = false, recordHistory = true) {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  const previousPath = currentPath.value
  if (recordHistory && path !== previousPath) {
    directoryHistory.value = [...directoryHistory.value, previousPath].slice(
      -DIRECTORY_HISTORY_LIMIT
    )
  }
  currentPath.value = path
  selectedFile.value = null
  const key = cacheKey(connectionId, path)
  const cached = directoryCache.get(key)
  if (cached && !force) {
    files.value = cached
    return
  }
  try {
    const r = await ipc.sshFileList(connectionId, path)
    if (props.connection?.sessionId !== connectionId) return
    if (r.ok) {
      files.value = r.files
      directoryCache.set(key, r.files)
    } else {
      ui.toast(`读取目录失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`读取目录失败：${e}`)
  }
}

function refreshCurrent() {
  return navigate(currentPath.value, true)
}

function changeSort(key: 'name' | 'modifiedAt') {
  if (sortKey.value === key) sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc'
  else {
    sortKey.value = key
    sortDirection.value = key === 'name' ? 'asc' : 'desc'
  }
}

function navigateUp() {
  if (parentPath.value) navigate(parentPath.value)
}

function navigateBack() {
  const previousPath = directoryHistory.value[directoryHistory.value.length - 1]
  if (!previousPath) return
  directoryHistory.value = directoryHistory.value.slice(0, -1)
  void navigate(previousPath, false, false)
}

/** 双击：目录进入，符合文本规则的文件打开统一编辑弹窗。 */
async function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path)
    return
  }
  if (!canEditRemoteFile(file)) {
    ui.toast('该文件类型或大小不支持在线编辑')
    return
  }
  await openFile(file)
}

/** 双击与右键“编辑”共享同一打开路径。 */
async function openFile(file: RemoteFile) {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  try {
    const r = await ipc.sshEditOpen(connectionId, file.path)
    if (props.connection?.sessionId !== connectionId) return
    if (r.ok) {
      editing.value = { connectionId, path: r.path, content: r.content }
    } else {
      ui.toast(`打开文件失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`打开文件失败：${e}`)
  }
}

/** 保存：回写服务器（ssh_edit_save） */
async function onSave(content: string) {
  const target = editing.value
  if (!target || savingEdit.value) return
  savingEdit.value = true
  try {
    const r = await ipc.sshEditSave(target.connectionId, target.path, content)
    if (r.ok) {
      ui.toast(`已保存 ${target.path}（${content.length} 字符）`)
      editing.value = null
    } else {
      ui.toast(`保存失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`保存失败：${e}`)
  } finally {
    savingEdit.value = false
  }
}

/** 将一个或多个本地文件传输到当前目录。 */
async function uploadLocalPaths(localPaths: string[], remoteDirectory = currentPath.value) {
  const connectionId = props.connection?.sessionId
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
    } catch (e) {
      ui.toast(`上传失败（${name}）：${e}`)
    }
  }
  if (started) ui.toast(started === 1 ? '已开始上传' : `已开始上传 ${started} 个文件`)
}

/** 上传：选择本地文件 → 传输到当前目录 */
async function upload(remoteDirectory = currentPath.value, selectDirectory = false) {
  let localPath: string | null
  try {
    localPath = await dialogOpen({ multiple: false, directory: selectDirectory })
  } catch (error) {
    ui.toast(`打开本地文件选择器失败：${error}`)
    return
  }
  if (typeof localPath !== 'string') return
  await uploadLocalPaths([localPath], remoteDirectory)
}

function dropIsInside(position: { x: number; y: number }) {
  const bounds = filePage.value?.getBoundingClientRect()
  if (!bounds) return false
  const scale = window.devicePixelRatio || 1
  const x = position.x / scale
  const y = position.y / scale
  return x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom
}

onMounted(async () => {
  try {
    stopDragDrop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (!props.active) {
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
    /* 浏览器预览没有 Tauri 原生文件拖放事件。 */
  }
})

onUnmounted(() => stopDragDrop?.())

/** 下载：选择保存位置 → 传输到本地 */
async function download(file: RemoteFile | null = selectedFile.value) {
  if (!file || !props.connection?.sessionId) {
    if (!file) ui.toast('请先选择文件')
    return
  }
  let localPath: string | null
  try {
    const directory = settings.settings.defaultDownloadDirectory.trim()
    const separator = directory.includes('\\') ? '\\' : '/'
    const defaultPath = directory
      ? `${directory.replace(/[\\/]$/, '')}${separator}${file.name}`
      : file.name
    localPath = await dialogSave({ defaultPath })
  } catch (error) {
    ui.toast(`打开保存位置选择器失败：${error}`)
    return
  }
  if (!localPath) return
  try {
    await ipc.sshFileDownload({
      connectionId: props.connection.sessionId,
      remotePath: file.path,
      localPath,
    })
    transferStatus.value = `正在下载 ${file.name}`
    ui.toast(`已开始下载：${file.name}`)
  } catch (e) {
    ui.toast(`下载失败：${e}`)
  }
}

function requestDelete(file: RemoteFile | null = selectedFile.value) {
  if (!file) {
    ui.toast('请先选择文件')
    return
  }
  deleteTarget.value = file
}

async function confirmDelete() {
  const file = deleteTarget.value
  deleteTarget.value = null
  if (!file || !props.connection?.sessionId) return
  try {
    const r = await ipc.sshFileDelete(props.connection.sessionId, file.path, file.isDir)
    if (r.ok) {
      ui.toast(`已删除 ${file.name}`)
      selectedFile.value = null
      refreshCurrent()
    } else {
      ui.toast(`删除失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`删除失败：${e}`)
  }
}

function requestRename(file: RemoteFile | null = selectedFile.value) {
  if (!file) {
    ui.toast('请先选择文件')
    return
  }
  renameTarget.value = file
}

async function confirmRename(name: string) {
  const file = renameTarget.value
  renameTarget.value = null
  if (!file || !props.connection?.sessionId || name === file.name) return
  const dir = file.path.slice(0, file.path.lastIndexOf('/') + 1)
  const newPath = `${dir}${name}`
  try {
    const r = await ipc.sshFileRename(props.connection.sessionId, file.path, newPath)
    if (r.ok) {
      ui.toast(`已重命名为 ${name}`)
      selectedFile.value = null
      refreshCurrent()
    } else {
      ui.toast(`重命名失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`重命名失败：${e}`)
  }
}

const { menu, menuItems, openMenu } = useFileContextMenu({
  refresh: () => void refreshCurrent(),
  upload: (target) => void upload(target?.isDir ? target.path : currentPath.value),
  uploadDirectory: (target) => void upload(target?.isDir ? target.path : currentPath.value, true),
  download: (file) => void download(file),
  edit: (file) => void openFile(file),
  rename: requestRename,
  select: (file) => (selectedFile.value = file),
})

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    files.value = []
    directoryCache.clear()
    selectedFile.value = null
    editing.value = null
    savingEdit.value = false
    renameTarget.value = null
    deleteTarget.value = null
    menu.value = null
    directoryHistory.value = []
    currentPath.value = '/'
    if (sessionId) void navigate('/')
  },
  { immediate: true }
)
</script>

<template>
  <div ref="filePage" class="relative flex h-full min-h-0 flex-col">
    <div
      v-if="dragActive"
      class="pointer-events-none absolute inset-[8px] z-[60] flex items-center justify-center rounded-lg border-2 border-dashed border-tertiary bg-tertiary-soft/90 text-h2 text-tertiary-strong shadow-card dark:border-tertiary-dark dark:bg-tertiary-soft-dark/90 dark:text-tertiary-dark"
    >
      释放文件，上传到 {{ currentPath }}
    </div>
    <FileBrowser
      :current-path="currentPath"
      :files="sortedFiles"
      :active="active"
      :can-go-back="directoryHistory.length > 0"
      :sort-key="sortKey"
      :sort-direction="sortDirection"
      :selected-path="selectedFile?.path"
      :selected-name="selectedFile?.name"
      :transfer-status="transferStatus"
      @navigate="navigate"
      @back="navigateBack"
      @up="navigateUp"
      @upload="upload"
      @upload-directory="upload(currentPath, true)"
      @download="download()"
      @rename="requestRename()"
      @delete="requestDelete()"
      @select="selectedFile = $event"
      @open="onDoubleClick"
      @context="openMenu"
      @sort="changeSort"
    />

    <EditorDialog
      v-if="editing"
      :path="editing.path"
      :content="editing.content"
      :saving="savingEdit"
      @save="onSave"
      @cancel="editing = null"
    />
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
    <InputDialog
      :open="renameTarget !== null"
      title="重命名"
      label="新名称"
      :initial-value="renameTarget?.name"
      confirm-label="重命名"
      @close="renameTarget = null"
      @confirm="confirmRename"
    />
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除文件"
      :message="`确定删除「${deleteTarget?.name ?? ''}」？此操作不可恢复。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="confirmDelete"
    />
  </div>
</template>
