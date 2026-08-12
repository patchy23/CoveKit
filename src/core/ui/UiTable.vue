<script setup lang="ts">
withDefaults(
  defineProps<{
    density?: 'compact' | 'default' | 'comfortable'
    striped?: boolean
    hoverable?: boolean
  }>(),
  { density: 'default', striped: false, hoverable: true }
)
</script>

<template>
  <div class="overflow-x-auto rounded-lg border border-border dark:border-border-dark">
    <table
      class="ui-data-table w-full border-collapse text-left text-secondary dark:text-secondary-dark"
      :class="[
        `ui-data-table-${density}`,
        { 'ui-data-table-striped': striped, 'ui-data-table-hoverable': hoverable },
      ]"
    >
      <slot />
    </table>
  </div>
</template>

<style scoped>
.ui-data-table :deep(th) {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
  font-weight: 600;
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
  background: var(--color-tertiary-soft);
}
:global([data-theme='dark']) .ui-data-table :deep(th),
:global([data-theme='dark']) .ui-data-table-striped :deep(tbody tr:nth-child(even)) {
  background: var(--color-surface-muted-dark);
}
:global([data-theme='dark']) .ui-data-table :deep(th),
:global([data-theme='dark']) .ui-data-table :deep(td) {
  border-color: var(--color-border-dark);
}
:global([data-theme='dark']) .ui-data-table-hoverable :deep(tbody tr:hover) {
  background: var(--color-tertiary-soft-dark);
}
</style>
