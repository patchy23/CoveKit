<script setup lang="ts">
/**
 * SSH 服务器列表（分组版）：连接按分组归组，拖拽入组；「未分组」虚拟组固定沉底。
 * 连接状态由右侧连接页签持有，列表不展示连接状态。
 * 搜索时退化为平铺列表（不带分组壳）。
 */
import { computed, ref } from 'vue'
import type { ServerProfile } from '../contracts'
import type { ServerGroup } from './useServerGroups'
import type { UiTreeItem, UiCollectionMove } from '@/core/ui'
import { serverTreeItems, serverTreeDestination } from './serverTree'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiTooltip,
  UiTree,
  UiSortableList,
  UiModal,
  UiSearchInput,
} from '@/core/ui'

const props = defineProps<{
  busy?: boolean
  profiles: ServerProfile[]
  groups: ServerGroup[]
  /** 展开的分组 id 集合（null 键 = 未分组虚拟组） */
  expandedIds: Set<string>
  searchKeyword: string
}>()

const emit = defineEmits<{
  (event: 'update:searchKeyword', value: string): void
  (event: 'openConnection', profileId: string): void
  (event: 'add', groupId?: string): void
  (event: 'knownHosts'): void
  (event: 'edit', profile: ServerProfile): void
  (event: 'deleteRequest', profile: ServerProfile): void
  (event: 'moveToGroup', profileId: string, groupId: string | null): void
  (event: 'toggleGroup', groupKey: string): void
  (event: 'createGroup', name: string): void
  (event: 'renameGroup', groupId: string, name: string): void
  (event: 'deleteGroup', groupId: string): void
  (event: 'treeMove', move: UiCollectionMove): void
}>()

const searching = computed(() => props.searchKeyword.trim().length > 0)

/** 组内连接沿用已保存的顺序。 */
function profilesOf(groupId: string | null): ServerProfile[] {
  return props.profiles.filter((p) => (p.groupId ?? null) === groupId)
}

const selected = ref('')
const rows = computed(() =>
  searching.value
    ? props.profiles.map((profile) => ({
        id: `profile:${profile.id}`,
        label: profile.name,
        depth: 0,
        kind: 'profile',
        showIcon: false,
      }))
    : serverTreeItems(props.groups, props.profiles, props.expandedIds)
)
function canDrop(move: UiCollectionMove) {
  return serverTreeDestination(move, props.profiles) !== null
}
function nodeMenu(item: UiTreeItem, event: MouseEvent) {
  const id = item.id.slice(item.id.indexOf(':') + 1)
  if (item.kind === 'group') {
    const group = props.groups.find((group) => group.id === id)
    if (group) openGroupMenu(event, group)
    else {
      event.preventDefault()
      menu.value = { kind: 'blank', x: event.clientX, y: event.clientY }
    }
  } else {
    const profile = props.profiles.find((profile) => profile.id === id)
    if (profile) openProfileMenu(event, profile)
  }
}
function nodeTitle(item: UiTreeItem) {
  const profile = props.profiles.find((profile) => `profile:${profile.id}` === item.id)
  return profile
    ? `${profile.username}@${profile.host}:${profile.port}（双击新建连接）`
    : item.label
}

/* ── 右键菜单 ── */
const menu = ref<
  | { kind: 'profile'; profile: ServerProfile; x: number; y: number }
  | { kind: 'group'; group: ServerGroup; x: number; y: number }
  | { kind: 'blank'; x: number; y: number }
  | null
>(null)

function openBlankMenu(event: MouseEvent) {
  if (
    event.target instanceof Element &&
    event.target.closest('input, textarea, button, [role="combobox"]')
  )
    return
  event.preventDefault()
  menu.value = { kind: 'blank', x: event.clientX, y: event.clientY }
}

function openProfileMenu(event: MouseEvent, profile: ServerProfile) {
  event.preventDefault()
  event.stopPropagation()
  menu.value = {
    kind: 'profile',
    profile,
    x: event.clientX,
    y: event.clientY,
  }
}

function openGroupMenu(event: MouseEvent, group: ServerGroup) {
  event.preventDefault()
  event.stopPropagation()
  menu.value = {
    kind: 'group',
    group,
    x: event.clientX,
    y: event.clientY,
  }
}

const menuItems = computed<ContextMenuItem[]>(() => {
  if (!menu.value) return []
  if (menu.value.kind === 'blank')
    return [
      { label: '添加服务器', onClick: () => emit('add') },
      { label: '新建分组', onClick: openCreate },
    ]
  if (menu.value.kind === 'profile') {
    const profile = menu.value.profile
    return [
      { label: '新建连接', onClick: () => emit('openConnection', profile.id) },
      { label: '编辑', onClick: () => emit('edit', profile) },
      { label: '', separator: true },
      { label: '删除', danger: true, onClick: () => emit('deleteRequest', profile) },
    ]
  }
  const group = menu.value.group
  return [
    { label: '添加服务器', onClick: () => emit('add', group.id) },
    { label: '', separator: true },
    { label: '重命名分组', onClick: () => openRename(group) },
    { label: '', separator: true },
    { label: '删除分组', danger: true, onClick: () => requestDeleteGroup(group) },
  ]
})

