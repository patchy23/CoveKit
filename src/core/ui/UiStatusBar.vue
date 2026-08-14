<script setup lang="ts">
withDefaults(defineProps<{ tone?: 'neutral' | 'success' | 'danger' }>(), { tone: 'neutral' })
</script>

<template>
  <div
    class="flex h-[24px] shrink-0 items-center gap-[12px] overflow-hidden border-t border-border bg-surface-muted px-[8px] text-caption text-text-muted dark:border-border-dark dark:bg-surface-muted-dark dark:text-text-muted-dark"
  >
    <span
      class="h-[7px] w-[7px] shrink-0 rounded-full"
      :class="{
        'bg-text-muted dark:bg-text-muted-dark': tone === 'neutral',
        'bg-success-strong dark:bg-success-dark': tone === 'success',
        'bg-danger-strong dark:bg-danger-dark': tone === 'danger',
      }"
    />
    <slot />
  </div>
</template>
