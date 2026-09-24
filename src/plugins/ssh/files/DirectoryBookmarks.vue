<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  UiButton,
  UiTooltip,
  UiPopover,
  UiSearchInput,
  UiSortableList,
  UiIconButton,
  UiIcon,
  type UiCollectionMove,
} from '@/core/ui'
import { ipc } from '../ipc'
import type { RemoteFile, SshBookmark } from '../contracts'
const props = defineProps<{ profileId?: string; path: string }>()
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ navigate: [path: string] }>()
const query = ref(''),
  error = ref(''),
  busy = ref(false)
const bookmarks = ref<SshBookmark[]>([])
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
  <UiPopover v-model:open="open" label="目录书签" width="320px" side="top">
    <template #trigger><UiButton size="sm" variant="ghost">书签</UiButton></template>
    <div class="flex gap-sm border-b border-border p-sm dark:border-border-dark">
      <UiSearchInput
        v-model="query"
        size="sm"
        class="min-w-0 flex-1"
        placeholder="搜索名称或路径"
      /><UiButton size="sm" :disabled="busy || !profileId" @click="add()">收藏当前目录</UiButton>
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
    >
      <template #row="{ item }">
        <UiTooltip :content="item.description"
          ><div class="min-w-0 flex-1">
            <span class="block truncate text-body-sm">{{ item.label }}</span>
            <span class="block truncate text-caption text-secondary dark:text-secondary-dark">{{
              item.description
            }}</span>
          </div></UiTooltip
        >
      </template>
      <template #suffix="{ item }">
        <UiIconButton
          size="xs"
          label="删除书签"
          :disabled="busy"
          @pointerdown.stop
          @click.stop="mutate(() => ipc.sshBookmarkDelete(item.id))"
        >
          <UiIcon name="x" :size="12" />
        </UiIconButton>
      </template>
    </UiSortableList>
  </UiPopover>
</template>
