<script setup lang="ts">
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui'
withDefaults(
  defineProps<{ open: boolean; label: string; side?: 'top' | 'bottom'; width?: string }>(),
  { side: 'bottom', width: '320px' }
)
defineEmits<{ 'update:open': [value: boolean] }>()
</script>
<template>
  <PopoverRoot :open="open" @update:open="$emit('update:open', $event)">
    <PopoverTrigger as-child><slot name="trigger" /></PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        :side="side"
        align="end"
        :side-offset="6"
        :collision-padding="12"
        :aria-label="label"
        class="pointer-events-auto z-[220] flex max-h-[var(--reka-popover-content-available-height)] max-w-[calc(100vw-24px)] flex-col overflow-hidden rounded-lg border border-border bg-surface text-primary shadow-lg dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
        :style="{ width }"
        ><slot
      /></PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
