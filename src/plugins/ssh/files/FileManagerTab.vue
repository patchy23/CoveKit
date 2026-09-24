<script setup lang="ts">
/** FileManagerTab · 文件页签装配层（导航 useRemoteDirectory + 多选/批量/菜单/本地侧各 composable 组合） */
import { UiButton, UiConfirmDialog, UiAlert } from '@/core/ui'
import TransferPanel from './TransferPanel.vue'
import ArchiveDialog from './ArchiveDialog.vue'
import ArchivePreview from './ArchivePreview.vue'
import { useRemoteArchives } from './useRemoteArchives'
import { archiveFormat } from './archiveFiles'
import type { ArchiveRequest } from '../contracts'
import type { TransferItem } from './useFileTransfer'
import { dirname } from '@tauri-apps/api/path'
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, RemoteFile } from '../contracts'
import { useRemoteFileOps } from './useRemoteFileOps'
import { useRemoteCreateOps } from './useRemoteCreateOps'
import { useRemoteDirectory } from './useRemoteDirectory'
import { useUiStore } from '@/stores/ui'
import { useSettingsStore } from '@/stores/settings'
import FileManagerPanes from './FileManagerPanes.vue'
import FileManagerOverlays from './FileManagerOverlays.vue'
import { useFileManagerMenus } from './useFileManagerMenus'
import { useFileSelection } from './useFileSelection'
import { useFileBatchOps } from './useFileBatchOps'
import { useLocalFileOps } from './useLocalFileOps'
import { useSftpTransfers } from './useSftpTransfers'
const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  active?: boolean
  navigation?: { id: number; sessionId: string; path: string }
}>()

const activeSessionId = computed(() =>
  props.connection?.status === 'connected' ? props.connection.sessionId : undefined
)
const activePopover = ref<'bookmarks' | 'transfers' | null>(null)
const transfersOpen = computed({
  get: () => activePopover.value === 'transfers',
  set: (open) => {
    activePopover.value = open ? 'transfers' : null
  },
})
const retryTarget = ref<TransferItem>()
async function retryTransfer(item: TransferItem) {
  const connectionId = activeSessionId.value
  if (!connectionId || (item.kind !== 'upload' && item.kind !== 'download')) return
  try {
    await startTransfer(item.kind, item.localPath, item.remotePath, true)
  } catch (e) {
    ui.toast('重新传输失败：' + String(e))
  }
}
async function locateTransfer(item: TransferItem) {
  if (item.kind !== 'download')
    await navigate(item.remotePath.slice(0, item.remotePath.lastIndexOf('/')) || '/')
  else {
    try {
      await panes.value?.locateLocal(await dirname(item.localPath))
    } catch (error) {
      ui.toast('定位本地目录失败：' + String(error))
    }
  }
}
const emit = defineEmits<{
  openFile: [file: Pick<RemoteFile, 'path'>]
  renamed: [oldPath: string, newPath: string]
  directory: [path: string]
}>()
const openFile = async (file: Pick<RemoteFile, 'path'>) => {
  emit('openFile', file)
}
const openFileGuarded = openFile
const ui = useUiStore()
const settings = useSettingsStore()
const filePage = ref<HTMLElement | null>(null)
const panes = ref<InstanceType<typeof FileManagerPanes> | null>(null)
const localBrowser = computed(() => panes.value?.localBrowser ?? null)

/* ── 远程目录导航（缓存/历史/排序/竞态守卫在 useRemoteDirectory） ── */
const remoteSel = useFileSelection(() => sortedFiles.value.map((f) => f.path))
const {
  currentPath,
  directoryHistory,
  sortKey,
  sortDirection,
  sortedFiles,
  navigate,
  refreshCurrent,
  changeSort,
  navigateUp,
  navigateBack,
  reset,
} = useRemoteDirectory({
  sessionId: () => activeSessionId.value,
  onNavigate: () => remoteSel.clear(),
})

const localSel = useFileSelection(() => localBrowser.value?.files.map((f) => f.path) ?? [])

/** 选中项列表（按列表顺序，远程/本地各一） */
const selectedRemoteFiles = computed(() =>
  sortedFiles.value.filter((f) => remoteSel.selectedPaths.value.has(f.path))
)
const selectedLocalFiles = computed(() =>
  (localBrowser.value?.files ?? []).filter((f: RemoteFile) =>
    localSel.selectedPaths.value.has(f.path)
  )
)
/** 单选 shim：恰好选中 1 项时返回该项；置 null = 清空选择（兼容 useRemoteFileOps 的 Ref 契约） */
const selectedFile = computed<RemoteFile | null>({
  get: () => (selectedRemoteFiles.value.length === 1 ? selectedRemoteFiles.value[0] : null),
  set: (v) => {
    if (!v) remoteSel.clear()
  },
})

