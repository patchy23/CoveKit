<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  UiTree,
  UiToolbar,
  UiInput,
  UiIconButton,
  UiIcon,
  UiAlert,
  UiContextMenu,
  UiInputDialog,
  type UiTreeItem,
  type UiContextMenuItem,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { ipc } from '../ipc'
import { normalizePathInput } from './pathInput'
import type { RemoteFile } from '../contracts'
const props = defineProps<{
  connectionId?: string
  connected: boolean
  initialPath: string
  busy?: boolean
}>()
const emit = defineEmits<{
  open: [file: RemoteFile]
  renamed: [oldPath: string, newPath: string]
}>()
const root = ref('/'),
  path = ref('/'),
  query = ref(''),
  error = ref('')
const cache = ref<Record<string, RemoteFile[]>>({}),
  expanded = ref(new Set<string>()),
  loading = ref(new Set<string>())
const menu = ref<{ x: number; y: number; items: UiContextMenuItem[] }>()
const action = ref<{ kind: 'file' | 'directory' | 'rename'; path: string; name: string }>()
const { copyText } = useCopy()
let version = 0
onUnmounted(() => version++)
async function load(dir: string) {
  const id = props.connectionId,
    seq = version
  if (!id || !props.connected || loading.value.has(dir)) return
  loading.value.add(dir)
  try {
    const r = await ipc.sshFileList(id, dir)
    if (seq !== version) return
    if (!r.ok) throw new Error(r.error ?? '读取失败')
    cache.value[dir] = r.files
    error.value = ''
  } catch (e) {
    if (seq === version) error.value = String(e)
  } finally {
    if (seq === version) loading.value.delete(dir)
  }
}
function changeRoot(value: string) {
  version++
  loading.value.clear()
  cache.value = {}
  expanded.value.clear()
  root.value = normalizePathInput(value, '/')
  path.value = root.value
  void load(root.value)
}
watch(
  () => [props.connectionId, props.connected],
  () => {
    version++
    cache.value = {}
    loading.value = new Set()
    if (props.connected) void load(root.value)
  }
)
watch(
  () => props.initialPath,
  (p) => {
    if (Object.keys(cache.value).length === 0) changeRoot(p)
  },
  { immediate: true }
)
const rows = computed(() => {
  const result: UiTreeItem[] = []
  function visit(dir: string, depth: number) {
    for (const file of [...(cache.value[dir] ?? [])].sort(
      (a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name)
    )) {
      if (
        query.value &&
        !file.name.toLowerCase().includes(query.value.toLowerCase()) &&
        !file.isDir
      )
        continue
      result.push({
        id: file.path,
        label: file.name,
        depth,
        expandable: file.isDir,
        expanded: expanded.value.has(file.path),
        loading: loading.value.has(file.path),
        draggable: false,
      })
      if (file.isDir && expanded.value.has(file.path)) visit(file.path, depth + 1)
    }
  }
  visit(root.value, 0)
  return result
})
function fileOf(id: string) {
  return Object.values(cache.value)
    .flat()
    .find((f) => f.path === id)
}
function toggle(item: UiTreeItem) {
  if (expanded.value.has(item.id)) expanded.value.delete(item.id)
  else {
    expanded.value.add(item.id)
    if (!cache.value[item.id]) void load(item.id)
  }
}
function open(item: UiTreeItem) {
  const file = fileOf(item.id)
  if (file && !file.isDir) emit('open', file)
}
function context(item: UiTreeItem, event: MouseEvent) {
  const file = fileOf(item.id)
  if (!file) return
  menu.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      {
        label: file.isDir ? '在此目录浏览' : '编辑',
        onClick: () => (file.isDir ? changeRoot(file.path) : emit('open', file)),
      },
      {
        label: '复制路径',
        onClick: () => {
          void copyText(file.path)
        },
      },
      {
        label: '重命名',
        disabled: props.busy,
        onClick: () => {
          action.value = { kind: 'rename', path: file.path, name: file.name }
        },
      },
    ],
  }
}
async function confirm(name: string) {
  const job = action.value,
    id = props.connectionId
  if (!job || !id || !props.connected || props.busy) return
  if (!name || name === '.' || name === '..' || name.includes('/')) {
    error.value = '请输入有效名称'
    return
  }
  try {
    const target =
      job.kind === 'rename'
        ? job.path.slice(0, job.path.lastIndexOf('/') + 1) + name
        : job.path.replace(/\/$/, '') + '/' + name
    const result =
      job.kind === 'rename'
        ? await ipc.sshFileRename(id, job.path, target)
        : job.kind === 'directory'
          ? await ipc.sshFileMkdir(id, target)
          : await ipc.sshFileCreate(id, target)
    if (!result.ok) throw new Error(result.error ?? '操作失败')
    if (job.kind === 'rename') emit('renamed', job.path, target)
    action.value = undefined
    expanded.value.clear()
    cache.value = {}
    void load(root.value)
  } catch (e) {
    error.value = String(e)
  }
}
</script>
<template>
  <div class="flex min-h-0 flex-col">
    <UiToolbar bordered
      ><UiIconButton
        label="上一级"
        size="xs"
        @click="changeRoot(root.slice(0, root.lastIndexOf('/')) || '/')"
        ><UiIcon name="arrow-up" :size="14" /></UiIconButton
      ><UiInput
        v-model="path"
        size="sm"
        class="min-w-0 flex-1"
        @keydown.enter="changeRoot(path)" /><UiIconButton label="刷新" size="xs" @click="load(root)"
        ><UiIcon name="refresh" :size="14" /></UiIconButton
    ></UiToolbar>
    <UiToolbar bordered
      ><UiInput v-model="query" size="sm" placeholder="筛选已加载的文件名" /><UiIconButton
        label="新建文件"
        size="xs"
        @click="action = { kind: 'file', path: root, name: '' }"
        ><UiIcon name="plus" :size="14" /></UiIconButton
      ><UiIconButton
        label="新建目录"
        size="xs"
        @click="action = { kind: 'directory', path: root, name: '' }"
        ><UiIcon name="folder" :size="14" /></UiIconButton
    ></UiToolbar>
    <UiAlert v-if="error" tone="danger" size="xs">{{ error }}</UiAlert>
    <UiTree
      :items="rows"
      :draggable="false"
      :loading="loading.has(root)"
      @toggle="toggle"
      @open="open"
      @contextmenu="context"
    />
    <UiContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menu.items"
      @close="menu = undefined"
    />
    <UiInputDialog
      :open="!!action"
      :title="
        action?.kind === 'rename'
          ? '重命名'
          : action?.kind === 'directory'
            ? '新建目录'
            : '新建文件'
      "
      label="名称"
      :initial-value="action?.name"
      @close="action = undefined"
      @confirm="confirm"
    />
  </div>
</template>
