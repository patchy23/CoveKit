<script setup lang="ts">
/** 无层级列表，顺序更新仅通过 move 请求。 */
import { computed } from 'vue'
import CollectionSurface from './collection/CollectionSurface.vue'
import type { UiListItem, UiCollectionMove, UiDropGuard } from './collection/types'
const props = withDefaults(
  defineProps<{
    items: UiListItem[]
    modelValue?: string
    rowHeight?: 22 | 24 | 28 | 32
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
    canDrop: undefined,
    error: '',
    rowHeight: 28,
    draggable: true,
    label: '可排序列表',
    emptyText: '暂无项目',
  }
)
const rows = computed(() => props.items.map((item) => ({ ...item, depth: 0 })))
const emit = defineEmits<{
  'update:modelValue': [id: string]
  select: [item: UiListItem]
  open: [item: UiListItem]
  contextmenu: [item: UiListItem, event: MouseEvent]
  blankContextmenu: [event: MouseEvent]
  move: [move: UiCollectionMove]
  retry: []
}>()
</script>
<template>
  <CollectionSurface
    v-bind="$props"
    :items="rows"
    @update:model-value="emit('update:modelValue', $event)"
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
