<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    /**
     * 密度档位；custom 是逃生舱：不带任何内置 padding/字号（类名 ui-data-table-custom 无对应样式），
     * 配 styled=false 或自行在 tableClass 里全权定义单元格排版时使用。
     */
    density?: 'compact' | 'default' | 'comfortable' | 'custom'
    striped?: boolean
    hoverable?: boolean
    framed?: boolean
    tableClass?: string
    styled?: boolean
  }>(),
  {
    density: 'default',
    striped: false,
    hoverable: true,
    framed: true,
    tableClass: '',
    styled: true,
  }
)

/** framed / unframed 两形态共用同一份表格类列表 */
const classes = computed(() => [
  'w-full border-collapse text-left text-secondary dark:text-secondary-dark',
  props.tableClass,
  { 'ui-data-table': props.styled },
  props.styled ? `ui-data-table-${props.density}` : '',
  { 'ui-data-table-striped': props.striped, 'ui-data-table-hoverable': props.hoverable },
])
</script>

<template>
  <UiScrollArea v-if="framed" as-child axis="horizontal">
    <div class="rounded-lg border border-border dark:border-border-dark">
      <table :class="classes">
        <slot />
      </table>
    </div>
  </UiScrollArea>
  <table v-else :class="classes">
    <slot />
  </table>
</template>

<style scoped>
.ui-data-table > :deep(thead > tr > th) {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
  font-weight: 500;
}
.ui-data-table > :deep(thead > tr > th),
.ui-data-table > :deep(tbody > tr > td) {
  border-bottom: 1px solid var(--color-border);
}
.ui-data-table > :deep(tbody > tr:last-child > td) {
  border-bottom: 0;
}
.ui-data-table-compact > :deep(thead > tr > th),
.ui-data-table-compact > :deep(tbody > tr > td) {
  padding: 5px 8px;
  font-size: var(--text-caption);
}
.ui-data-table-default > :deep(thead > tr > th),
.ui-data-table-default > :deep(tbody > tr > td) {
  padding: 8px 10px;
  font-size: var(--text-body-sm);
}
.ui-data-table-comfortable > :deep(thead > tr > th),
.ui-data-table-comfortable > :deep(tbody > tr > td) {
  padding: 12px 14px;
  font-size: var(--text-body);
}
.ui-data-table-striped > :deep(tbody > tr:nth-child(even of :not([data-table-detail]))) {
  background: var(--color-surface-muted);
}
.ui-data-table-hoverable > :deep(tbody > tr:not([data-table-detail]):hover) {
  background: var(--ui-table-row-hover);
}
.ui-data-table-hoverable > :deep(tbody > tr[data-selected='true']),
.ui-data-table-hoverable > :deep(tbody > tr[data-selected='true']:hover) {
  background: var(--ui-table-row-selected);
}
</style>
