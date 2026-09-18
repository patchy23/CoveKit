<script setup lang="ts">
import { ref } from 'vue'
import UiScrollArea from './UiScrollArea.vue'
import UiIcon from './UiIcon.vue'
import UiSpinner from './UiSpinner.vue'

export interface UiTreeItem {
  id: string
  label: string
  depth: number
  kind?: string
  expanded?: boolean
  expandable?: boolean
  loading?: boolean
  badge?: string | number
  muted?: boolean
}

const props = withDefaults(
  defineProps<{
    items: UiTreeItem[]
    modelValue?: string
    rowHeight?: 22 | 24 | 28
  }>(),
  { modelValue: '', rowHeight: 24 }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'toggle', item: UiTreeItem): void
  /** 右键：统一约定 payload 在前、鼠标事件在后（与 UiTabs 的 contextmenu 一致） */
  (event: 'contextmenu', item: UiTreeItem, mouse: MouseEvent): void
  /** 双击不可展开的叶子节点（业务打开语义，如查看结构）；可展开节点双击仍为折叠/展开 */
  (event: 'open', item: UiTreeItem): void
}>()

/** 键盘焦点所在项（roving tabindex；独立于选中态） */
const focusedId = ref('')

/** 方向键导航：↑↓ 移动焦点；→ 展开或进入首个子项；← 收起或回到父项 */
function onKeydown(event: KeyboardEvent, item: UiTreeItem, index: number) {
  const key = event.key
  if (key !== 'ArrowDown' && key !== 'ArrowUp' && key !== 'ArrowLeft' && key !== 'ArrowRight')
    return
  event.preventDefault()
  event.stopPropagation()
  if (key === 'ArrowDown' || key === 'ArrowUp') {
    const next = key === 'ArrowDown' ? index + 1 : index - 1
    focusItem(props.items[next])
    return
  }
  if (key === 'ArrowRight') {
    if (item.expandable && !item.expanded) emit('toggle', item)
    else focusItem(props.items[index + 1])
    return
  }
  // ArrowLeft：已展开则收起；否则找到最近的上一级节点并聚焦
  if (item.expandable && item.expanded) {
    emit('toggle', item)
    return
  }
  for (let i = index - 1; i >= 0; i--) {
    if (props.items[i].depth === item.depth - 1) {
      focusItem(props.items[i])
      return
    }
  }
}

/** 把焦点落到指定树节点按钮上 */
function focusItem(item?: UiTreeItem) {
  if (!item) return
  focusedId.value = item.id
  document.getElementById(treeItemId(item.id))?.focus({ preventScroll: true })
}

/** 节点 id → DOM id（供方向键定位焦点） */
function treeItemId(id: string): string {
  return `ui-tree-item-${id.replace(/[^\w-]/g, '_')}`
}

/**
 * 默认节点图标（业务方可通过 #icon 插槽整体覆盖，如 database 插件的对象图标）
 * group/分组类 → 文件夹；其余 → 小方块
 */
function isGroupKind(kind?: string): boolean {
  return !!kind && (kind === 'group' || kind.startsWith('group-'))
}
</script>

<template>
  <UiScrollArea as-child axis="vertical">
    <div role="tree" class="min-h-0 overflow-x-hidden py-[3px] text-body-sm">
      <button
        v-for="(item, index) in items"
        :id="treeItemId(item.id)"
        :key="item.id"
        type="button"
        role="treeitem"
        :tabindex="(focusedId || modelValue) === item.id ? 0 : -1"
        class="group flex w-full items-center gap-[4px] whitespace-nowrap pr-[6px] text-left text-secondary outline-none transition-colors hover:bg-border focus-visible:bg-border dark:text-secondary-dark dark:hover:bg-border-dark dark:focus-visible:bg-border-dark"
        :class="[
          modelValue === item.id
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : '',
          item.muted ? 'opacity-60' : '',
        ]"
        :style="{ height: `${rowHeight}px`, paddingLeft: `${item.depth * 14 + 5}px` }"
        :aria-selected="modelValue === item.id"
        :aria-expanded="item.expandable ? item.expanded : undefined"
        :aria-level="item.depth + 1"
        @click="emit('update:modelValue', item.id)"
        @focus="focusedId = item.id"
        @keydown="onKeydown($event, item, index)"
        @dblclick="item.expandable ? emit('toggle', item) : emit('open', item)"
        @contextmenu.prevent="emit('contextmenu', item, $event)"
      >
        <span
          class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[4px] text-text-muted transition-colors dark:text-text-muted-dark"
          :class="
            item.expandable || item.loading
              ? 'cursor-pointer hover:bg-border hover:text-secondary dark:hover:bg-border-dark dark:hover:text-secondary-dark'
              : ''
          "
          @click.stop="item.expandable && emit('toggle', item)"
        >
          <!-- loading：统一走 UiSpinner（单一转圈实现源） -->
          <UiSpinner v-if="item.loading" size="sm" label="加载中" />
          <!-- 可展开：chevron（展开时旋转 90°） -->
          <UiIcon
            v-else-if="item.expandable"
            name="chevron-right"
            :size="12"
            class="transition-transform duration-150"
            :class="{ 'rotate-90': item.expanded }"
          />
        </span>
        <slot name="icon" :item="item">
          <svg
            class="h-[12px] w-[12px] shrink-0 text-text-muted dark:text-text-muted-dark"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <!-- 分组：文件夹；叶子：小方块 -->
            <path
              v-if="isGroupKind(item.kind)"
              d="M4 6.75A1.75 1.75 0 0 1 5.75 5h3.4l1.9 2.25h7.2A1.75 1.75 0 0 1 20 9v8.25A1.75 1.75 0 0 1 18.25 19H5.75A1.75 1.75 0 0 1 4 17.25Z"
            />
            <rect v-else x="7" y="7" width="10" height="10" rx="2" />
          </svg>
        </slot>
        <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
        <span
          v-if="item.badge !== undefined"
          class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
        >
          {{ item.badge }}
        </span>
        <slot name="suffix" :item="item" />
      </button>
    </div>
  </UiScrollArea>
</template>
