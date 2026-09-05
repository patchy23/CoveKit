<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, RemoteFile } from './contracts'
import { canEditRemoteFile } from './useSsh'
import { useUiStore } from '@/stores/ui'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import FileBrowser from './FileBrowser.vue'
import FileManagerDialogs from './FileManagerDialogs.vue'
import { ipc } from './ipc'
import { useFileContextMenu } from './useFileContextMenu'
import { useSftpTransfers } from './useSftpTransfers'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  active?: boolean
}>()

const ui = useUiStore()

const currentPath = ref('/')
const filePage = ref<HTMLElement | null>(null)
const directoryHistory = ref<string[]>([])
const DIRECTORY_HISTORY_LIMIT = 20
const files = ref<RemoteFile[]>([])
const sortKey = ref<'name' | 'modifiedAt'>('name')
const sortDirection = ref<'asc' | 'desc'>('asc')
const directoryCache = new Map<string, RemoteFile[]>()
/** 目录缓存上限：超出时淘汰最旧条目（Map 保持插入序），防长期浏览无限增长 */
const DIRECTORY_CACHE_LIMIT = 100
/** 目录请求序号：快速连续切换时只认最后一次请求的目录（竞态守卫） */
let navigateSeq = 0

/** 写入目录缓存（带淘汰：同 key 刷新位置，超限删最旧） */
function cacheDirectory(key: string, list: RemoteFile[]) {
  if (directoryCache.has(key)) directoryCache.delete(key)
  directoryCache.set(key, list)
  if (directoryCache.size > DIRECTORY_CACHE_LIMIT) {
    const oldest = directoryCache.keys().next().value
    if (oldest !== undefined) directoryCache.delete(oldest)
  }
}
const selectedFile = ref<RemoteFile | null>(null)

const editing = ref<{ connectionId: string; path: string; content: string } | null>(null)
const savingEdit = ref(false)
const renameTarget = ref<RemoteFile | null>(null)
const deleteTarget = ref<RemoteFile | null>(null)

const { dragActive, transferStatus, upload, download } = useSftpTransfers({
  connectionId: () => props.connection?.sessionId,
  active: () => props.active,
  currentPath,
  selectedFile,
  page: filePage,
  refresh: () => void refreshCurrent(),
})

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
  // 竞态守卫：序号单调递增，慢返回的旧目录请求直接丢弃（不再回写 files/缓存）
  const seq = ++navigateSeq
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
    if (seq !== navigateSeq) return // 已有更新的目录请求，丢弃本次结果
    if (props.connection?.sessionId !== connectionId) return
    if (r.ok) {
      files.value = r.files
      cacheDirectory(key, r.files)
    } else {
      ui.toast(`读取目录失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    if (seq !== navigateSeq) return
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

    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
    <FileManagerDialogs
      :editing="editing"
      :saving="savingEdit"
      :rename-target="renameTarget"
      :delete-target="deleteTarget"
      @cancel-edit="editing = null"
      @cancel-rename="renameTarget = null"
      @cancel-delete="deleteTarget = null"
      @save="onSave"
      @rename="confirmRename"
      @delete="confirmDelete"
    />
  </div>
</template>
