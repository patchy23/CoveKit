<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  UiButton,
  UiPopover,
  UiSearchInput,
  UiSortableList,
  UiInputDialog,
  UiContextMenu,
  type UiCollectionMove,
  type UiContextMenuItem,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { ipc } from '../ipc'
import type { RemoteFile, SshBookmark } from '../contracts'
const props = defineProps<{ profileId?: string; path: string }>()
const emit = defineEmits<{ navigate: [path: string] }>()
const { copyText } = useCopy()
const open = ref(false),
  query = ref(''),
  error = ref(''),
  busy = ref(false)
const bookmarks = ref<SshBookmark[]>([])
const rename = ref<SshBookmark>()
const context = ref<{ x: number; y: number; items: UiContextMenuItem[] }>()
let version = 0
onUnmounted(() => version++)
async function load() {
  const id = props.profileId,
    current = ++version
  if (!id) {
    bookmarks.value = []
    return
  }
  try {
    const rows = await ipc.sshBookmarkList(id)
    if (current === version) {
      bookmarks.value = rows
      error.value = ''
    }
  } catch (e) {
    if (current === version) error.value = String(e)
  }
}
watch(
  () => props.profileId,
  () => {
    bookmarks.value = []
    open.value = false
    void load()
  },
  { immediate: true }
)
watch(open, (value) => {
  if (value) void load()
})
const items = computed(() =>
  bookmarks.value
    .filter((b) => (b.name + b.path).toLowerCase().includes(query.value.toLowerCase()))
    .map((b) => ({ id: b.id, label: b.name, description: b.path }))
)
async function mutate(action: () => Promise<unknown>) {
  if (busy.value) return
  busy.value = true
  try {
    await action()
    await load()
  } catch (e) {
    error.value = String(e)
  } finally {
    busy.value = false
  }
}
function add(file?: Pick<RemoteFile, 'name' | 'path'>) {
  const id = props.profileId
  if (!id) return
  const path = file?.path ?? props.path
  void mutate(() =>
    ipc.sshBookmarkAdd(id, file?.name ?? path.split('/').filter(Boolean).pop() ?? '/', path)
  )
}
function update(rows: SshBookmark[]) {
  const profileId = props.profileId
  if (profileId) void mutate(() => ipc.sshBookmarkUpdate({ profileId, bookmarks: rows }))
}
function move(change: UiCollectionMove) {
  const rows = [...bookmarks.value],
    source = rows.findIndex((b) => b.id === change.id)
  if (source < 0) return
  const [item] = rows.splice(source, 1)
  const target =
    change.targetId === null
      ? rows.length
      : rows.findIndex((b) => b.id === change.targetId) + (change.position === 'after' ? 1 : 0)
  rows.splice(Math.max(0, target), 0, item)
  update(rows)
}
function showMenu(item: { id: string }, event: MouseEvent) {
  const b = bookmarks.value.find((b) => b.id === item.id)
  if (!b) return
  context.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      {
        label: '重命名',
        onClick: () => {
          rename.value = b
        },
      },
      {
        label: '复制路径',
        onClick: () => {
          void copyText(b.path)
        },
      },
      {
        label: '移到最前',
        onClick: () => update([b, ...bookmarks.value.filter((v) => v.id !== b.id)]),
      },
      { label: '移到最后', onClick: () => move({ id: b.id, targetId: null, position: 'inside' }) },
      {
        label: '移除书签',
        onClick: () => {
          void mutate(() => ipc.sshBookmarkDelete(b.id))
        },
      },
    ],
  }
}
function selectBookmark(item: { id: string }) {
  const bookmark = bookmarks.value.find((b) => b.id === item.id)
  if (bookmark) {
    emit('navigate', bookmark.path)
    open.value = false
  }
}
defineExpose({ addBookmark: add })
</script>
<template>
  <UiPopover v-model:open="open" label="目录书签" width="360px">
    <template #trigger><UiButton size="sm" variant="ghost">目录书签</UiButton></template>
    <div class="flex gap-sm border-b border-border p-sm dark:border-border-dark">
      <UiSearchInput v-model="query" class="min-w-0 flex-1" placeholder="搜索名称或路径" /><UiButton
        size="sm"
        :disabled="busy || !profileId"
        @click="add()"
        >收藏当前目录</UiButton
      >
    </div>
    <UiSortableList
      class="max-h-[360px]"
      :items="items"
      :filtered="!!query"
      :busy="busy"
      :error="error"
      empty-text="暂无目录书签"
      @retry="load"
      @select="selectBookmark"
      @move="move"
      @contextmenu="showMenu"
    />
    <p class="px-sm py-xs text-caption text-secondary dark:text-secondary-dark">
      拖动调整顺序 · 右键重命名、复制路径或移除
    </p>
  </UiPopover>
  <UiContextMenu
    v-if="context"
    :x="context.x"
    :y="context.y"
    :items="context.items"
    @close="context = undefined"
  />
  <UiInputDialog
    :open="!!rename"
    title="重命名书签"
    label="名称"
    :initial-value="rename?.name"
    @close="rename = undefined"
    @confirm="
      (name) => {
        update(bookmarks.map((b) => (b.id === rename?.id ? { ...b, name } : b)))
        rename = undefined
      }
    "
  />
</template>
