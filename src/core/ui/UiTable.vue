<script setup lang="ts">
withDefaults(
  defineProps<{
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
</script>

<template>
  <div
    v-if="framed"
    class="overflow-x-auto rounded-lg border border-border dark:border-border-dark"
  >
    <table
      class="w-full border-collapse text-left text-secondary dark:text-secondary-dark"
      :class="[
        tableClass,
        { 'ui-data-table': styled },
        styled ? `ui-data-table-${density}` : '',
        { 'ui-data-table-striped': striped, 'ui-data-table-hoverable': hoverable },
      ]"
    >
      <slot />
    </table>
  </div>
  <table
    v-else
    class="w-full border-collapse text-left text-secondary dark:text-secondary-dark"
    :class="[
      tableClass,
      { 'ui-data-table': styled },
      styled ? `ui-data-table-${density}` : '',
      { 'ui-data-table-striped': striped, 'ui-data-table-hoverable': hoverable },
    ]"
  >
    <slot />
  </table>
</template>

<style scoped>
.ui-data-table :deep(th) {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
  font-weight: 500;
}
.ui-data-table :deep(th),
.ui-data-table :deep(td) {
  border-bottom: 1px solid var(--color-border);
}
.ui-data-table :deep(tbody tr:last-child td) {
  border-bottom: 0;
}
.ui-data-table-compact :deep(th),
.ui-data-table-compact :deep(td) {
  padding: 5px 8px;
  font-size: var(--text-caption);
}
.ui-data-table-default :deep(th),
.ui-data-table-default :deep(td) {
  padding: 8px 10px;
  font-size: var(--text-body-sm);
}
.ui-data-table-comfortable :deep(th),
.ui-data-table-comfortable :deep(td) {
  padding: 12px 14px;
  font-size: var(--text-body);
}
.ui-data-table-striped :deep(tbody tr:nth-child(even)) {
  background: var(--color-surface-muted);
}
.ui-data-table-hoverable :deep(tbody tr:hover) {
  background: var(--ui-table-row-hover);
}
.ui-data-table-hoverable :deep(tbody tr[data-selected='true']),
.ui-data-table-hoverable :deep(tbody tr[data-selected='true']:hover) {
  background: var(--ui-table-row-selected);
}
</style>
