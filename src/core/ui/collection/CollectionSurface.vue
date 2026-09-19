<script setup lang="ts">
/** 列表与树共用的行、焦点与落点视图；数据和持久化完全受控。 */
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiScrollArea from '../UiScrollArea.vue'
import UiIcon from '../UiIcon.vue'
import UiSpinner from '../UiSpinner.vue'
import UiButton from '../UiButton.vue'
import { parentOf, type UiCollectionMove, type UiDropGuard, type UiTreeItem } from './types'
import { useCollectionDrag } from './useCollectionDrag'

const props = withDefaults(
  defineProps<{
    items: UiTreeItem[]
    tree?: boolean
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
    rowHeight: 28,
    emptyText: '暂无项目',
    label: '项目列表',
    error: '',
  }
)
const emit = defineEmits<{
  'update:modelValue': [id: string]
  select: [item: UiTreeItem]
  toggle: [item: UiTreeItem]
  open: [item: UiTreeItem]
  contextmenu: [item: UiTreeItem, event: MouseEvent]
  blankContextmenu: [event: MouseEvent]
  move: [move: UiCollectionMove]
  retry: []
}>()
const root = ref<HTMLElement | null>(null)
const focusedId = ref('')
const instanceId = useId()
const locked = computed(() => props.disabled || props.busy || props.loading)
const enabled = () => !!props.draggable && !locked.value && !props.filtered
const { drag, target, start, cancel, captureClick, allowed } = useCollectionDrag(root, {
  items: () => props.items,
  enabled,
  tree: () => !!props.tree,
  guard: () => props.canDrop,
  move: (move) => emit('move', move),
  expand: (item) => emit('toggle', item),
})
const focusable = computed(() => props.items.filter((item) => !item.disabled))
const tabId = computed(() => {
  const ids = focusable.value.map((item) => item.id)
  return ids.includes(focusedId.value)
    ? focusedId.value
    : ids.includes(props.modelValue)
      ? props.modelValue
      : ids[0]
})
watch(
  () => props.items,
  () => {
    if (drag.value && !props.items.some((item) => item.id === drag.value?.id)) cancel()
    const active = document.activeElement
    if (
      active instanceof HTMLElement &&
      active.hasAttribute('data-collection-id') &&
      root.value?.contains(active)
    ) {
      const id = active.dataset.collectionId
      void nextTick(() => focus(props.items.find((item) => item.id === id)))
    }
  }
)
function domId(item: UiTreeItem) {
  return `${instanceId}-${encodeURIComponent(item.id)}`
}
function focus(item?: UiTreeItem) {
  if (!item || item.disabled) return
  focusedId.value = item.id
  const element = root.value?.querySelector<HTMLElement>(`[id="${domId(item)}"]`)
  element?.focus({ preventScroll: true })
  element?.scrollIntoView?.({ block: 'nearest' })
}
function select(item: UiTreeItem) {
  if (locked.value || item.disabled) return
  focusedId.value = item.id
  emit('update:modelValue', item.id)
  emit('select', item)
}
function interactive(event: Event) {
  return (
    event.target instanceof Element &&
    !!event.target.closest('button,input,textarea,select,a,[contenteditable="true"],[data-no-drag]')
  )
}
function click(event: MouseEvent, item: UiTreeItem) {
  if (!interactive(event)) select(item)
}
function open(event: MouseEvent, item: UiTreeItem) {
  if (interactive(event) || locked.value || item.disabled) return
  if (item.expandable) emit('toggle', item)
  else emit('open', item)
}
function context(event: MouseEvent, item?: UiTreeItem) {
  cancel()
  event.preventDefault()
  event.stopPropagation()
  if (item) emit('contextmenu', item, event)
  else emit('blankContextmenu', event)
}
function keyboardMove(event: KeyboardEvent, item: UiTreeItem) {
  if (!enabled()) return
  const parent = parentOf(props.items, item.id)
  const peers = props.items.filter((row) => parentOf(props.items, row.id)?.id === parent?.id)
  const index = peers.findIndex((row) => row.id === item.id)
  let move: UiCollectionMove | undefined
  if (event.key === 'ArrowUp' && peers[index - 1])
    move = { id: item.id, targetId: peers[index - 1].id, position: 'before' }
  if (event.key === 'ArrowDown' && peers[index + 1])
    move = { id: item.id, targetId: peers[index + 1].id, position: 'after' }
  if (props.tree && event.key === 'ArrowLeft' && parent)
    move = { id: item.id, targetId: parent.id, position: 'after' }
  if (props.tree && event.key === 'ArrowRight' && peers[index - 1]?.expandable)
    move = { id: item.id, targetId: peers[index - 1].id, position: 'inside' }
  if (move && allowed(move)) emit('move', move)
}
function keydown(event: KeyboardEvent, item: UiTreeItem) {
  if (interactive(event) || locked.value || item.disabled) return
  const key = event.key
  if (
    !['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter', ' '].includes(key)
  )
    return
  event.preventDefault()
  event.stopPropagation()
  if (key === 'Enter' && (event.ctrlKey || event.metaKey) && !item.expandable) {
    emit('open', item)
    return
  }
  if (event.altKey && key.startsWith('Arrow')) {
    keyboardMove(event, item)
    return
  }
  const index = focusable.value.findIndex((row) => row.id === item.id)
  if (key === 'ArrowUp') focus(focusable.value[index - 1])
  else if (key === 'ArrowDown') focus(focusable.value[index + 1])
  else if (key === 'Home') focus(focusable.value[0])
  else if (key === 'End') focus(focusable.value.at(-1))
  else if (key === 'Enter' || key === ' ') select(item)
  else if (props.tree && key === 'ArrowRight') {
    if (item.expandable && !item.expanded) emit('toggle', item)
    else if ((focusable.value[index + 1]?.depth ?? -1) > item.depth)
      focus(focusable.value[index + 1])
  } else if (props.tree && key === 'ArrowLeft') {
    if (item.expandable && item.expanded) emit('toggle', item)
    else focus(parentOf(props.items, item.id))
  }
}
const feedback = computed(() => {
  if (!target.value) return '不能放置在此处'
  if (target.value.targetId === null) return '移至末尾'
  const label = props.items.find((item) => item.id === target.value?.targetId)?.label ?? ''
  return target.value.position === 'inside'
    ? `移入 ${label}`
    : `放在 ${label} ${target.value.position === 'before' ? '之前' : '之后'}`
})
/** 放到目录之后时，插入线落在可见子树末尾，避免与“移入目录”混淆。 */
const insertion = computed(() => {
  const move = target.value
  if (!move || move.targetId === null || move.position === 'inside') return null
  let index = props.items.findIndex((item) => item.id === move.targetId)
  const item = props.items[index]
  if (!item) return null
  if (props.tree && move.position === 'after')
    while ((props.items[index + 1]?.depth ?? -1) > item.depth) index++
  return { id: props.items[index].id, depth: item.depth, position: move.position }
})
</script>

