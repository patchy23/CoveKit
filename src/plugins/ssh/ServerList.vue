<script setup lang="ts">
/** SSH 服务器配置列表；连接由右侧连接页签持有，列表不展示连接状态。 */
import { computed, ref } from 'vue'
import type { ServerProfile } from './contracts'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import { UiButton, UiSearchInput } from '@/core/ui'

defineProps<{
  profiles: ServerProfile[]
  searchKeyword: string
}>()

const emit = defineEmits<{
  (event: 'update:searchKeyword', value: string): void
  (event: 'openConnection', profileId: string): void
  (event: 'add'): void
  (event: 'edit', profile: ServerProfile): void
  (event: 'deleteRequest', profile: ServerProfile): void
}>()

const menu = ref<{ profile: ServerProfile; x: number; y: number } | null>(null)

function openMenu(event: MouseEvent, profile: ServerProfile) {
  event.preventDefault()
  menu.value = {
    profile,
    x: Math.min(event.clientX, window.innerWidth - 158),
    y: Math.min(event.clientY, window.innerHeight - 132),
  }
}

const menuItems = computed<ContextMenuItem[]>(() => {
  if (!menu.value) return []
  const profile = menu.value.profile
  return [
    { label: '新建连接', onClick: () => emit('openConnection', profile.id) },
    { label: '编辑', onClick: () => emit('edit', profile) },
    { label: '', separator: true },
    { label: '删除', danger: true, onClick: () => emit('deleteRequest', profile) },
  ]
})
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
      <div
        v-for="profile in profiles"
        :key="profile.id"
        class="mb-[2px] flex cursor-default items-center rounded-md px-[8px] py-[8px] transition-colors hover:bg-border dark:hover:bg-border-dark"
        :title="`${profile.username}@${profile.host}:${profile.port}（双击新建连接）`"
        @dblclick="emit('openConnection', profile.id)"
        @contextmenu="openMenu($event, profile)"
      >
        <span
          class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
        >
          {{ profile.name }}
        </span>
      </div>

      <p
        v-if="!profiles.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无服务器<br />点击「+ 添加服务器」新建配置
      </p>
    </div>

    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
  </div>
</template>
