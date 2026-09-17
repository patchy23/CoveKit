<script setup lang="ts">
import UiTooltip from './UiTooltip.vue'
export interface UiWorkbenchTab {
  id: string
  label: string
  kind?: 'query' | 'data' | 'structure' | 'custom'
  dirty?: boolean
  pinned?: boolean
  closable?: boolean
}

defineProps<{ modelValue: string; items: UiWorkbenchTab[] }>()
const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'close', value: string): void
  (event: 'context', mouse: MouseEvent, item: UiWorkbenchTab): void
}>()

function kindGlyph(item: UiWorkbenchTab): string {
  if (item.pinned) return '◆'
  if (item.kind === 'query') return 'SQL'
  if (item.kind === 'structure') return 'DDL'
  return '▦'
}
</script>

<template>
  <div
    class="flex h-[28px] min-w-0 shrink-0 overflow-x-auto border-b border-border bg-surface-muted dark:border-border-dark dark:bg-surface-muted-dark"
  >
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="relative flex h-full max-w-[190px] shrink-0 items-center gap-[5px] border-r border-border px-[9px] text-body-sm text-secondary outline-none hover:bg-surface focus-visible:bg-surface dark:border-border-dark dark:text-secondary-dark dark:hover:bg-surface-dark dark:focus-visible:bg-surface-dark"
      :class="
        modelValue === item.id
          ? 'bg-surface text-primary after:absolute after:inset-x-0 after:bottom-0 after:h-[2px] after:bg-tertiary dark:bg-surface-dark dark:text-primary-dark dark:after:bg-tertiary-dark'
          : ''
      "
      @click="emit('update:modelValue', item.id)"
      @contextmenu.prevent="emit('context', $event, item)"
    >
      <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
        kindGlyph(item)
      }}</span>
      <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
      <UiTooltip v-if="item.dirty" content="未保存">
        <span class="h-[6px] w-[6px] shrink-0 rounded-full bg-tertiary" />
      </UiTooltip>
      <span
        v-if="item.closable !== false && !item.pinned"
        role="button"
        tabindex="0"
        class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[3px] text-caption hover:bg-border dark:hover:bg-border-dark"
        :aria-label="`关闭${item.label}`"
        @click.stop="emit('close', item.id)"
        @keydown.enter.stop="emit('close', item.id)"
        >×</span
      >
    </button>
  </div>
</template>
