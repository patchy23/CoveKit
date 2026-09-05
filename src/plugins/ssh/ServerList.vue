<script setup lang="ts">
/**
 * SSH 服务器列表（分组版）：连接按分组归组，拖拽入组；「未分组」虚拟组固定沉底。
 * 连接状态由右侧连接页签持有，列表不展示连接状态。
 * 搜索时退化为平铺列表（不带分组壳）。
 */
import { computed, ref } from 'vue'
import type { ServerProfile } from './contracts'
import type { ServerGroup } from './useServerGroups'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiButton, UiIcon, UiInput, UiModal, UiSearchInput } from '@/core/ui'

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
  (event: 'edit', profile: ServerProfile): void
  (event: 'deleteRequest', profile: ServerProfile): void
  (event: 'moveToGroup', profileId: string, groupId: string | null): void
  (event: 'toggleGroup', groupKey: string): void
  (event: 'createGroup', name: string): void
  (event: 'renameGroup', groupId: string, name: string): void
  (event: 'deleteGroup', groupId: string): void
}>()

/** 未分组虚拟组的展开键（固定沉底，可折叠/接收拖出） */
const UNGROUPED_KEY = '__ungrouped__'

const searching = computed(() => props.searchKeyword.trim().length > 0)

/** 组内连接（按名称排序，稳定展示） */
function profilesOf(groupId: string | null): ServerProfile[] {
  return props.profiles
    .filter((p) => (p.groupId ?? null) === groupId)
    .sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
}

function isExpanded(groupId: string | null): boolean {
  return props.expandedIds.has(groupId ?? UNGROUPED_KEY)
}

/* ── 拖拽入组（HTML5 原生 DnD，dataTransfer 带 profile id） ── */
const DND_TYPE = 'application/x-ssh-profile'
/** 当前悬停的投放目标（高亮反馈） */
const dragOverId = ref<string | null>(null)