<template>
  <UiScrollArea as-child axis="vertical">
    <div
      ref="root"
      :role="tree ? 'tree' : 'listbox'"
      :aria-label="label"
      :aria-busy="busy || loading"
      :aria-disabled="disabled"
      class="relative min-h-0 flex-1 select-none px-[6px] py-[4px] text-body-sm"
      @click.capture="captureClick"
      @contextmenu.self="context($event)"
    >
      <div
        v-if="error"
        role="alert"
        class="px-[8px] py-[6px] text-danger-strong dark:text-danger-dark"
      >
        {{ error }}<UiButton size="xs" variant="ghost" @click="emit('retry')">重试</UiButton>
      </div>
      <div
        v-if="loading && !items.length"
        role="status"
        class="flex items-center justify-center gap-[6px] py-[12px] text-text-muted dark:text-text-muted-dark"
      >
        <UiSpinner size="sm" />正在读取…
      </div>
      <div
        v-else-if="!items.length && !error"
        class="px-[8px] py-[12px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        @contextmenu="context($event)"
      >
        <slot name="empty">{{ emptyText }}</slot>
      </div>
      <div
        v-for="item in items"
        :id="domId(item)"
        :key="item.id"
        :role="tree ? 'treeitem' : 'option'"
        :data-collection-id="item.id"
        :data-depth="item.depth"
        :tabindex="!locked && tabId === item.id ? 0 : -1"
        :aria-selected="modelValue === item.id"
        :aria-disabled="locked || item.disabled"
        :aria-expanded="tree && item.expandable ? !!item.expanded : undefined"
        :aria-level="tree ? item.depth + 1 : undefined"
        :aria-keyshortcuts="draggable ? 'Alt+ArrowUp Alt+ArrowDown' : undefined"
        class="group relative flex w-full items-center gap-[5px] rounded-sm pr-[6px] text-secondary outline-none transition-colors focus-visible:ring-1 focus-visible:ring-tertiary-strong dark:text-secondary-dark dark:focus-visible:ring-tertiary-dark"
        :class="[
          locked || item.disabled ? 'opacity-50' : 'hover:bg-border dark:hover:bg-border-dark',
          modelValue === item.id
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : '',
          item.muted ? 'opacity-60' : '',
          target?.targetId === item.id && target.position === 'inside'
            ? 'ring-1 ring-inset ring-tertiary-strong bg-tertiary-soft dark:ring-tertiary-dark dark:bg-tertiary-soft-dark'
            : '',
          enabled() && item.draggable !== false ? 'cursor-grab' : 'cursor-default',
          drag?.active && drag.id === item.id ? 'opacity-40' : '',
        ]"
        :style="{ minHeight: `${rowHeight}px`, paddingLeft: `${item.depth * 18 + 5}px` }"
        @click="click($event, item)"
        @dblclick="open($event, item)"
        @focus="focusedId = item.id"
        @keydown="keydown($event, item)"
        @contextmenu="context($event, item)"
        @pointerdown="!dragHandle && start($event, item)"
      >
        <span
          v-if="insertion?.id === item.id"
          data-drop-line
          aria-hidden="true"
          class="pointer-events-none absolute right-0 z-10 h-[2px] rounded-full bg-tertiary-strong dark:bg-tertiary-dark"
          :class="insertion.position === 'before' ? 'top-0' : 'bottom-0'"
          :style="{ left: `${insertion.depth * 18 + 5}px` }"
        />
        <button
          v-if="dragHandle && draggable"
          type="button"
          data-drag-handle
          :disabled="locked || item.disabled || item.draggable === false"
          :aria-label="`拖动 ${item.label}；也可聚焦行后按 Alt 加方向键排序`"
          class="shrink-0 cursor-grab text-text-muted dark:text-text-muted-dark"
          @pointerdown.stop="start($event, item)"
        >
          <UiIcon name="grip-vertical" :size="12" />
        </button>
        <button
          v-if="tree && item.expandable"
          type="button"
          :disabled="locked || item.disabled || item.loading"
          :aria-label="`${item.expanded ? '收起' : '展开'} ${item.label}`"
          :aria-expanded="!!item.expanded"
          class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[4px] hover:bg-border dark:hover:bg-border-dark"
          @click.stop="emit('toggle', item)"
        >
          <UiSpinner v-if="item.loading" size="sm" /><UiIcon
            v-else
            name="chevron-right"
            :size="12"
            class="transition-transform duration-150"
            :class="{ 'rotate-90': item.expanded }"
          />
        </button>
        <span v-else-if="tree" class="w-[16px] shrink-0" />
        <slot name="row" :item="item" :selected="modelValue === item.id">
          <slot name="icon" :item="item"
            ><UiIcon
              :name="item.expandable ? 'folder' : 'square'"
              :size="12"
              class="shrink-0 text-text-muted dark:text-text-muted-dark"
          /></slot>
          <div class="min-w-0 flex-1">
            <slot name="label" :item="item"
              ><span class="block truncate">{{ item.label }}</span></slot
            >
            <span
              v-if="item.description"
              class="block truncate text-caption text-text-muted dark:text-text-muted-dark"
              >{{ item.description }}</span
            >
          </div>
          <span
            v-if="item.badge !== undefined"
            class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
            >{{ item.badge }}</span
          >
        </slot>
        <div
          v-if="$slots.suffix"
          data-no-drag
          class="flex shrink-0 items-center"
          @click.stop
          @dblclick.stop
        >
          <slot name="suffix" :item="item" />
        </div>
      </div>
      <div
        v-if="target && target.targetId === null"
        class="pointer-events-none sticky bottom-0 h-0"
        aria-hidden="true"
      >
        <div
          class="absolute inset-x-0 bottom-0 h-[2px] rounded-full bg-tertiary-strong dark:bg-tertiary-dark"
        />
      </div>
      <div v-if="busy" role="status" class="pointer-events-none absolute right-[8px] top-[6px]">
        <UiSpinner size="sm" label="正在保存顺序" />
      </div>
      <Teleport to="body"
        ><div
          v-if="drag?.active"
          class="pointer-events-none fixed z-[300] max-w-[280px] truncate rounded-md border border-border bg-surface px-[8px] py-[5px] text-body-sm text-primary shadow-sm dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
          :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 14}px` }"
        >
          {{ drag.label }} · {{ feedback }}
        </div></Teleport
      >
    </div>
  </UiScrollArea>
</template>
