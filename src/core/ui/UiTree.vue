<script setup lang="ts">
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

withDefaults(
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
  (event: 'context', mouse: MouseEvent, item: UiTreeItem): void
}>()

function kindGlyph(kind?: string): string {
  switch (kind) {
    case 'database':
      return '◈'
    case 'schema':
      return '◇'
    case 'table':
      return '▦'
    case 'view':
      return '◫'
    case 'function':
      return 'ƒ'
    case 'group':
      return '⌘'
    default:
      return '◇'
  }
}
</script>

<template>
  <div role="tree" class="min-h-0 overflow-auto py-[3px] text-body-sm">
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      role="treeitem"
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
      @click="emit('update:modelValue', item.id)"
      @dblclick="item.expandable && emit('toggle', item)"
      @contextmenu.prevent="emit('context', $event, item)"
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
        <!-- loading：旋转圆弧（参考 dbx Loader2） -->
        <svg
          v-if="item.loading"
          class="h-[12px] w-[12px] animate-spin"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
        >
          <path d="M21 12a9 9 0 1 1-6.219-8.56" />
        </svg>
        <!-- 可展开：chevron（参考 dbx ChevronRight/ChevronDown，旋转过渡） -->
        <svg
          v-else-if="item.expandable"
          class="h-[12px] w-[12px] transition-transform duration-150"
          :class="{ 'rotate-90': item.expanded }"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="m9 18 6-6-6-6" />
        </svg>
      </span>
      <slot name="icon" :item="item">
        <span class="w-[15px] shrink-0 text-center text-caption">{{ kindGlyph(item.kind) }}</span>
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
</template>