function onDragStart(event: DragEvent, profile: ServerProfile) {
  event.dataTransfer?.setData(DND_TYPE, profile.id)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onDragOver(event: DragEvent, groupId: string | null) {
  if (!event.dataTransfer?.types.includes(DND_TYPE)) return
  event.preventDefault()
  event.dataTransfer.dropEffect = 'move'
  dragOverId.value = groupId ?? '__ungrouped__'
}

function onDrop(event: DragEvent, groupId: string | null) {
  event.preventDefault()
  dragOverId.value = null
  const profileId = event.dataTransfer?.getData(DND_TYPE)
  if (profileId) emit('moveToGroup', profileId, groupId)
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
  <div class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <div class="shrink-0 space-y-[8px] px-[12px] py-[10px]">
      <UiSearchInput
        :model-value="searchKeyword"
        size="sm"
        placeholder="搜索服务器..."
        @update:model-value="emit('update:searchKeyword', $event)"
      />
      <UiButton variant="secondary" size="sm" block @click="emit('add')">+ 添加服务器</UiButton>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <!-- 搜索态：平铺列表 -->
      <template v-if="searching">
        <div
          v-for="profile in profiles"
          :key="profile.id"
          class="mb-[2px] flex cursor-default items-center rounded-md px-[8px] py-[8px] transition-colors hover:bg-border dark:hover:bg-border-dark"
          :title="`${profile.username}@${profile.host}:${profile.port}（双击新建连接）`"
          @dblclick="emit('openConnection', profile.id)"
          @contextmenu="openProfileMenu($event, profile)"
        >
          <span
            class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
          >
            {{ profile.name }}
          </span>
        </div>
      </template>

      <!-- 分组态 -->
      <template v-else>
        <template v-for="group in groups" :key="group.id">
          <!-- 分组行：单击折叠/展开，右键管理，拖拽投放目标 -->
          <div
            class="flex h-[32px] cursor-default items-center gap-[4px] rounded-md px-[6px] transition-colors hover:bg-border dark:hover:bg-border-dark"
            :class="{
              'bg-tertiary-soft outline-1 outline-tertiary dark:bg-tertiary-soft-dark':
                dragOverId === group.id,
            }"
            :title="`${group.name}（单击${isExpanded(group.id) ? '折叠' : '展开'}，拖拽连接到此入组）`"
            @click="emit('toggleGroup', group.id)"
            @contextmenu="openGroupMenu($event, group)"
            @dragover="onDragOver($event, group.id)"
            @dragleave="dragOverId = null"
            @drop="onDrop($event, group.id)"
          >
            <UiIcon
              name="chevron-right"
              :size="12"
              class="shrink-0 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
              :class="{ 'rotate-90': isExpanded(group.id) }"
            />
            <span
              class="min-w-0 flex-1 truncate text-body-sm font-semibold text-primary dark:text-primary-dark"
              >{{ group.name }}</span
            >
            <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
              >({{ profilesOf(group.id).length }})</span
            >
          </div>
          <!-- 组内连接 -->
          <template v-if="isExpanded(group.id)">
            <div
              v-for="profile in profilesOf(group.id)"
              :key="profile.id"
              draggable="true"
              class="mb-[2px] ml-[12px] flex cursor-default items-center rounded-md px-[8px] py-[8px] transition-colors hover:bg-border dark:hover:bg-border-dark"
              :title="`${profile.username}@${profile.host}:${profile.port}（双击新建连接，拖拽移动分组）`"
              @dblclick="emit('openConnection', profile.id)"
              @contextmenu="openProfileMenu($event, profile)"
              @dragstart="onDragStart($event, profile)"
            >
              <span
                class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
              >
                {{ profile.name }}
              </span>
            </div>
            <p
              v-if="!profilesOf(group.id).length"
              class="ml-[12px] px-[8px] py-[4px] text-caption text-text-muted dark:text-text-muted-dark"
            >
              拖拽连接到此入组
            </p>
          </template>
        </template>

        <!-- 未分组（虚拟组，固定沉底，可折叠/接收拖出） -->
        <div
          class="mt-[4px] flex h-[32px] cursor-default items-center gap-[4px] rounded-md border-t border-border px-[6px] transition-colors hover:bg-border dark:border-border-dark dark:hover:bg-border-dark"
          :class="{
            'bg-tertiary-soft outline-1 outline-tertiary dark:bg-tertiary-soft-dark':
              dragOverId === UNGROUPED_KEY,
          }"
          title="未分组（拖拽到此处移出分组）"
          @click="emit('toggleGroup', UNGROUPED_KEY)"
          @dragover="onDragOver($event, null)"
          @dragleave="dragOverId = null"
          @drop="onDrop($event, null)"
        >
          <UiIcon
            name="chevron-right"
            :size="12"
            class="shrink-0 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
            :class="{ 'rotate-90': isExpanded(null) }"
          />
          <span
            class="min-w-0 flex-1 truncate text-body-sm font-semibold text-text-muted dark:text-text-muted-dark"
            >未分组</span
          >
          <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
            >({{ profilesOf(null).length }})</span
          >
        </div>
        <template v-if="isExpanded(null)">
          <div
            v-for="profile in profilesOf(null)"
            :key="profile.id"
            draggable="true"
            class="mb-[2px] ml-[12px] flex cursor-default items-center rounded-md px-[8px] py-[8px] transition-colors hover:bg-border dark:hover:bg-border-dark"
            :title="`${profile.username}@${profile.host}:${profile.port}（双击新建连接，拖拽移动分组）`"
            @dblclick="emit('openConnection', profile.id)"
            @contextmenu="openProfileMenu($event, profile)"
            @dragstart="onDragStart($event, profile)"
          >
            <span
              class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
            >
              {{ profile.name }}
            </span>
          </div>
        </template>
      </template>

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
  </div>
</template>