const {
  chmodTarget,
  requestChmod,
  confirmChmod,
  deleteTarget,
  renameTarget,
  requestDelete,
  confirmDelete,
  requestRename,
  confirmRename,
} = useRemoteFileOps({
  sessionId: () => activeSessionId.value,
  selectedFile,
  onRename: (oldPath, newPath) => emit('renamed', oldPath, newPath),
  refresh: () => refreshCurrent(),
})

const {
  startTransfer,
  clearEnded,
  dragActive,
  transferStatus,
  transfers,
  cancelTransfer,
  cancelAll,
  uploadLocalPaths,
  upload,
  download,
  downloadSelection: requestBatchDownload,
  downloadToPane,
} = useSftpTransfers({
  connectionId: () => activeSessionId.value,
  active: () => props.active,
  currentPath,
  selectedFile,
  localDir: () => localBrowser.value?.currentPath ?? '',
  preferredDirectory: () => settings.settings.defaultDownloadDirectory || '',
  downloadDone: async (path) => {
    try {
      const parent = await dirname(path)
      if (parent === localBrowser.value?.currentPath) await localBrowser.value.refresh()
    } catch (error) {
      ui.toast('下载已完成，本地列表刷新失败：' + String(error))
    }
  },
  page: filePage,
  refresh: () => void refreshCurrent(),
})

/* 批量操作（确认弹窗 + 执行在 useFileBatchOps） */
const {
  batchConfirm,
  batchRunning,
  requestBatchDeleteRemote,
  requestBatchUpload,
  requestBatchDeleteLocal,
  uploadSelection,
  confirmBatch,
} = useFileBatchOps({
  sessionId: () => activeSessionId.value,
  refreshRemote: () => void refreshCurrent(),
  refreshLocal: () => localBrowser.value?.refresh(),
  uploadLocalPaths,
  remoteDir: () => currentPath.value,
})
const {
  deleteTarget: localDeleteTarget,
  renameTarget: localRenameTarget,
  createKind: localCreateKind,
  confirmCreate: confirmLocalCreate,
  requestRename: requestLocalRename,
  confirmRename: confirmLocalRename,
  requestDelete: requestLocalDelete,
  confirmDelete: confirmLocalDelete,
} = useLocalFileOps({
  localDir: () => localBrowser.value?.currentPath ?? '',
  afterChange: () => {
    localSel.clear()
    localBrowser.value?.refresh()
  },
})

const { newFileTarget, mkdirTarget, confirmNewFile, confirmMkdir, requestNewFile, requestMkdir } =
  useRemoteCreateOps({
    sessionId: () => activeSessionId.value,
    currentDir: () => currentPath.value,
    refresh: () => void refreshCurrent(),
  })

/** 双击：目录进入，文本文件打开编辑工作区 */
async function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path)
    return
  }
  if (archiveFormat(file.path)) archives.openPreview(file.path)
  else await openFileGuarded(file)
}

const archives = useRemoteArchives(
  () => activeSessionId.value,
  () => {
    void refreshCurrent()
  }
)
const allTasks = computed(() =>
  [...transfers.value.values(), ...archives.tasks.value.values()].sort(
    (a, b) => a.updatedAt - b.updatedAt
  )
)
const archiveDialog = ref<{
  mode: 'compress' | 'extract'
  files: RemoteFile[]
  sessionId: string
}>()
function requestArchive(mode: 'compress' | 'extract', files: RemoteFile[]) {
  const sessionId = activeSessionId.value
  if (sessionId && files.length)
    archiveDialog.value = { mode, files: files.map((file) => ({ ...file })), sessionId }
}
function submitArchive(request: ArchiveRequest) {
  if (archiveDialog.value?.sessionId !== activeSessionId.value) {
    ui.toast('连接已变化，请重新选择文件')
    return
  }
  archives.start(request)
  archiveDialog.value = undefined
  transfersOpen.value = true
}
function archiveSource(): RemoteFile | undefined {
  const path = archives.preview.value?.path
  if (!path) return
  return (
    sortedFiles.value.find((file) => file.path === path) ?? {
      name: path.split('/').pop() || '压缩包',
      path,
      isDir: false,
      size: 0,
      modifiedAt: 0,
      owner: '',
      group: '',
      permissions: '',
    }
  )
}
function extractPreview() {
  const file = archiveSource()
  if (file) requestArchive('extract', [file])
}
function downloadPreview() {
  const file = archiveSource()
  if (file) void download(file)
}
function reloadPreview() {
  const path = archives.preview.value?.path
  archives.closePreview()
  if (path) archives.openPreview(path)
}
function cancelAllTasks() {
  cancelAll()
  for (const task of allTasks.value)
    if (task.id.startsWith('archive-') && !task.done) cancelTask(task.id)
}
function cancelTask(id: string) {
  if (id.startsWith('archive-')) void archives.cancel(id)
  else void cancelTransfer(id)
}
function clearTasks() {
  clearEnded()
  archives.clearEnded()
}

