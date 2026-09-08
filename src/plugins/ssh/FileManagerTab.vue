<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, RemoteFile } from './contracts'
import { useRemoteFileOps } from './useRemoteFileOps'
import { useUiStore } from '@/stores/ui'
import { UiButton, UiIconButton } from '@/core/ui'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import InputDialog from '@/core/ui/InputDialog.vue'
import FileBrowser from './FileBrowser.vue'
import FileManagerDialogs from './FileManagerDialogs.vue'
import { ipc } from './ipc'
import { useFileContextMenu } from './useFileContextMenu'
import { useSftpTransfers } from './useSftpTransfers'
import LocalBrowser from './LocalBrowser.vue'

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

/* ── 本地侧（双栏右栏）：目录浏览 + 选中上传 ── */
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const localBrowser = ref<InstanceType<typeof LocalBrowser> | null>(null)
const localSelected = ref<RemoteFile | null>(null)

/** 上传本地选中项到远端当前目录 */
async function uploadLocalSelection() {
  const file = localSelected.value
  if (!file) {
    ui.toast('请先在本地列表中选择文件')
    return
  }
  if (file.isDir) {
    // 目录走既有上传入口（对话框选择语义不同，这里直接传目录路径）
    await uploadLocalPaths([file.path], currentPath.value)
    return
  }
  await uploadLocalPaths([file.path], currentPath.value)
}

/** 新建远程目录（右键空白处菜单） */
const mkdirTarget = ref<string | null>(null)
async function confirmMkdir(name: string) {
  const dir = mkdirTarget.value
  mkdirTarget.value = null
  const connectionId = props.connection?.sessionId
  if (!dir || !connectionId || !name.trim()) return
  const path = dir.endsWith('/') ? `${dir}${name.trim()}` : `${dir}/${name.trim()}`
  try {
    const r = await ipc.sshFileMkdir(connectionId, path)
    if (r.ok) {
      ui.toast(`已创建目录 ${name.trim()}`)
      void refreshCurrent()
    } else {
      ui.toast(`创建目录失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`创建目录失败：${e}`)
  }
}

const activeTransfers = computed(() =>
  [...transfers.value.values()].filter((t) => !t.done || t.error)
)

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
  await openFileGuarded(file)
}

const { menu, menuItems, openMenu } = useFileContextMenu({
  mkdir: (dir: string) => (mkdirTarget.value = dir === '/' ? currentPath.value : dir),
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
    mkdirTarget.value = null
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
    <div class="flex min-h-0 flex-1">
      <FileBrowser
        class="min-w-0 flex-1"
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

      <!-- 中列传输按钮 -->
      <div
        class="flex w-[42px] shrink-0 flex-col items-center justify-center gap-[8px] border-l border-border dark:border-border-dark"
      >
        <UiIconButton
          label="上传到远端"
          title="把左侧选中的本地文件/目录上传到远端当前目录"
          @click="uploadLocalSelection"
        >
          →
        </UiIconButton>
      </div>

      <!-- 右栏：本地目录 -->
      <LocalBrowser
        ref="localBrowser"
        class="w-[280px] shrink-0"
        :initial-path="settings.settings.defaultDownloadDirectory || 'C:/'"
        @select="localSelected = $event"
        @error="(m: string) => ui.toast(m)"
      />
    </div>

    <!-- 传输队列 -->
    <div
      v-if="activeTransfers.length"
      class="flex max-h-[110px] shrink-0 flex-col gap-[4px] overflow-y-auto border-t border-border px-[12px] py-[6px] dark:border-border-dark"
    >
      <div
        v-for="item in activeTransfers"
        :key="item.id"
        class="flex items-center gap-[8px] text-caption"
      >
        <span
          class="shrink-0 rounded-full bg-neutral px-[7px] py-[1px] font-medium text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark"
        >
          {{ item.kind === 'upload' ? '上传' : '下载' }}
        </span>
        <span class="min-w-0 flex-1 truncate text-secondary dark:text-secondary-dark">{{
          item.label
        }}</span>
        <span v-if="item.total > 0" class="shrink-0 font-mono text-text-muted">
          {{ Math.min(100, Math.round((item.transferred / item.total) * 100)) }}%
        </span>
        <span v-if="item.error" class="shrink-0 text-danger-strong dark:text-danger-dark">{{
          item.error
        }}</span>
        <UiButton
          v-if="!item.done"
          variant="ghost"
          size="xs"
          class="!h-auto !px-[6px] !py-[1px] text-caption text-danger-strong dark:text-danger-dark"
          title="取消传输"
          @click="cancelTransfer(item.id)"
        >
          取消
        </UiButton>
      </div>
    </div>

    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
    <InputDialog
      :open="mkdirTarget !== null"
      title="新建目录"
      label="目录名"
      confirm-label="创建"
      @close="mkdirTarget = null"
      @confirm="confirmMkdir"
    />
    <FileManagerDialogs
      :editing="editing"
      :saving="savingEdit"
      :rename-target="renameTarget"
      :delete-target="deleteTarget"
      @cancel-edit="editing = null"
      @cancel-rename="renameTarget = null"
      @cancel-delete="deleteTarget = null"
      @save="(content: string, force?: boolean) => onSave(content, force)"
      @rename="confirmRename"
      @delete="confirmDelete"
    />
  </div>
</template>
