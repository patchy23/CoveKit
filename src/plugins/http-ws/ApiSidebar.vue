<script setup lang="ts">
/** SSH 风格侧栏：分组树、空白与分组右键菜单，整行统一选中和悬停。 */
import { computed, ref } from 'vue'
import { UiButton, UiDropdownMenu, UiIcon, UiListRow, UiScrollArea, UiSearchInput } from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import type { ApiKind, ApiRecord } from './contracts'
import { groupRows, type ApiSidebarRow } from './apiGroups'
import { methodTextClass } from './useHttp'
import NewRequestMenu from './NewRequestMenu.vue'
import { useApiDrag } from './useApiDrag'
const props = defineProps<{
  apis: ApiRecord[]
  groups: string[]
  activeId: number | null
  loading: boolean
  error: string
}>()
const emit = defineEmits<{
  select: [record: ApiRecord]
  rename: [record: ApiRecord]
  delete: [record: ApiRecord]
  new: [kind: ApiKind, group: string]
  newGroup: [parent: string]
  retry: []
  move: [id: number, group: string]
}>()
const search = ref(''),
  collapsed = ref(new Set<string>())
const root = ref<HTMLElement | null>(null)
const {
  drag,
  target: dropTarget,
  start: startDrag,
  captureClick,
  cancel: cancelDrag,
} = useApiDrag(root, (id, group) => {
  collapsed.value.delete(group)
  emit('move', id, group)
})
const menu = ref<{ x: number; y: number; parent: string; api?: ApiRecord; group: boolean } | null>(
  null
)
const rows = computed<ApiSidebarRow[]>(() => {
  const query = search.value.trim().toLowerCase()
  if (query)
    return props.apis
      .filter((api) =>
        `${api.name} ${api.url} ${api.groupName} ${api.type} ${api.method}`
          .toLowerCase()
          .includes(query)
      )
      .map((api) => ({ kind: 'api', api, depth: 0 }))
  return groupRows(props.groups, props.apis, collapsed.value)
})
function toggle(path: string) {
  const next = new Set(collapsed.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  collapsed.value = next
}
function openMenu(event: MouseEvent, parent = '', group = false, api?: ApiRecord) {
  cancelDrag()
  event.preventDefault()
  event.stopPropagation()
  menu.value = { x: event.clientX, y: event.clientY, parent, group, api }
}
function blankMenu(event: MouseEvent) {
  if (event.target instanceof Element && event.target.closest('input,button,[role="combobox"]'))
    return
  openMenu(event)
}
const menuItems = computed<ContextMenuItem[]>(() => {
  const target = menu.value
  if (!target) return []
  if (target.api) {
    const api = target.api
    return [
      { label: '打开接口', onClick: () => emit('select', api) },
      { label: '重命名 / 移动分组', onClick: () => emit('rename', api) },
      { label: '删除接口', danger: true, onClick: () => emit('delete', api) },
    ]
  }
  return [
    {
      label: target.group && target.parent ? '创建子分组' : '添加分组',
      onClick: () => {
        collapsed.value.delete(target.parent)
        emit('newGroup', target.parent)
      },
    },
    { label: '', separator: true },
    ...(
      [
        { kind: 'http', label: '新建 HTTP 接口' },
        { kind: 'sse', label: '新建 SSE 接口' },
        { kind: 'ws', label: '新建 WebSocket 接口' },
      ] as const
    ).map((item) => ({
      label: item.label,
      onClick: () => {
        collapsed.value.delete(target.parent)
        emit('new', item.kind, target.parent)
      },
    })),
  ]
})
</script>
<template>
  <aside
    ref="root"
    class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark"
    :class="{ 'select-none': drag?.active }"
    @contextmenu="blankMenu"
    @click.capture="captureClick"
  >
    <div class="shrink-0 px-[12px] py-[10px]">
      <UiSearchInput
        v-model="search"
        size="sm"
        placeholder="搜索接口…"
        aria-label="搜索接口名称、地址或协议"
        ><template #actions><NewRequestMenu compact @create="emit('new', $event, '')" /></template
      ></UiSearchInput>
    </div>
    <div
      v-if="error"
      role="alert"
      class="px-[12px] text-body-sm text-tertiary-strong dark:text-tertiary-dark"
    >
      {{ error }}<UiButton size="xs" variant="ghost" @click="emit('retry')">重试</UiButton>
    </div>
    <UiScrollArea as-child axis="vertical"
      ><div class="min-h-0 flex-1 px-[6px] pb-[8px]">
        <div
          v-for="row in rows"
          :key="row.kind === 'group' ? `group:${row.path}` : `api:${row.api.id}`"
          class="relative"
          :data-depth="row.depth"
          :style="{ paddingLeft: `${row.depth * 18}px` }"
        >
          <span
            v-for="level in row.depth"
            :key="level"
            aria-hidden="true"
            class="pointer-events-none absolute inset-y-0 border-l border-border-strong dark:border-border-strong-dark"
            :style="{ left: `${(level - 1) * 18 + 11}px` }"
          />
          <span
            v-if="row.depth"
            aria-hidden="true"
            class="pointer-events-none absolute top-1/2 w-[7px] border-t border-border-strong dark:border-border-strong-dark"
            :style="{ left: `${row.depth * 18 - 7}px` }"
          />
          <UiButton
            v-if="row.kind === 'group'"
            size="sm"
            variant="ghost"
            block
            class="!justify-start !gap-[4px] !pr-[6px]"
            :data-api-group-drop="row.path"
            :class="
              dropTarget === row.path
                ? '!bg-tertiary-soft ring-1 ring-tertiary-strong dark:!bg-tertiary-soft-dark dark:ring-tertiary-dark'
                : ''
            "
            :title="row.path || '未分组'"
            :aria-expanded="!collapsed.has(row.path)"
            @click="toggle(row.path)"
            @contextmenu="openMenu($event, row.path, true)"
          >
            <UiIcon
              name="chevron-right"
              :size="12"
              class="shrink-0 transition-transform"
              :class="{ 'rotate-90': !collapsed.has(row.path) }"
            /><UiIcon
              name="folder"
              :size="13"
              class="shrink-0 text-text-muted dark:text-text-muted-dark"
            /><span
              class="min-w-0 flex-1 truncate text-left"
              :class="row.depth === 0 ? 'font-semibold' : 'font-medium'"
              >{{ row.label }}</span
            ><span class="text-caption text-text-muted dark:text-text-muted-dark">{{
              row.count
            }}</span>
          </UiButton>
          <UiListRow
            v-else
            size="sm"
            :active="row.api.id === activeId"
            class="group !py-0 !pr-0"
            @contextmenu="openMenu($event, row.api.groupName, false, row.api)"
          >
            <UiButton
              variant="ghost"
              size="sm"
              class="min-w-0 flex-1 !justify-start !gap-[5px] !px-0 !bg-transparent hover:!bg-transparent dark:hover:!bg-transparent"
              :title="`${row.api.name} · ${row.api.url}`"
              @pointerdown="startDrag($event, row.api)"
              @click="emit('select', row.api)"
              ><span
                class="shrink-0 font-mono text-caption font-semibold"
                :class="methodTextClass(row.api.method, row.api.type)"
                >{{ row.api.type === 'http' ? row.api.method : row.api.type.toUpperCase() }}</span
              ><span class="truncate font-medium text-primary dark:text-primary-dark">{{
                row.api.name
              }}</span></UiButton
            >
            <UiDropdownMenu
              size="xs"
              :trigger-label="`管理 ${row.api.name}`"
              :items="[
                { value: 'rename', label: '重命名 / 移动分组' },
                { value: 'delete', label: '删除接口', danger: true },
              ]"
              @select="$event === 'rename' ? emit('rename', row.api) : emit('delete', row.api)"
            />
          </UiListRow>
        </div>
        <p
          v-if="!rows.length"
          class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          {{
            loading
              ? '正在读取…'
              : search.trim()
                ? '没有匹配的接口'
                : '右键添加分组，或点击 + 新建接口。'
          }}
        </p>
      </div></UiScrollArea
    >
    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      size="sm"
      @close="menu = null"
    />
    <Teleport to="body"
      ><div
        v-if="drag?.active"
        class="pointer-events-none fixed z-[300] max-w-[240px] truncate rounded-md border border-border bg-surface px-[8px] py-[4px] text-body-sm text-primary shadow-sm dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
        :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 12}px` }"
      >
        {{ drag.name }} ·
        {{ dropTarget === null ? '拖到分组以移动' : `移至 ${dropTarget || '未分组'}` }}
      </div></Teleport
    >
  </aside>
</template>