const { menu, menuItems, openMenu, localMenu, localMenuItems, openLocalMenu } = useFileManagerMenus(
  {
    remoteSel,
    localSel,
    selectedRemoteFiles: () => selectedRemoteFiles.value,
    selectedLocalFiles: () => selectedLocalFiles.value,
    refreshRemote: () => void refreshCurrent(),
    refreshLocal: () => localBrowser.value?.refresh(),
    currentDir: () => currentPath.value,
    requestNewFile,
    requestMkdir,
    upload,
    download: (file) => void download(file),
    requestBatchDownload,
    openFile,
    requestRename,
    requestDelete,
    requestBatchDeleteRemote,
    onChmod: requestChmod,
    compress: (files) => requestArchive('compress', files),
    extract: (file) => requestArchive('extract', [file]),
    preview: (file) => archives.openPreview(file.path),
    onBookmark: (dir) => panes.value?.statusActions?.addBookmark(dir),
    uploadLocalPaths,
    requestBatchUpload,
    requestLocalNewFile: () => (localCreateKind.value = 'file'),
    requestLocalMkdir: () => (localCreateKind.value = 'dir'),
    requestLocalRename,
    requestLocalDelete,
    requestBatchDeleteLocal,
  }
)

watch(
  () => activeSessionId.value,
  (sessionId) => {
    archiveDialog.value = undefined
    // 换连接/断开：清空全部交互态（菜单/弹窗/选择），目录回到根
    remoteSel.clear()
    localSel.clear()
    renameTarget.value = null
    deleteTarget.value = null
    menu.value = null
    localMenu.value = null
    mkdirTarget.value = null
    newFileTarget.value = null
    reset(
      sessionId,
      props.navigation && props.navigation.sessionId === sessionId ? props.navigation.path : '/'
    )
  },
  { immediate: true }
)
watch(currentPath, (path) => emit('directory', path), { immediate: true })
watch(
  () => props.navigation,
  (request) => {
    if (request && request.sessionId === activeSessionId.value) void navigate(request.path)
  }
)
</script>

