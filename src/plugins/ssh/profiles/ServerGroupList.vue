<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * ServerGroupList · SSH 服务器分组态列表（分组行 + 组内连接 + 未分组虚拟组）
 * 从 ServerList 拆出（300 行红线）；拖拽命中标记 data-group-drop 与 useGroupDrag 约定。
 */
import type { ServerProfile } from '../contracts'
import { UNGROUPED_DROP_KEY, type ServerGroup } from './useServerGroups'
import { UiIcon, UiListRow } from '@/core/ui'

const props = defineProps<{
  profiles: ServerProfile[]
  groups: ServerGroup[]
  /** 展开的分组键集合（'__ungrouped__' = 未分组虚拟组） */
  expandedIds: Set<string>
  /** 拖拽命中的投放目标（高亮反馈） */
  dragOverId: string | null
}>()

const emit = defineEmits<{
  (event: 'openConnection', profileId: string): void
  (event: 'profileMenu', e: MouseEvent, profile: ServerProfile): void
  (event: 'groupMenu', e: MouseEvent, group: ServerGroup): void
  (event: 'toggleGroup', groupKey: string): void
  (event: 'rowPointerDown', e: PointerEvent, profile: ServerProfile): void
}>()

/** 组内连接（按名称排序，稳定展示） */
function profilesOf(groupId: string | null): ServerProfile[] {
  return props.profiles
    .filter((p) => (p.groupId ?? null) === groupId)
    .sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
}

function isExpanded(groupId: string | null): boolean {
  return props.expandedIds.has(groupId ?? UNGROUPED_DROP_KEY)
}
</script>

<template>
  <template v-for="group in groups" :key="group.id">
    <!-- 分组行：单击折叠/展开，右键管理，拖拽投放目标 -->
    <UiTooltip
      :content="`${group.name}（单击${isExpanded(group.id) ? '折叠' : '展开'}，拖拽连接到此入组）`"
    >
      <div
        class="flex h-[32px] cursor-default items-center gap-[4px] rounded-md px-[6px] transition-colors hover:bg-border dark:hover:bg-border-dark"
        :class="{
          'bg-tertiary-soft outline-1 outline-tertiary dark:bg-tertiary-soft-dark':
            dragOverId === group.id,
        }"
        :data-group-drop="group.id"
        @click="emit('toggleGroup', group.id)"
        @contextmenu="emit('groupMenu', $event, group)"
      >
        <UiIcon
          name="chevron-right"
          :size="12"
          class="shrink-0 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
          :class="{ 'rotate-90': isExpanded(group.id) }"
        />
        <span
          class="min-w-0 flex-1 truncate text-body-sm font-semibold text-secondary dark:text-secondary-dark"
          >{{ group.name }}</span
        >
        <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
          >({{ profilesOf(group.id).length }})</span
        >
      </div>
    </UiTooltip>
    <!-- 组内连接 -->
    <template v-if="isExpanded(group.id)">
      <UiTooltip
        v-for="profile in profilesOf(group.id)"
        :key="profile.id"
        :content="`${profile.username}@${profile.host}:${profile.port}（双击新建连接，拖拽移动分组）`"
      >
        <UiListRow
          size="sm"
          :indent="12"
          cursor="grab"
          @dblclick="emit('openConnection', profile.id)"
          @contextmenu="emit('profileMenu', $event, profile)"
          @pointerdown="emit('rowPointerDown', $event, profile)"
        >
          <span class="min-w-0 flex-1 truncate font-medium text-primary dark:text-primary-dark">
            {{ profile.name }}
          </span>
        </UiListRow>
      </UiTooltip>
      <p
        v-if="!profilesOf(group.id).length"
        class="ml-[12px] px-[8px] py-[4px] text-caption text-text-muted dark:text-text-muted-dark"
      >
        拖拽连接到此入组
      </p>
    </template>
  </template>

  <!-- 未分组（虚拟组，固定沉底，可折叠/接收拖出） -->
  <UiTooltip content="未分组（拖拽到此处移出分组）">
    <div
      class="mt-[4px] flex h-[32px] cursor-default items-center gap-[4px] rounded-md border-t border-border px-[6px] transition-colors hover:bg-border dark:border-border-dark dark:hover:bg-border-dark"
      :class="{
        'bg-tertiary-soft outline-1 outline-tertiary dark:bg-tertiary-soft-dark':
          dragOverId === UNGROUPED_DROP_KEY,
      }"
      data-group-drop=""
      @click="emit('toggleGroup', UNGROUPED_DROP_KEY)"
    >
      <UiIcon
        name="chevron-right"
        :size="12"
        class="shrink-0 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
        :class="{ 'rotate-90': isExpanded(null) }"
      />
      <span
        class="min-w-0 flex-1 truncate text-body-sm font-semibold text-secondary dark:text-secondary-dark"
        >未分组</span
      >
      <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
        >({{ profilesOf(null).length }})</span
      >
    </div>
  </UiTooltip>
  <template v-if="isExpanded(null)">
    <UiTooltip
      v-for="profile in profilesOf(null)"
      :key="profile.id"
      :content="`${profile.username}@${profile.host}:${profile.port}（双击新建连接，拖拽移动分组）`"
    >
      <UiListRow
        size="sm"
        :indent="12"
        cursor="grab"
        @dblclick="emit('openConnection', profile.id)"
        @contextmenu="emit('profileMenu', $event, profile)"
        @pointerdown="emit('rowPointerDown', $event, profile)"
      >
        <span class="min-w-0 flex-1 truncate font-medium text-primary dark:text-primary-dark">
          {{ profile.name }}
        </span>
      </UiListRow>
    </UiTooltip>
  </template>
</template>