/* ── 新建/重命名分组弹窗 ── */
const groupDialog = ref<{ mode: 'create' } | { mode: 'rename'; group: ServerGroup } | null>(null)
const groupNameInput = ref('')

function openRename(group: ServerGroup) {
  groupDialog.value = { mode: 'rename', group }
  groupNameInput.value = group.name
}

/** 打开新建分组弹窗（清空上次输入） */
function openCreate() {
  groupDialog.value = { mode: 'create' }
  groupNameInput.value = ''
}

function submitGroupDialog() {
  const name = groupNameInput.value.trim()
  if (!name || !groupDialog.value) return
  if (groupDialog.value.mode === 'create') emit('createGroup', name)
  else emit('renameGroup', groupDialog.value.group.id, name)
  groupDialog.value = null
  groupNameInput.value = ''
}

/* ── 删除分组确认 ── */
const deleteGroupTarget = ref<ServerGroup | null>(null)

function requestDeleteGroup(group: ServerGroup) {
  deleteGroupTarget.value = group
}

function confirmDeleteGroup() {
  if (deleteGroupTarget.value) emit('deleteGroup', deleteGroupTarget.value.id)
  deleteGroupTarget.value = null
}
</script>

<template>
  <div
    class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark"
    @contextmenu="openBlankMenu"
  >
    <div class="shrink-0 px-[12px] py-[10px]">
      <UiSearchInput
        :model-value="searchKeyword"
        size="sm"
        class="min-w-0"
        placeholder="搜索服务器..."
        @update:model-value="emit('update:searchKeyword', $event)"
      >
        <template #actions>
          <UiIconButton label="添加服务器" size="xs" @click="emit('add')">
            <UiIcon name="plus" :size="14" />
          </UiIconButton>
        </template>
      </UiSearchInput>
    </div>

    <component
      :is="searching ? UiSortableList : UiTree"
      v-model="selected"
      :items="rows"
      :row-height="28"
      draggable
      :filtered="searching"
      :busy="busy"
      :can-drop="canDrop"
      label="服务器目录"
      :empty-text="searching ? '没有匹配的服务器' : '暂无服务器'"
      @toggle="emit('toggleGroup', $event.id.slice(6))"
      @open="emit('openConnection', $event.id.slice(8))"
      @contextmenu="nodeMenu"
      @blank-contextmenu="openBlankMenu"
      @move="emit('treeMove', $event)"
    >
      <template #icon="{ item }"
        ><UiIcon
          v-if="item.kind === 'group'"
          name="folder"
          :size="13"
          class="text-text-muted dark:text-text-muted-dark"
      /></template>
      <template #label="{ item }"
        ><UiTooltip :content="nodeTitle(item)"
          ><span class="block truncate font-medium">{{ item.label }}</span></UiTooltip
        ></template
      >
    </component>

    <!-- 底部入口常驻，搜索或列表铺满时仍可管理指纹。 -->
    <div class="shrink-0 border-t border-border px-[12px] py-[8px] dark:border-border-dark">
      <UiButton variant="ghost" size="sm" block @click="emit('knownHosts')">主机指纹管理</UiButton>
    </div>

    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />

    <!-- 新建/重命名分组 -->
    <UiModal
      :open="groupDialog !== null"
      :title="groupDialog?.mode === 'rename' ? '重命名分组' : '新建分组'"
      width="min(360px, 92vw)"
      @close="groupDialog = null"
    >
      <UiInput
        v-model="groupNameInput"
        placeholder="分组名称，如：生产环境"
        @keyup.enter="submitGroupDialog"
      />
      <template #footer>
        <UiButton variant="ghost" @click="groupDialog = null">取消</UiButton>
        <UiButton variant="primary" :disabled="!groupNameInput.trim()" @click="submitGroupDialog"
          >确定</UiButton
        >
      </template>
    </UiModal>

    <!-- 删除分组确认（不删连接，组内连接移回未分组） -->
    <ConfirmDialog
      :open="deleteGroupTarget !== null"
      title="删除分组"
      :message="`删除分组「${deleteGroupTarget?.name}」？组内 ${deleteGroupTarget ? profilesOf(deleteGroupTarget.id).length : 0} 个连接将移回未分组，连接配置本身不会被删除。`"
      confirm-label="删除"
      danger
      @confirm="confirmDeleteGroup"
      @close="deleteGroupTarget = null"
    />
  </div>
</template>