<template>
  <div ref="filePage" class="relative flex h-full min-h-0 flex-col">
    <UiAlert v-if="connection?.status !== 'connected'" tone="warning" size="sm"
      >SSH 已断开，文件操作暂不可用；编辑器草稿已保留。</UiAlert
    >
    <FileManagerPanes
      ref="panes"
      :drag-active="dragActive"
      :current-path="currentPath"
      :files="sortedFiles"
      :active="active"
      :can-go-back="directoryHistory.length > 0"
      :sort-key="sortKey"
      :sort-direction="sortDirection"
      :remote-selected-paths="remoteSel.selectedPaths.value"
      :remote-selected-count="remoteSel.count.value"
      :transfer-status="transferStatus"
      :local-initial-path="settings.settings.defaultDownloadDirectory || ''"
      :local-selected-paths="localSel.selectedPaths.value"
      :profile-id="profile?.id"
      :bookmarks-open="activePopover === 'bookmarks'"
      @update:bookmarks-open="activePopover = $event ? 'bookmarks' : null"
      @navigate="navigate"
      @back="navigateBack"
      @up="navigateUp"
      @remote-row-click="(e: MouseEvent, f: RemoteFile) => remoteSel.onRowClick(e, f.path)"
      @open="onDoubleClick"
      @remote-context="openMenu"
      @sort="changeSort"
      @upload-local="uploadSelection(selectedLocalFiles)"
      @local-row-click="(e: MouseEvent, f: RemoteFile) => localSel.onRowClick(e, f.path)"
      @local-row-context="openLocalMenu"
      @local-blank-context="(e: MouseEvent) => openLocalMenu(e, null)"
      @local-error="(m: string) => ui.toast(m)"
      @drop-remote-to-local="downloadToPane"
      @drop-local-to-remote="
        (items: RemoteFile[]) =>
          void uploadLocalPaths(
            items.map((f) => f.path),
            currentPath
          )
      "
    >
      <template #file-actions>
        <UiButton
          size="xs"
          variant="ghost"
          :disabled="!selectedRemoteFiles.length || !activeSessionId"
          @click="requestBatchDownload(selectedRemoteFiles)"
          >下载到本地</UiButton
        >
        <UiButton
          size="xs"
          variant="ghost"
          :disabled="!selectedRemoteFiles.length || !activeSessionId"
          @click="requestArchive('compress', selectedRemoteFiles)"
          >压缩</UiButton
        >
        <UiButton
          v-if="selectedFile && !selectedFile.isDir && archiveFormat(selectedFile.path)"
          size="xs"
          variant="ghost"
          @click="requestArchive('extract', [selectedFile])"
          >解压</UiButton
        >
      </template>
      <template #transfers>
        <UiButton
          v-if="archives.preview.value?.minimized"
          size="sm"
          variant="ghost"
          @click="archives.preview.value.minimized = false"
          >压缩包预览</UiButton
        >
        <TransferPanel
          v-model:open="transfersOpen"
          :items="allTasks"
          @cancel="cancelTask"
          @cancel-all="cancelAllTasks"
          @clear="clearTasks"
          @retry="
            $event.id.startsWith('archive-') ? archives.retry($event.id) : (retryTarget = $event)
          "
          @locate="locateTransfer"
        />
      </template>
    </FileManagerPanes>
    <ArchiveDialog
      v-if="archiveDialog"
      :mode="archiveDialog.mode"
      :files="archiveDialog.files"
      :directory="currentPath"
      @close="archiveDialog = undefined"
      @submit="submitArchive"
    />
    <ArchivePreview
      v-if="archives.preview.value"
      v-show="!archives.preview.value.minimized"
      :key="archives.preview.value.id"
      :path="archives.preview.value.path"
      :entries="archives.preview.value.entries"
      :busy="archives.preview.value.busy"
      :error="archives.preview.value.error"
      @close="archives.closePreview"
      @minimize="archives.preview.value.minimized = true"
      @cancel="archives.cancel(archives.preview.value.id)"
      @reload="reloadPreview"
      @extract="extractPreview"
      @download="downloadPreview"
    />
    <UiConfirmDialog
      :open="!!retryTarget"
      title="重新传输文件"
      message="将重新传输整个文件或目录，目标位置已有的同名文件会被覆盖。"
      confirm-label="重新传输"
      @close="retryTarget = undefined"
      @confirm="
        () => {
          const item = retryTarget
          retryTarget = undefined
          if (item) void retryTransfer(item)
        }
      "
    />
    <FileManagerOverlays
      :menu="menu"
      :menu-items="menuItems"
      :local-menu="localMenu"
      :local-menu-items="localMenuItems"
      :mkdir-target="mkdirTarget"
      :new-file-target="newFileTarget"
      :local-create-kind="localCreateKind"
      :batch-confirm="batchConfirm"
      :batch-running="batchRunning"
      :local-rename-target="localRenameTarget"
      :local-delete-target="localDeleteTarget"
      :chmod-target="chmodTarget"
      :rename-target="renameTarget"
      :delete-target="deleteTarget"
      @close-menu="menu = null"
      @close-local-menu="localMenu = null"
      @close-mkdir="mkdirTarget = null"
      @mkdir="confirmMkdir"
      @close-new-file="newFileTarget = null"
      @new-file="confirmNewFile"
      @close-local-create="localCreateKind = null"
      @local-create="confirmLocalCreate"
      @close-batch="batchConfirm = null"
      @batch="confirmBatch"
      @close-local-rename="localRenameTarget = null"
      @local-rename="confirmLocalRename"
      @close-local-delete="localDeleteTarget = null"
      @local-delete="confirmLocalDelete"
      @cancel-rename="renameTarget = null"
      @cancel-delete="deleteTarget = null"
      @close-chmod="chmodTarget = null"
      @chmod="confirmChmod"
      @rename="confirmRename"
      @delete="confirmDelete"
    />
  </div>
</template>
