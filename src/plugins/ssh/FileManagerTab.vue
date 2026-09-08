<script setup lang="ts">
/** FileManagerTab · 文件页签装配层（导航 useRemoteDirectory + 多选/批量/菜单/本地侧各 composable 组合） */
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, RemoteFile } from './contracts'
import { useRemoteFileOps } from './useRemoteFileOps'
import { useRemoteCreateOps } from './useRemoteCreateOps'
import { useRemoteDirectory } from './useRemoteDirectory'
import { useUiStore } from '@/stores/ui'
import { useSettingsStore } from '@/stores/settings'
import FileManagerPanes from './FileManagerPanes.vue'
import FileManagerOverlays from './FileManagerOverlays.vue'
import FileTransferStrip from './FileTransferStrip.vue'
import { useFileManagerMenus } from './useFileManagerMenus'
import { useFileSelection } from './useFileSelection'
import { useFileBatchOps } from './useFileBatchOps'
import { useLocalFileOps } from './useLocalFileOps'
import { useSftpTransfers } from './useSftpTransfers'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
  active?: boolean
}>()

const emit = defineEmits<{
  /** 请求打开 chmod 弹窗（P3） */
  (e: 'chmod', file: RemoteFile): void
  /** 请求添加书签（P4） */
  (e: 'bookmark', dir: RemoteFile): void
}>()

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
  sessionId: () => props.connection?.sessionId,
  onNavigate: () => remoteSel.clear(),
})

/* ── 多选模型（本地侧；远程在上面导航前已建） ── */
const localSel = useFileSelection(() => localBrowser.value?.files.map((f) => f.path) ?? [])

/** 远程选中文件列表（按列表顺序） */
const selectedRemoteFiles = computed(() =>
  sortedFiles.value.filter((f) => remoteSel.selectedPaths.value.has(f.path))
)
/** 本地选中文件列表 */
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

/* 编辑/删除/重命名操作在 useRemoteFileOps（toast/确认目标/编辑态统一管理） */
const {
  editing,
  savingEdit,
  deleteTarget,
  renameTarget,
  openFile,
  openFileGuarded,
  onSave,
  requestDelete,
  confirmDelete,
  requestRename,
  confirmRename,
} = useRemoteFileOps({
  sessionId: () => props.connection?.sessionId,
  selectedFile,
  refresh: () => refreshCurrent(),
})

const {
  dragActive,
  transferStatus,
  transfers,
  cancelTransfer,
  uploadLocalPaths,
  upload,
  download,
} = useSftpTransfers({
  connectionId: () => props.connection?.sessionId,
  active: () => props.active,
  currentPath,
  selectedFile,
  page: filePage,
  refresh: () => void refreshCurrent(),
})

/* ── 批量操作（确认弹窗 + 执行在 useFileBatchOps） ── */
const {
  batchConfirm,
  batchRunning,
  requestBatchDownload,
  requestBatchDeleteRemote,
  requestBatchUpload,
  requestBatchDeleteLocal,
  uploadSelection,
  confirmBatch,
} = useFileBatchOps({
  sessionId: () => props.connection?.sessionId,
  refreshRemote: () => void refreshCurrent(),
  refreshLocal: () => localBrowser.value?.refresh(),
  uploadLocalPaths,
  remoteDir: () => currentPath.value,
  localDir: () => localBrowser.value?.currentPath ?? '',
  localFiles: () => localBrowser.value?.files ?? [],
})

/* ── 本地文件操作（新建/重命名/删除在 useLocalFileOps） ── */
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

/* ── 远程新建文件/目录（useRemoteCreateOps） ── */
const { newFileTarget, mkdirTarget, confirmNewFile, confirmMkdir, requestNewFile, requestMkdir } =
  useRemoteCreateOps({
    sessionId: () => props.connection?.sessionId,
    currentDir: () => currentPath.value,
    refresh: () => void refreshCurrent(),
  })

const activeTransfers = computed(() =>
  [...transfers.value.values()].filter((t) => !t.done || t.error)
)

/** 双击：目录进入，符合文本规则的文件打开统一编辑弹窗。 */
async function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path)
    return
  }
  await openFileGuarded(file)
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
    onChmod: (file) => emit('chmod', file),
    onBookmark: (dir) => emit('bookmark', dir),
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
  () => props.connection?.sessionId,
  (sessionId) => {
    remoteSel.clear()
    localSel.clear()
    editing.value = null
    savingEdit.value = false
    renameTarget.value = null
    deleteTarget.value = null
    menu.value = null
    localMenu.value = null
    mkdirTarget.value = null
    newFileTarget.value = null
    reset(sessionId)
  },
  { immediate: true }
)
</script>

<template>
  <div ref="filePage" class="relative flex h-full min-h-0 flex-col">
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
      :local-initial-path="settings.settings.defaultDownloadDirectory || 'C:/'"
      :local-selected-paths="localSel.selectedPaths.value"
      @navigate="navigate"
      @back="navigateBack"
      @up="navigateUp"
      @upload="upload"
      @upload-directory="upload(currentPath, true)"
      @download="download()"
      @rename="requestRename()"
      @delete="requestDelete()"
      @remote-row-click="(e: MouseEvent, f: RemoteFile) => remoteSel.onRowClick(e, f.path)"
      @open="onDoubleClick"
      @remote-context="openMenu"
      @sort="changeSort"
      @upload-local="uploadSelection(selectedLocalFiles)"
      @local-row-click="(e: MouseEvent, f: RemoteFile) => localSel.onRowClick(e, f.path)"
      @local-row-context="openLocalMenu"
      @local-blank-context="(e: MouseEvent) => openLocalMenu(e, null)"
      @local-error="(m: string) => ui.toast(m)"
    />

    <FileTransferStrip :items="activeTransfers" @cancel="cancelTransfer" />

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
      :editing="editing"
      :saving-edit="savingEdit"
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
      @cancel-edit="editing = null"
      @cancel-rename="renameTarget = null"
      @cancel-delete="deleteTarget = null"
      @save="(c: string, f?: boolean) => onSave(c, f)"
      @rename="confirmRename"
      @delete="confirmDelete"
    />
  </div>
</template>
