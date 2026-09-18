<script setup lang="ts">
/** 沿用 SSH 侧栏：搜索栏内新建、紧凑分组、搜索时平铺结果。 */
import { computed, ref } from 'vue'
import { UiButton, UiDropdownMenu, UiIcon, UiListRow, UiScrollArea, UiSearchInput } from '@/core/ui'
import type { ApiKind, ApiRecord } from './contracts'
import NewRequestMenu from './NewRequestMenu.vue'
const props = defineProps<{
  apis: ApiRecord[]
  activeId: number | null
  loading: boolean
  error: string
}>()
const emit = defineEmits<{
  select: [record: ApiRecord]
  rename: [record: ApiRecord]
  delete: [record: ApiRecord]
  new: [kind: ApiKind]
  retry: []
}>()
const search = ref('')
const collapsed = ref(new Set<string>())
const groups = computed(() => {
  const query = search.value.trim().toLowerCase()
  const list = props.apis.filter((a) =>
    `${a.name} ${a.url} ${a.groupName} ${a.type} ${a.method}`.toLowerCase().includes(query)
  )
  if (query) return [{ name: '', items: list }]
  const result = new Map<string, ApiRecord[]>()
  for (const a of list) {
    const group = a.groupName || ''
    result.set(group, [...(result.get(group) || []), a])
  }
  return [...result]
    .sort(([a], [b]) => (!a ? 1 : !b ? -1 : a.localeCompare(b, 'zh-CN')))
    .map(([name, items]) => ({ name, items }))
})
function toggle(name: string) {
  const next = new Set(collapsed.value)
  if (next.has(name)) next.delete(name)
  else next.add(name)
  collapsed.value = next
}
</script>
<template>
  <aside class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <div class="shrink-0 px-[12px] py-[10px]">
      <UiSearchInput
        v-model="search"
        size="sm"
        placeholder="搜索接口…"
        aria-label="搜索接口名称、地址或协议"
      >
        <template #actions><NewRequestMenu compact @create="emit('new', $event)" /></template>
      </UiSearchInput>
    </div>
    <div
      v-if="error"
      role="alert"
      class="px-[12px] text-body-sm text-tertiary-strong dark:text-tertiary-dark"
    >
      {{ error }}<UiButton size="xs" variant="ghost" @click="emit('retry')">重试</UiButton>
    </div>
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1 px-[6px] pb-[8px]">
        <div v-for="group in groups" :key="group.name">
          <UiButton
            v-if="!search.trim()"
            size="sm"
            variant="ghost"
            block
            class="!justify-start !gap-[4px] !px-[6px]"
            :aria-expanded="!collapsed.has(group.name)"
            @click="toggle(group.name)"
          >
            <UiIcon
              name="chevron-right"
              :size="12"
              class="shrink-0 transition-transform"
              :class="{ 'rotate-90': !collapsed.has(group.name) }"
            />
            <span class="min-w-0 flex-1 truncate text-left font-semibold">{{
              group.name || '未分组'
            }}</span
            ><span class="text-caption text-text-muted dark:text-text-muted-dark">{{
              group.items.length
            }}</span>
          </UiButton>
          <div v-show="search.trim() || !collapsed.has(group.name)">
            <UiListRow
              v-for="api in group.items"
              :key="api.id"
              size="sm"
              :active="api.id === activeId"
              :indent="search.trim() ? 0 : 6"
              class="group !py-0 !pr-0"
            >
              <UiButton
                variant="ghost"
                size="sm"
                class="min-w-0 flex-1 !justify-start !gap-[5px] !px-0"
                :title="`${api.name} · ${api.url}`"
                @click="emit('select', api)"
                ><span
                  class="shrink-0 font-mono text-caption text-secondary dark:text-secondary-dark"
                  >{{ api.type === 'http' ? api.method : api.type.toUpperCase() }}</span
                ><span class="truncate font-medium">{{ api.name }}</span></UiButton
              >
              <UiDropdownMenu
                size="xs"
                :trigger-label="`管理 ${api.name}`"
                :items="[
                  { value: 'rename', label: '重命名 / 移动分组' },
                  { value: 'delete', label: '删除接口', danger: true },
                ]"
                @select="$event === 'rename' ? emit('rename', api) : emit('delete', api)"
              />
            </UiListRow>
          </div>
        </div>
        <p
          v-if="!groups.some((g) => g.items.length)"
          class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          {{ loading ? '正在读取…' : search.trim() ? '没有匹配的接口' : '暂无接口，点击 + 新建。' }}
        </p>
      </div>
    </UiScrollArea>
  </aside>
</template>
