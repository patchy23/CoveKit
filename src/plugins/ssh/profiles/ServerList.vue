<script setup lang="ts">
/**
 * SSH 服务器列表（分组版）：连接按分组归组，拖拽入组；「未分组」虚拟组固定沉底。
 * 连接状态由右侧连接页签持有，列表不展示连接状态。
 * 搜索时退化为平铺列表（不带分组壳）。
 */
import { computed, ref } from 'vue'
import type { ServerProfile } from '../contracts'
import { useGroupDrag, type ServerGroup } from './useServerGroups'
import ServerGroupList from './ServerGroupList.vue'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiListRow,
  UiModal,
  UiSearchInput,
} from '@/core/ui'

const props = defineProps<{
  profiles: ServerProfile[]
  groups: ServerGroup[]
  /** 展开的分组 id 集合（null 键 = 未分组虚拟组） */
  expandedIds: Set<string>
  searchKeyword: string
}>()

const emit = defineEmits<{
  (event: 'update:searchKeyword', value: string): void
  (event: 'openConnection', profileId: string): void
  (event: 'add'): void
  (event: 'knownHosts'): void
  (event: 'edit', profile: ServerProfile): void
  (event: 'deleteRequest', profile: ServerProfile): void
  (event: 'moveToGroup', profileId: string, groupId: string | null): void
  (event: 'toggleGroup', groupKey: string): void
  (event: 'createGroup', name: string): void
  (event: 'renameGroup', groupId: string, name: string): void
  (event: 'deleteGroup', groupId: string): void
}>()

/** 拖拽入组（逻辑在 useGroupDrag；命中上抛为 moveToGroup 事件） */
const { drag, dragOverId, onRowPointerDown } = useGroupDrag((profileId, groupId) =>
  emit('moveToGroup', profileId, groupId)
)

const searching = computed(() => props.searchKeyword.trim().length > 0)

/** 组内连接（按名称排序，稳定展示） */
function profilesOf(groupId: string | null): ServerProfile[] {
  return props.profiles
    .filter((p) => (p.groupId ?? null) === groupId)
    .sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
}

/* ── 右键菜单 ── */
const menu = ref<
  | { kind: 'profile'; profile: ServerProfile; x: number; y: number }
  | { kind: 'group'; group: ServerGroup; x: number; y: number }
  | null
>(null)

function openProfileMenu(event: MouseEvent, profile: ServerProfile) {
  event.preventDefault()
  menu.value = {
    kind: 'profile',
    profile,
    x: Math.min(event.clientX, window.innerWidth - 158),
    y: Math.min(event.clientY, window.innerHeight - 132),
  }
}

function openGroupMenu(event: MouseEvent, group: ServerGroup) {
  event.preventDefault()
  event.stopPropagation()
  menu.value = {
    kind: 'group',
    group,
    x: Math.min(event.clientX, window.innerWidth - 158),
    y: Math.min(event.clientY, window.innerHeight - 132),
  }
}

const menuItems = computed<ContextMenuItem[]>(() => {
  if (!menu.value) return []
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
    :class="{ 'select-none': drag?.active }"
  >
    <div class="shrink-0 px-[12px] py-[10px]">
      <!-- 搜索 + 添加同一行：搜索框弹性占满，右侧 + 号按钮 -->
      <div class="flex items-center gap-[6px]">
        <UiSearchInput
          :model-value="searchKeyword"
          size="sm"
          class="min-w-0 flex-1"
          placeholder="搜索服务器..."
          @update:model-value="emit('update:searchKeyword', $event)"
        />
        <UiIconButton label="添加服务器" size="sm" title="添加服务器" @click="emit('add')">
          <UiIcon name="plus" :size="14" />
        </UiIconButton>

        <UiIconButton
          label="已知主机"
          size="sm"
          title="已知主机（指纹管理）"
          @click="emit('knownHosts')"
        >
          <UiIcon name="eye" :size="14" />
        </UiIconButton>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <!-- 搜索态：平铺列表 -->
      <template v-if="searching">
        <UiListRow
          v-for="profile in profiles"
          :key="profile.id"
          size="sm"
          :title="`${profile.username}@${profile.host}:${profile.port}（双击新建连接）`"
          @dblclick="emit('openConnection', profile.id)"
          @contextmenu="openProfileMenu($event, profile)"
        >
          <span class="min-w-0 flex-1 truncate font-medium text-primary dark:text-primary-dark">
            {{ profile.name }}
          </span>
        </UiListRow>
      </template>

      <!-- 分组态（ServerGroupList 子组件承载分组行/组内行/未分组） -->
      <ServerGroupList
        v-else
        :profiles="profiles"
        :groups="groups"
        :expanded-ids="expandedIds"
        :drag-over-id="dragOverId"
        @open-connection="emit('openConnection', $event)"
        @profile-menu="openProfileMenu"
        @group-menu="openGroupMenu"
        @toggle-group="emit('toggleGroup', $event)"
        @row-pointer-down="onRowPointerDown"
      />

      <p
        v-if="!profiles.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无服务器<br />点击「+ 添加服务器」新建配置
      </p>
    </div>

    <!-- 底部：新建分组入口 -->
    <div class="shrink-0 border-t border-border px-[12px] py-[8px] dark:border-border-dark">
      <UiButton variant="ghost" size="sm" block @click="openCreate">+ 新建分组</UiButton>
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

    <!-- 拖拽中的浮动指示（跟随指针的迷你标签） -->
    <div
      v-if="drag?.active"
      class="pointer-events-none fixed z-[300] rounded-md border border-tertiary bg-surface px-[8px] py-[4px] text-body-sm font-medium text-tertiary-strong shadow-md dark:bg-surface-dark dark:text-tertiary-dark"
      :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 12}px` }"
    >
      {{ drag.name }}
    </div>
  </div>
</template>
