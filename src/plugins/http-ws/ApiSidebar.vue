<script setup lang="ts">
/** SSH 风格侧栏：分组树、空白与分组右键菜单，整行统一选中和悬停。 */
import { computed, ref } from 'vue'
import { UiDropdownMenu, UiIcon, UiTree, UiSortableList, UiSearchInput, UiTooltip } from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import type { ApiKind, ApiRecord } from './contracts'
import type { UiTreeItem, UiCollectionMove } from '@/core/ui'
import { apiTreeItems, apiTreeDestination } from './apiTree'
import { methodTextClass } from './useHttp'
import NewRequestMenu from './NewRequestMenu.vue'

const props = withDefaults(
  defineProps<{
    apis: ApiRecord[]
    groups: string[]
    activeId: number | null
    groupIdentities?: Record<string, string>
    busy?: boolean
    loading: boolean
    error: string
  }>(),
  { groupIdentities: () => ({}) }
)
const emit = defineEmits<{
  select: [record: ApiRecord]
  rename: [record: ApiRecord]
  requestMove: [record: ApiRecord]
  requestMoveGroup: [path: string]
  delete: [record: ApiRecord]
  new: [kind: ApiKind, group: string]
  newGroup: [parent: string]
  retry: []
  move: [id: number, group: string]
  moveGroup: [path: string, parent: string]
  treeMove: [move: UiCollectionMove]
}>()
const search = ref(''),
  collapsed = ref(new Set<string>())
const menu = ref<{ x: number; y: number; parent: string; api?: ApiRecord; group: boolean } | null>(
  null
)
const apiOf = (id: string) => props.apis.find((api) => `api:${api.id}` === id)
const rows = computed<UiTreeItem[]>(() => {
  const query = search.value.trim().toLowerCase()
  if (query)
    return props.apis
      .filter((api) =>
        `${api.name} ${api.url} ${api.groupName} ${api.type} ${api.method}`
          .toLowerCase()
          .includes(query)
      )
      .map((api) => ({ id: `api:${api.id}`, label: api.name, kind: 'api', depth: 0 }))
  return apiTreeItems(props.groups, props.apis, collapsed.value, props.groupIdentities)
})
function canDrop(move: UiCollectionMove) {
  return apiTreeDestination(move, props.apis, props.groupIdentities) !== null
}
function select(item: UiTreeItem) {
  const api = apiOf(item.id)
  if (api) emit('select', api)
}
function nodeMenu(item: UiTreeItem, event: MouseEvent) {
  const api = apiOf(item.id)
  const path =
    Object.entries(props.groupIdentities).find(([, key]) => key === item.id.slice(6))?.[0] ??
    item.id.slice(6)
  openMenu(event, api?.groupName ?? path, item.kind === 'group', api)
}
function toggle(path: string) {
  const next = new Set(collapsed.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  collapsed.value = next
}
function openMenu(event: MouseEvent, parent = '', group = false, api?: ApiRecord) {
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
      { label: '重命名', onClick: () => emit('rename', api) },
      { label: '移动到…', onClick: () => emit('requestMove', api) },
      { label: '删除接口', danger: true, onClick: () => emit('delete', api) },
    ]
  }
  return [
    ...(target.group && target.parent
      ? [{ label: '移动到…', onClick: () => emit('requestMoveGroup', target.parent) }]
      : []),
    {
      label: target.group && target.parent ? '创建子分组' : '添加分组',
      onClick: () => {
        collapsed.value.delete(props.groupIdentities[target.parent] ?? target.parent)
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
        collapsed.value.delete(props.groupIdentities[target.parent] ?? target.parent)
        emit('new', item.kind, target.parent)
      },
    })),
  ]
})
</script>
<template>
  <aside
    class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark"
    @contextmenu="blankMenu"
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
    <component
      :is="search.trim() ? UiSortableList : UiTree"
      :items="rows"
      :model-value="activeId === null ? '' : `api:${activeId}`"
      :row-height="28"
      draggable
      :filtered="!!search.trim()"
      :busy="busy"
      :loading="loading"
      :error="error"
      :can-drop="canDrop"
      :empty-text="search.trim() ? '没有匹配的接口' : '右键添加分组，或点击 + 新建接口。'"
      label="接口目录"
      @retry="emit('retry')"
      @select="select"
      @toggle="toggle($event.id.slice(6))"
      @contextmenu="nodeMenu"
      @blank-contextmenu="blankMenu"
      @move="emit('treeMove', $event)"
    >
      <template #icon="{ item }">
        <span
          v-if="apiOf(item.id)"
          class="shrink-0 font-mono text-caption font-semibold"
          :class="methodTextClass(apiOf(item.id)!.method, apiOf(item.id)!.type)"
          >{{
            apiOf(item.id)!.type === 'http'
              ? apiOf(item.id)!.method
              : apiOf(item.id)!.type.toUpperCase()
          }}</span
        >
        <UiIcon v-else name="folder" :size="13" class="text-text-muted dark:text-text-muted-dark" />
      </template>
      <template #label="{ item }"
        ><UiTooltip :content="apiOf(item.id)?.url || item.label"
          ><span class="block truncate font-medium">{{ item.label }}</span></UiTooltip
        ></template
      >
      <template #suffix="{ item }">
        <UiDropdownMenu
          v-if="apiOf(item.id)"
          size="xs"
          :trigger-label="`管理 ${item.label}`"
          :items="[
            { value: 'rename', label: '重命名' },
            { value: 'move', label: '移动到…' },
            { value: 'delete', label: '删除接口', danger: true },
          ]"
          @select="
            $event === 'rename'
              ? emit('rename', apiOf(item.id)!)
              : $event === 'move'
                ? emit('requestMove', apiOf(item.id)!)
                : emit('delete', apiOf(item.id)!)
          "
        />
      </template>
    </component>
    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      size="sm"
      @close="menu = null"
    />
  </aside>
</template>
