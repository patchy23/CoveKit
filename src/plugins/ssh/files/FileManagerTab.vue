<script setup lang="ts">
/** FileManagerTab · 文件页签装配层（导航 useRemoteDirectory + 多选/批量/菜单/本地侧各 composable 组合） */
import { UiButton, UiConfirmDialog, UiAlert } from '@/core/ui'
import RemoteEditorWorkspace from './RemoteEditorWorkspace.vue'
import { useRemoteEditor } from './useRemoteEditor'
import TransferPanel from './TransferPanel.vue'
import type { TransferItem } from './useFileTransfer'
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
}>()

const activeSessionId = computed(() =>
  props.connection?.status === 'connected' ? props.connection.sessionId : undefined
)
const transfersOpen = ref(false)
const retryTarget = ref<TransferItem>()
async function retryTransfer(item: TransferItem) {
  const connectionId = activeSessionId.value
  if (!connectionId) return
  try {
    await startTransfer(item.kind, item.localPath, item.remotePath, true)
  } catch (e) {
    ui.toast('重新传输失败：' + String(e))
  }
}
function locateTransfer(item: TransferItem) {
  if (item.kind === 'upload')
    void navigate(item.remotePath.slice(0, item.remotePath.lastIndexOf('/')) || '/')
  else
    void localBrowser.value?.navigate(
      item.localPath.replaceAll(String.fromCharCode(92), '/').split('/').slice(0, -1).join('/')
    )
}
const editor = useRemoteEditor(() => props.connection)
const emit = defineEmits<{ state: [state: { dirty: boolean; busy: boolean }] }>()
watch(
  () => [editor.dirty.value.length, editor.busy.value],
  () => emit('state', { dirty: !!editor.dirty.value.length, busy: editor.busy.value }),
  { immediate: true }
)
function showEditor() {
  editor.directory.value = currentPath.value
  editor.visible.value = true
}
const openFile = editor.openFile
const openFileGuarded = editor.openFile
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
  onRename: editor.renamed,
  refresh: () => refreshCurrent(),
})

const {
  startTransfer,
  clearEnded,
  dragActive,
  transferStatus,
  transfers,
  cancelTransfer,
  uploadLocalPaths,
  upload,
  download,
} = useSftpTransfers({
  connectionId: () => activeSessionId.value,
  active: () => props.active,
  currentPath,
  selectedFile,
  localDir: () => localBrowser.value?.currentPath ?? '',
  page: filePage,
  refresh: () => void refreshCurrent(),
})

/* 批量操作（确认弹窗 + 执行在 useFileBatchOps） */
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
  startTransfer,
  sessionId: () => activeSessionId.value,
  refreshRemote: () => void refreshCurrent(),
  refreshLocal: () => localBrowser.value?.refresh(),
  uploadLocalPaths,
  remoteDir: () => currentPath.value,
  localDir: () => localBrowser.value?.currentPath ?? '',
  localFiles: () => localBrowser.value?.files ?? [],
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
    onChmod: requestChmod,
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

function cancelAllTransfers() {
  for (const item of transfers.value.values()) if (!item.done) void cancelTransfer(item.id)
}
watch(
  () => activeSessionId.value,
  (sessionId) => {
    // 换连接/断开：清空全部交互态（菜单/弹窗/选择），目录回到根
    remoteSel.clear()
    localSel.clear()
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
      :local-initial-path="settings.settings.defaultDownloadDirectory || 'C:/'"
      :local-selected-paths="localSel.selectedPaths.value"
      :profile-id="profile?.id"
      :transfers="[...transfers.values()]"
      @navigate="navigate"
      @back="navigateBack"
      @up="navigateUp"
      @download="download()"
      @batch-download-selected="requestBatchDownload"
      @remote-row-click="(e: MouseEvent, f: RemoteFile) => remoteSel.onRowClick(e, f.path)"
      @open="onDoubleClick"
      @remote-context="openMenu"
      @sort="changeSort"
      @upload-local="uploadSelection(selectedLocalFiles)"
      @local-row-click="(e: MouseEvent, f: RemoteFile) => localSel.onRowClick(e, f.path)"
      @local-row-context="openLocalMenu"
      @local-blank-context="(e: MouseEvent) => openLocalMenu(e, null)"
      @local-error="(m: string) => ui.toast(m)"
      @toggle-transfers="transfersOpen = !transfersOpen"
      @cancel-transfer="cancelTransfer"
      @cancel-all-transfers="cancelAllTransfers"
      @drop-remote-to-local="(items: RemoteFile[]) => items.forEach((f) => void download(f))"
      @drop-local-to-remote="
        (items: RemoteFile[]) =>
          void uploadLocalPaths(
            items.map((f) => f.path),
            currentPath
          )
      "
      ><template #editor-action
        ><UiButton size="sm" variant="ghost" @click="showEditor"
          >编辑器 {{ editor.documents.value.length
          }}{{
            editor.dirty.value.length ? ' · 未保存 ' + editor.dirty.value.length : ''
          }}</UiButton
        ></template
      ></FileManagerPanes
    >
    <RemoteEditorWorkspace
      v-if="editor.visible.value || editor.documents.value.length > 0"
      :editor="editor"
      :connection="connection"
      :title="profile?.name ?? '服务器'"
    />
    <TransferPanel
      v-model:open="transfersOpen"
      :items="[...transfers.values()]"
      @cancel="cancelTransfer"
      @clear="clearEnded"
      @retry="retryTarget = $event"
      @locate="locateTransfer"
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
