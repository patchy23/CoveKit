<script setup lang="ts">
/** 可见节点与展开状态受控，兼容原有 UiTree 契约。 */
import CollectionSurface from './collection/CollectionSurface.vue'
import type { UiTreeItem, UiCollectionMove, UiDropGuard } from './collection/types'
withDefaults(
  defineProps<{
    items: UiTreeItem[]
    modelValue?: string
    rowHeight?: 22 | 24 | 28 | 32
    /** 固定单行节点的视口渲染，完整键盘导航仍覆盖全部 items。 */
    virtual?: boolean
    /** 调用方保证 row/label/suffix 插槽只需固定一行；与 virtual 一起显式启用。 */
    fixedRowSlots?: boolean
    draggable?: boolean
    dragHandle?: boolean
    disabled?: boolean
    busy?: boolean
    loading?: boolean
    error?: string
    filtered?: boolean
    emptyText?: string
    label?: string
    canDrop?: UiDropGuard
  }>(),
  {
    modelValue: '',
    fixedRowSlots: false,
    canDrop: undefined,
    error: '',
    rowHeight: 24,
    label: '树形菜单',
    emptyText: '暂无项目',
  }
)
export type { UiTreeItem } from './collection/types'
const emit = defineEmits<{
  'update:modelValue': [id: string]
  select: [item: UiTreeItem]
  open: [item: UiTreeItem]
  toggle: [item: UiTreeItem]
  contextmenu: [item: UiTreeItem, event: MouseEvent]
  blankContextmenu: [event: MouseEvent]
  move: [move: UiCollectionMove]
  retry: []
}>()
</script>
<template>
  <CollectionSurface
    v-bind="$props"
    tree
    @update:model-value="emit('update:modelValue', $event)"
    @toggle="emit('toggle', $event)"
    @select="emit('select', $event)"
    @open="emit('open', $event)"
    @contextmenu="(item, event) => emit('contextmenu', item, event)"
    @blank-contextmenu="emit('blankContextmenu', $event)"
    @move="emit('move', $event)"
    @retry="emit('retry')"
  >
    <template v-for="(_, name) in $slots" #[name]="scope"
      ><slot :name="name" v-bind="scope || {}"
    /></template>
  </CollectionSurface>
</template>
